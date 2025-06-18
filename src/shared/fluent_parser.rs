use anyhow::Result;
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use fluent_syntax::parser::parse;
use fluent_syntax::serializer;
use std::collections::HashMap;

// Type aliases for better readability of AST types
type AstMessage = fluent_syntax::ast::Message<String>;
type AstPattern = fluent_syntax::ast::Pattern<String>;
type AstPatternElement = fluent_syntax::ast::PatternElement<String>;
type AstVariant = fluent_syntax::ast::Variant<String>;
type AstIdentifier = fluent_syntax::ast::Identifier<String>;

// Constants for better maintainability
const UNSUPPORTED_PLACEHOLDER: &str = "{unsupported}";

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
        // Convert our custom structures back to fluent_syntax AST
        let ast_resource = self.to_fluent_syntax_ast();
        
        // Use the built-in serializer - this is more reliable and handles all edge cases
        serializer::serialize(&ast_resource)
    }

    /// Convert our custom FluentResource back to fluent_syntax AST for serialization
    fn to_fluent_syntax_ast(&self) -> fluent_syntax::ast::Resource<String> {
        let entries: Vec<fluent_syntax::ast::Entry<String>> = self.messages
            .iter()
            .map(|message| fluent_syntax::ast::Entry::Message(self.convert_message_to_ast(message)))
            .collect();

        fluent_syntax::ast::Resource { body: entries }
    }

    /// Convert a FluentMessage to fluent_syntax AST Message
    fn convert_message_to_ast(&self, message: &FluentMessage) -> AstMessage {
        let id = Self::create_identifier(&message.id);
        let value = message.value.as_ref().map(|pattern| self.convert_pattern_to_ast(pattern));
        
        let attributes: Vec<fluent_syntax::ast::Attribute<String>> = message.attributes
            .iter()
            .map(|(attr_name, attr_pattern)| fluent_syntax::ast::Attribute {
                id: Self::create_identifier(attr_name),
                value: self.convert_pattern_to_ast(attr_pattern),
            })
            .collect();

        let comment = message.comment.as_ref().map(|comment_text| {
            fluent_syntax::ast::Comment {
                content: comment_text.lines().map(|line| line.to_string()).collect(),
            }
        });

        fluent_syntax::ast::Message {
            id,
            value,
            attributes,
            comment,
        }
    }

    /// Convert a FluentPattern to fluent_syntax AST Pattern
    fn convert_pattern_to_ast(&self, pattern: &FluentPattern) -> AstPattern {
        let elements: Vec<AstPatternElement> = pattern.elements
            .iter()
            .map(|element| self.convert_element_to_ast(element))
            .collect();

        fluent_syntax::ast::Pattern { elements }
    }

    /// Convert a FluentElement to fluent_syntax AST PatternElement
    fn convert_element_to_ast(&self, element: &FluentElement) -> AstPatternElement {
        match element {
            FluentElement::Text(text) => {
                fluent_syntax::ast::PatternElement::TextElement {
                    value: text.clone(),
                }
            }
            FluentElement::Variable(var_name) => {
                self.create_variable_placeable(var_name)
            }
            FluentElement::Plural { selector, variants } => {
                self.create_plural_placeable(selector, variants)
            }
        }
    }

    fn create_variable_placeable(&self, var_name: &str) -> AstPatternElement {
        fluent_syntax::ast::PatternElement::Placeable {
            expression: fluent_syntax::ast::Expression::Inline(
                Self::create_variable_reference(var_name)
            ),
        }
    }

    fn create_plural_placeable(&self, selector: &str, variants: &HashMap<String, FluentPattern>) -> AstPatternElement {
        let selector_expr = Self::create_variable_reference(selector);
        let ast_variants = self.convert_variants_to_ast(variants);

        fluent_syntax::ast::PatternElement::Placeable {
            expression: fluent_syntax::ast::Expression::Select {
                selector: selector_expr,
                variants: ast_variants,
            },
        }
    }

    fn convert_variants_to_ast(&self, variants: &HashMap<String, FluentPattern>) -> Vec<AstVariant> {
        variants
            .iter()
            .map(|(key, pattern)| {
                fluent_syntax::ast::Variant {
                    key: Self::create_variant_key(key),
                    value: self.convert_pattern_to_ast(pattern),
                    default: key == "other",
                }
            })
            .collect()
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

    // Helper functions for creating common AST nodes
    fn create_identifier(name: &str) -> AstIdentifier {
        fluent_syntax::ast::Identifier {
            name: name.to_string(),
        }
    }

    fn create_variable_reference(var_name: &str) -> fluent_syntax::ast::InlineExpression<String> {
        fluent_syntax::ast::InlineExpression::VariableReference {
            id: Self::create_identifier(var_name),
        }
    }

    fn create_variant_key(key: &str) -> fluent_syntax::ast::VariantKey<String> {
        if key.parse::<i32>().is_ok() {
            fluent_syntax::ast::VariantKey::NumberLiteral {
                value: key.to_string(),
            }
        } else {
            fluent_syntax::ast::VariantKey::Identifier {
                name: key.to_string(),
            }
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

        // Verify content preservation (accounting for normalization by built-in serializer)
        let expected_content = [
            "hello = Hello World",
            "greeting = Hello, { $name }!", // Normalized variable formatting
            "items =", // Multiline formatting for plurals
            "{ $count ->", // Selector on separate line
            "[0] No items",
            "*[other] { $count } items", // Normalized variable formatting
            "save-button = Save",
            ".tooltip = Save your changes",
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
        // Built-in serializer normalizes variable formatting to include spaces
        assert!(source.contains("greeting = Hello, { $name }!"));
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
        
        // The built-in serializer formats multiline messages with proper indentation
        // Verify that the content is preserved correctly
        assert!(generated.contains("multiline ="));
        assert!(generated.contains("This is line one"));
        assert!(generated.contains("This is line two"));
        assert!(generated.contains("And this is line three"));
        
        // Parse it back to ensure round-trip works semantically
        let reparsed = FluentResource::from_source(&generated).unwrap();
        assert_eq!(reparsed.messages.len(), 1);
        assert_eq!(reparsed.messages[0].id, "multiline");
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

    #[test]
    fn test_builtin_serializer_integration() {
        // Test that our new implementation using the built-in serializer works correctly
        let original_ftl = r#"# Welcome message
hello = Hello World

# Personalized greeting
greeting = Hello, {$name}!

# Item counter with pluralization
items = {$count ->
    [0] No items
    [one] One item
   *[other] {$count} items
}"#;

        let resource = FluentResource::from_source(original_ftl).unwrap();
        
        // Our to_source now uses the built-in serializer
        let output = resource.to_source();
        
        // Should parse back to equivalent resource
        let reparsed = FluentResource::from_source(&output).unwrap();
        
        assert_eq!(resource.messages.len(), reparsed.messages.len());
        
        // Verify that content is preserved
        for (original_msg, reparsed_msg) in resource.messages.iter().zip(reparsed.messages.iter()) {
            assert_eq!(original_msg.id, reparsed_msg.id);
            assert_eq!(original_msg.comment, reparsed_msg.comment);
            // Note: The built-in serializer may normalize formatting, but semantic content should be the same
        }
    }
}
