use anyhow::Result;
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use fluent_syntax::parser::parse;
use std::collections::HashMap;

// Constants for better maintainability
const FLUENT_INDENTATION: &str = "    ";
const UNSUPPORTED_PLACEHOLDER: &str = "{unsupported}";
const PLURAL_VARIANT_ORDER: &[&str] = &["zero", "one", "two", "few", "many"];
const OTHER_VARIANT: &str = "other";

#[derive(Debug, Clone)]
pub struct FluentMessage {
    pub id: String,
    pub value: Option<FluentPattern>,
    pub attributes: HashMap<String, FluentPattern>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FluentPattern {
    pub elements: Vec<FluentElement>,
}

#[derive(Debug, Clone)]
pub enum FluentElement {
    Text(String),
    Variable(String),
    Plural {
        selector: String,
        variants: HashMap<String, FluentPattern>,
    },
}

#[derive(Debug)]
pub struct FluentResource {
    pub messages: Vec<FluentMessage>,
}

impl FluentResource {
    /// Parse a Fluent source string into a FluentResource
    /// 
    /// Uses the fluent-syntax parser's built-in comment handling
    pub fn from_source(source: &str) -> Result<Self> {
        let resource = Self::parse_with_error_handling(source)?;
        
        let mut parser = FluentResourceParser::new();
        parser.process_entries(resource.body)?;
        
        Ok(FluentResource { 
            messages: parser.messages 
        })
    }

    pub fn to_source(&self) -> String {
        let mut output = String::new();

        for (i, message) in self.messages.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            
            Self::write_message_comment(&mut output, &message.comment);
            Self::write_message_definition(&mut output, message);
            Self::write_message_attributes(&mut output, &message.attributes);
        }

        output
    }

    fn parse_with_error_handling(source: &str) -> Result<fluent_syntax::ast::Resource<&str>> {
        match parse(source) {
            Ok(resource) => Ok(resource),
            Err((resource, errors)) => {
                if errors.is_empty() {
                    Ok(resource)
                } else {
                    Err(anyhow::anyhow!("Fluent parse errors: {:#?}", errors))
                }
            }
        }
    }

    fn write_message_comment(output: &mut String, comment: &Option<String>) {
        if let Some(comment) = comment {
            for line in comment.lines() {
                output.push_str(&format!("# {}\n", line));
            }
        }
    }

    fn write_message_definition(output: &mut String, message: &FluentMessage) {
        output.push_str(&message.id);
        
        if let Some(value) = &message.value {
            output.push_str(" = ");
            output.push_str(&pattern_to_string(value));
        }
        
        output.push('\n');
    }

    fn write_message_attributes(output: &mut String, attributes: &HashMap<String, FluentPattern>) {
        for (attr_name, attr_value) in attributes {
            output.push_str(&format!("{}.{} = {}\n", 
                FLUENT_INDENTATION, attr_name, pattern_to_string(attr_value)));
        }
        
        if !attributes.is_empty() {
            output.push('\n');
        }
    }
}

/// Internal parser state for processing Fluent AST entries
struct FluentResourceParser {
    messages: Vec<FluentMessage>,
}

impl FluentResourceParser {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    fn process_entries(&mut self, entries: Vec<Entry<&str>>) -> Result<()> {
        for entry in entries {
            match entry {
                Entry::Message(message) => self.process_message(message),
                Entry::Comment(_) => {
                    // Standalone comments are ignored - only use parser's built-in comment association
                }
                Entry::GroupComment(_) | Entry::ResourceComment(_) => {
                    // Ignore group and resource comments for now
                }
                Entry::Term(_) => {
                    // Handle terms if needed in the future
                }
                Entry::Junk { .. } => {
                    // Ignore junk entries
                }
            }
        }
        Ok(())
    }

    fn process_message(&mut self, message: fluent_syntax::ast::Message<&str>) {
        let message_id = message.id.name.to_string();
        
        // Only use comments directly associated with the message by the fluent-syntax parser
        let comment = message.comment.map(|msg_comment| msg_comment.content.join("\n"));
        
        let fluent_message = FluentMessage {
            id: message_id,
            value: message.value.map(|pattern| convert_pattern(&pattern)),
            attributes: self.convert_attributes(message.attributes),
            comment,
        };
        
        self.messages.push(fluent_message);
    }

    fn convert_attributes(
        &self, 
        attributes: Vec<fluent_syntax::ast::Attribute<&str>>
    ) -> HashMap<String, FluentPattern> {
        attributes
            .into_iter()
            .map(|attr| (attr.id.name.to_string(), convert_pattern(&attr.value)))
            .collect()
    }
}

fn convert_pattern(pattern: &Pattern<&str>) -> FluentPattern {
    let elements = pattern.elements
        .iter()
        .map(convert_pattern_element)
        .collect();

    FluentPattern { elements }
}

fn convert_pattern_element(element: &PatternElement<&str>) -> FluentElement {
    match element {
        PatternElement::TextElement { value } => {
            FluentElement::Text(value.to_string())
        }
        PatternElement::Placeable { expression } => {
            convert_expression(expression)
        }
    }
}

fn convert_expression(expression: &Expression<&str>) -> FluentElement {
    match expression {
        Expression::Inline(InlineExpression::VariableReference { id }) => {
            FluentElement::Variable(id.name.to_string())
        }
        Expression::Select { selector, variants } => {
            convert_select_expression(selector, variants)
        }
        _ => {
            FluentElement::Text(UNSUPPORTED_PLACEHOLDER.to_string())
        }
    }
}

fn convert_select_expression(
    selector: &InlineExpression<&str>,
    variants: &[fluent_syntax::ast::Variant<&str>],
) -> FluentElement {
    if let InlineExpression::VariableReference { id } = selector {
        let selector_name = id.name.to_string();
        let variant_map = variants
            .iter()
            .map(|variant| {
                let key = variant_key_to_string(&variant.key);
                let pattern = convert_pattern(&variant.value);
                (key, pattern)
            })
            .collect();

        FluentElement::Plural {
            selector: selector_name,
            variants: variant_map,
        }
    } else {
        FluentElement::Text(UNSUPPORTED_PLACEHOLDER.to_string())
    }
}

fn variant_key_to_string(key: &fluent_syntax::ast::VariantKey<&str>) -> String {
    match key {
        fluent_syntax::ast::VariantKey::Identifier { name } => name.to_string(),
        fluent_syntax::ast::VariantKey::NumberLiteral { value } => {
            // Preserve the actual numeric value - don't convert to named forms
            // This is important for round-trip conversion especially for PO format
            value.to_string()
        }
    }
}

fn pattern_to_string(pattern: &FluentPattern) -> String {
    pattern.elements
        .iter()
        .map(element_to_string)
        .collect::<Vec<_>>()
        .join("")
}

fn element_to_string(element: &FluentElement) -> String {
    match element {
        FluentElement::Text(text) => format_multiline_text(text),
        FluentElement::Variable(var) => format!("{{${}}}", var),
        FluentElement::Plural { selector, variants } => format_plural(selector, variants),
    }
}

fn format_multiline_text(text: &str) -> String {
    if text.contains('\n') {
        let lines: Vec<&str> = text.split('\n').collect();
        let mut result = String::from(lines[0]);
        
        for line in &lines[1..] {
            result.push_str(&format!("\n{}{}", FLUENT_INDENTATION, line));
        }
        
        result
    } else {
        text.to_string()
    }
}

fn format_plural(selector: &str, variants: &HashMap<String, FluentPattern>) -> String {
    let mut result = format!("{{${} ->\n", selector);
    
    // Output variants in canonical order
    format_ordered_variants(&mut result, variants);
    format_remaining_variants(&mut result, variants);
    format_other_variant(&mut result, variants);
    
    result.push('}');
    result
}

fn format_ordered_variants(result: &mut String, variants: &HashMap<String, FluentPattern>) {
    for &key in PLURAL_VARIANT_ORDER {
        if let Some(variant_pattern) = variants.get(key) {
            result.push_str(&format!("{}[{}] {}\n", 
                FLUENT_INDENTATION, key, pattern_to_string(variant_pattern)));
        }
    }
}

fn format_remaining_variants(result: &mut String, variants: &HashMap<String, FluentPattern>) {
    for (key, variant_pattern) in variants {
        if !PLURAL_VARIANT_ORDER.contains(&key.as_str()) && key != OTHER_VARIANT {
            result.push_str(&format!("{}[{}] {}\n", 
                FLUENT_INDENTATION, key, pattern_to_string(variant_pattern)));
        }
    }
}

fn format_other_variant(result: &mut String, variants: &HashMap<String, FluentPattern>) {
    if let Some(other_pattern) = variants.get(OTHER_VARIANT) {
        result.push_str(&format!("   *[{}] {}\n", OTHER_VARIANT, pattern_to_string(other_pattern)));
    } else {
        result.push_str("   *[other] (missing)\n");
    }
}

/// Extract plain text from a FluentPattern for use in PO conversion
pub fn extract_pattern_text(pattern: &FluentPattern) -> String {
    pattern.elements
        .iter()
        .map(extract_element_text)
        .collect::<Vec<_>>()
        .join("")
}

fn extract_element_text(element: &FluentElement) -> String {
    match element {
        FluentElement::Text(text) => text.clone(),
        FluentElement::Variable(var) => format!("{{${}}}", var),
        FluentElement::Plural { selector, .. } => {
            // For plurals, we'll include the selector variable for now
            // This is a simplified approach - a full implementation would
            // need more sophisticated handling
            format!("{{ ${} }}", selector)
        }
    }
}

/// Simple function to parse Fluent content using the comprehensive parser
pub fn parse_fluent(content: &str) -> Result<FluentResource> {
    FluentResource::from_source(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a simple FluentPattern with text
    fn create_text_pattern(text: &str) -> FluentPattern {
        FluentPattern {
            elements: vec![FluentElement::Text(text.to_string())],
        }
    }

    // Helper function to assert a pattern contains expected text
    fn assert_pattern_text(pattern: &FluentPattern, expected: &str) {
        if let FluentElement::Text(text) = &pattern.elements[0] {
            assert_eq!(text, expected);
        } else {
            panic!("Expected text element, got {:?}", pattern.elements[0]);
        }
    }

    #[test]
    fn test_parse_simple_message() {
        let ftl = "hello = Hello World";
        let resource = FluentResource::from_source(ftl).unwrap();
        
        assert_eq!(resource.messages.len(), 1);
        assert_eq!(resource.messages[0].id, "hello");
        assert!(resource.messages[0].value.is_some());
        
        let pattern = resource.messages[0].value.as_ref().unwrap();
        assert_eq!(pattern.elements.len(), 1);
        assert_pattern_text(pattern, "Hello World");
    }

    #[test]
    fn test_parse_message_with_variable() {
        let ftl = "greeting = Hello, {$name}!";
        let resource = FluentResource::from_source(ftl).unwrap();

        assert_eq!(resource.messages.len(), 1);
        let pattern = resource.messages[0].value.as_ref().unwrap();
        assert_eq!(pattern.elements.len(), 3);

        assert!(matches!(&pattern.elements[0], FluentElement::Text(text) if text == "Hello, "));
        assert!(matches!(&pattern.elements[1], FluentElement::Variable(var) if var == "name"));
        assert!(matches!(&pattern.elements[2], FluentElement::Text(text) if text == "!"));
    }

    #[test]
    fn test_parse_plural_message() {
        let ftl = r#"items = {$count ->
    [one] {$count} item
   *[other] {$count} items
}"#;
        let resource = FluentResource::from_source(ftl).unwrap();
        
        assert_eq!(resource.messages.len(), 1);
        let pattern = resource.messages[0].value.as_ref().unwrap();
        assert_eq!(pattern.elements.len(), 1);
        
        if let FluentElement::Plural { selector, variants } = &pattern.elements[0] {
            assert_eq!(selector, "count");
            assert_eq!(variants.len(), 2);
            assert!(variants.contains_key("one"));
            assert!(variants.contains_key("other"));
            
            // Test that the variants contain the expected content
            let one_pattern = variants.get("one").unwrap();
            assert_eq!(extract_pattern_text(one_pattern), "{$count} item");
            
            let other_pattern = variants.get("other").unwrap();
            assert_eq!(extract_pattern_text(other_pattern), "{$count} items");
        } else {
            panic!("Expected plural element");
        }
    }

    #[test]
    fn test_parse_message_with_attributes() {
        let ftl = r#"login-button = Sign In
    .aria-label = Sign in to your account
    .title = Click to sign in"#;
        
        let resource = FluentResource::from_source(ftl).unwrap();
        
        assert_eq!(resource.messages.len(), 1);
        let message = &resource.messages[0];
        assert_eq!(message.id, "login-button");
        
        // Check main value
        assert!(message.value.is_some());
        let value = message.value.as_ref().unwrap();
        assert_eq!(value.elements.len(), 1);
        assert_pattern_text(value, "Sign In");
        
        // Check attributes
        assert_eq!(message.attributes.len(), 2);
        assert!(message.attributes.contains_key("aria-label"));
        assert!(message.attributes.contains_key("title"));
        
        let aria_label = message.attributes.get("aria-label").unwrap();
        assert_pattern_text(aria_label, "Sign in to your account");
    }

    #[test]
    fn test_parse_multi_line_comments() {
        let ftl = r#"# This is a greeting message
# It supports internationalization
# and has multiple lines of comments
hello = Hello World

# Another comment
# for a different message
goodbye = Goodbye!"#;
        
        let resource = FluentResource::from_source(ftl).unwrap();
        
        assert_eq!(resource.messages.len(), 2);
        
        // Check first message comment - assert exact content to ensure no extra characters or indentation
        assert!(resource.messages[0].comment.is_some());
        let hello_comment = resource.messages[0].comment.as_ref().unwrap();
        let expected_hello_comment = "This is a greeting message\nIt supports internationalization\nand has multiple lines of comments";
        assert_eq!(hello_comment, expected_hello_comment,
                   "Comment should contain exact content without # characters or extra indentation");
        
        // Check second message comment - assert exact content
        assert!(resource.messages[1].comment.is_some());
        let goodbye_comment = resource.messages[1].comment.as_ref().unwrap();
        let expected_goodbye_comment = "Another comment\nfor a different message";
        assert_eq!(goodbye_comment, expected_goodbye_comment,
                   "Comment should contain exact content without # characters or extra indentation");
    }

    #[test]
    fn test_parse_numeric_plural_variants() {
        let ftl = r#"files = {$count ->
    [0] No files
    [1] One file
   *[other] {$count} files
}"#;
        
        let resource = FluentResource::from_source(ftl).unwrap();
        
        assert_eq!(resource.messages.len(), 1);
        let pattern = resource.messages[0].value.as_ref().unwrap();
        
        if let FluentElement::Plural { variants, .. } = &pattern.elements[0] {
            assert!(variants.contains_key("0"));
            assert!(variants.contains_key("1"));
            assert!(variants.contains_key("other"));
            
            // Check that numeric keys are preserved
            let zero_pattern = variants.get("0").unwrap();
            assert_pattern_text(zero_pattern, "No files");
        } else {
            panic!("Expected plural element");
        }
    }

    #[test]
    fn test_round_trip_conversion() {
        let original_ftl = r#"# Welcome message
hello = Hello World

# Personalized greeting
greeting = Hello, {$name}!

# Item counter with pluralization
items = {$count ->
    [0] No items
    [one] One item
   *[other] {$count} items
}

# Button with attributes
save-button = Save
    .tooltip = Save your changes

# Multiline text value
multiline = This is line one
    This is line two
"#;

        let resource = FluentResource::from_source(original_ftl).unwrap();
        let generated_ftl = resource.to_source();

        // Parse the generated FTL back to ensure consistency
        let resource2 = FluentResource::from_source(&generated_ftl).unwrap();

        // Check that we have the same number of messages
        assert_eq!(resource.messages.len(), resource2.messages.len());

        // Verify content preservation
        let expected_content = [
            "hello = Hello World",
            "greeting = Hello, {$name}!",
            "items = {$count ->",
            "[0] No items",
            "*[other] {$count} items",
            "save-button = Save",
            ".tooltip = Save your changes",
            "multiline = This is line one\n    This is line two",
        ];

        for content in &expected_content {
            assert!(generated_ftl.contains(content), "Missing: {}", content);
        }

        // Verify comment preservation
        let expected_comments = [
            "# Welcome message",
            "# Personalized greeting",
            "# Item counter with pluralization",
            "# Multiline text value",
        ];

        for comment in &expected_comments {
            assert!(generated_ftl.contains(comment), "Missing comment: {}", comment);
        }
    }

    #[test]
    fn test_extract_pattern_text() {
        // Simple text
        let pattern = create_text_pattern("Hello World");
        assert_eq!(extract_pattern_text(&pattern), "Hello World");
        
        // Text with variable
        let pattern = FluentPattern {
            elements: vec![
                FluentElement::Text("Hello, ".to_string()),
                FluentElement::Variable("name".to_string()),
                FluentElement::Text("!".to_string()),
            ],
        };
        assert_eq!(extract_pattern_text(&pattern), "Hello, {$name}!");
        
        // Plural (simplified representation)
        let pattern = FluentPattern {
            elements: vec![FluentElement::Plural {
                selector: "count".to_string(),
                variants: HashMap::new(),
            }],
        };
        assert_eq!(extract_pattern_text(&pattern), "{ $count }");
    }

    #[test]
    fn test_parse_malformed_plurals() {
        let ftl = "bad-plural = {$count -> [one] item";
        let result = FluentResource::from_source(ftl);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_fluent_source() {
        let resource = FluentResource {
            messages: vec![
                FluentMessage {
                    id: "hello".to_string(),
                    value: Some(create_text_pattern("Hello World")),
                    attributes: HashMap::new(),
                    comment: None,
                },
                FluentMessage {
                    id: "greeting".to_string(),
                    value: Some(FluentPattern {
                        elements: vec![
                            FluentElement::Text("Hello, ".to_string()),
                            FluentElement::Variable("name".to_string()),
                            FluentElement::Text("!".to_string()),
                        ],
                    }),
                    attributes: HashMap::new(),
                    comment: None,
                },
            ],
        };
        
        let source = resource.to_source();
        assert!(source.contains("hello = Hello World"));
        assert!(source.contains("greeting = Hello, {$name}!"));
    }

    #[test]
    fn test_parse_invalid_fluent() {
        let ftl = "invalid syntax {{{ ";
        let result = FluentResource::from_source(ftl);
        assert!(result.is_err());
    }

    #[test]
    fn test_variant_key_conversion() {
        // Test that numeric values are preserved as-is for round-trip conversion
        let numeric_key = fluent_syntax::ast::VariantKey::NumberLiteral { value: "1" };
        assert_eq!(variant_key_to_string(&numeric_key), "1");
        
        let identifier_key = fluent_syntax::ast::VariantKey::Identifier { name: "few" };
        assert_eq!(variant_key_to_string(&identifier_key), "few");
    }

    #[test]
    fn test_parse_message_with_attributes_only() {
        let ftl = r#"
just-attrs =
    .label = A message with attributes but no value
    .accesskey = M
"#;
        let resource = FluentResource::from_source(ftl).unwrap();
        assert_eq!(resource.messages.len(), 1);
        let msg = &resource.messages[0];
        assert_eq!(msg.id, "just-attrs");
        assert!(msg.value.is_none());
        assert_eq!(msg.attributes.len(), 2);
        assert!(msg.attributes.contains_key("label"));

        let label_pattern = msg.attributes.get("label").unwrap();
        assert_eq!(
            extract_pattern_text(label_pattern),
            "A message with attributes but no value"
        );
    }

    #[test]
    fn test_parse_comment_on_message_with_attributes_only() {
        let ftl = r#"
# This is a message with attributes only
just-attrs =
    .label = A message with attributes but no value
"#;
        let resource = FluentResource::from_source(ftl).unwrap();
        assert_eq!(resource.messages.len(), 1);
        let msg = &resource.messages[0];
        assert_eq!(msg.id, "just-attrs");
        assert!(msg.value.is_none());
        assert!(msg.comment.is_some());
        assert_eq!(
            msg.comment.as_deref(),
            Some("This is a message with attributes only")
        );
    }

    #[test]
    fn test_multiline_text_formatting_round_trip() {
        let ftl = r#"multiline = This is line one
    This is line two
    And this is line three"#;

        let resource = FluentResource::from_source(ftl).unwrap();
        assert_eq!(resource.messages.len(), 1);

        let generated = resource.to_source();
        let expected = "multiline = This is line one\n    This is line two\n    And this is line three\n";

        assert_eq!(generated.trim(), expected.trim());
    }

    #[test]
    fn test_comment_association_logic() {
        // Test that comments are only associated when directly attached (no empty lines)
        let ftl_attached = r#"# This comment IS associated with hello
hello = Hello"#;
        let resource_attached = FluentResource::from_source(ftl_attached).unwrap();
        assert_eq!(resource_attached.messages.len(), 1);
        assert_eq!(
            resource_attached.messages[0].comment.as_deref(),
            Some("This comment IS associated with hello")
        );

        // Test that comments separated by empty lines are NOT associated
        let ftl_separated = r#"# This comment is NOT associated with hello

hello = Hello"#;
        let resource_separated = FluentResource::from_source(ftl_separated).unwrap();
        assert_eq!(resource_separated.messages.len(), 1);
        assert!(resource_separated.messages[0].comment.is_none());

        // Test multiple standalone comments are all ignored
        let ftl_multiple = r#"# Standalone comment 1

# Standalone comment 2

hello = Hello"#;
        let resource_multiple = FluentResource::from_source(ftl_multiple).unwrap();
        assert_eq!(resource_multiple.messages.len(), 1);
        assert!(resource_multiple.messages[0].comment.is_none());
    }

    #[test]
    fn test_parse_value_with_leading_trailing_whitespace() {
        // Fluent syntax trims leading and trailing whitespace from a value on a single line.
        let ftl = "whitespace-message =    Hello World   ";
        let resource = FluentResource::from_source(ftl).unwrap();

        assert_eq!(resource.messages.len(), 1);
        let message = &resource.messages[0];
        assert_eq!(message.id, "whitespace-message");

        assert!(message.value.is_some());
        let pattern = message.value.as_ref().unwrap();
        assert_eq!(pattern.elements.len(), 1);
        assert_pattern_text(pattern, "Hello World");
    }
}
