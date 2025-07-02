use crate::shared::fluent_data::{FluentElement, FluentMessage, FluentPattern, FluentResource};
use anyhow::Result;
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use std::collections::HashMap;

// Constants for better maintainability
const UNSUPPORTED_PLACEHOLDER: &str = "{unsupported}";

/// Internal parser state for processing Fluent AST entries
pub struct FluentResourceParser;

impl FluentResourceParser {
    pub fn parse_source(source: &str) -> Result<FluentResource> {
        let resource = Self::parse_with_error_handling(source)?;

        Ok(FluentResource {
            messages: Self::process_entries(resource.body),
        })
    }

    fn process_entries(entries: Vec<Entry<&str>>) -> Vec<FluentMessage> {
        entries
            .into_iter()
            .map(TryFrom::try_from)
            .flat_map(Result::ok)
            .collect()
    }

    pub fn process_message(message: fluent_syntax::ast::Message<&str>) -> FluentMessage {
        let message_id = message.id.name.to_string();

        // Only use comments directly associated with the message by the fluent-syntax parser
        let comment = message
            .comment
            .map(|msg_comment| msg_comment.content.join("\n"));

        FluentMessage {
            id: message_id,
            value: message.value.map(|pattern| Self::convert_pattern(&pattern)),
            attributes: Self::convert_attributes(message.attributes),
            comment,
        }
    }

    pub fn convert_attributes(
        attributes: Vec<fluent_syntax::ast::Attribute<&str>>,
    ) -> HashMap<String, FluentPattern> {
        attributes
            .into_iter()
            .map(|attr| (attr.id.name.to_string(), Self::convert_pattern(&attr.value)))
            .collect()
    }

    pub fn convert_pattern(pattern: &Pattern<&str>) -> FluentPattern {
        let elements = pattern
            .elements
            .iter()
            .map(Self::convert_pattern_element)
            .collect();

        FluentPattern { elements }
    }

    fn parse_with_error_handling(source: &str) -> Result<fluent_syntax::ast::Resource<&str>> {
        match fluent_syntax::parser::parse(source) {
            Ok(resource) => Ok(resource),
            Err((_resource, errors)) => Err(anyhow::anyhow!("Fluent parse errors: {:#?}", errors)),
        }
    }

    fn convert_pattern_element(element: &PatternElement<&str>) -> FluentElement {
        match element {
            PatternElement::TextElement { value } => FluentElement::Text(value.to_string()),
            PatternElement::Placeable { expression } => Self::convert_expression(expression),
        }
    }

    fn convert_expression(expression: &Expression<&str>) -> FluentElement {
        match expression {
            Expression::Inline(InlineExpression::VariableReference { id }) => {
                FluentElement::Variable(id.name.to_string())
            }
            Expression::Select { selector, variants } => {
                Self::convert_select_expression(selector, variants)
            }
            _ => FluentElement::Text(UNSUPPORTED_PLACEHOLDER.to_string()),
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
                    let key = Self::variant_key_to_string(&variant.key);
                    let pattern = Self::convert_pattern(&variant.value);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::fluent_data::extract_pattern_text;

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
        assert_eq!(
            hello_comment, expected_hello_comment,
            "Comment should contain exact content without # characters or extra indentation"
        );

        // Check second message comment - assert exact content
        assert!(resource.messages[1].comment.is_some());
        let goodbye_comment = resource.messages[1].comment.as_ref().unwrap();
        let expected_goodbye_comment = "Another comment\nfor a different message";
        assert_eq!(
            goodbye_comment, expected_goodbye_comment,
            "Comment should contain exact content without # characters or extra indentation"
        );
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
    fn test_parse_malformed_plurals() {
        let ftl = "bad-plural = {$count -> [one] item";
        let result = FluentResource::from_source(ftl);
        assert!(result.is_err());
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
        assert_eq!(
            FluentResourceParser::variant_key_to_string(&numeric_key),
            "1"
        );

        let identifier_key = fluent_syntax::ast::VariantKey::Identifier { name: "few" };
        assert_eq!(
            FluentResourceParser::variant_key_to_string(&identifier_key),
            "few"
        );
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
