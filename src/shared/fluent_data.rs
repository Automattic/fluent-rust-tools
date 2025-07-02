use crate::shared::{
    fluent_resource_parser::FluentResourceParser, fluent_resource_writer::FluentResourceWriter,
};
use anyhow::{anyhow, Result};
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use std::collections::HashMap;

use super::fluent_resource_parser::UNSUPPORTED_PLACEHOLDER;

#[derive(Debug, Clone)]
pub struct FluentMessage {
    pub id: String,
    pub value: Option<FluentPattern>,
    pub attributes: HashMap<String, FluentPattern>,
    pub comment: Option<String>,
}

impl TryFrom<Entry<&str>> for FluentMessage {
    type Error = anyhow::Error;

    fn try_from(entry: Entry<&str>) -> Result<Self> {
        match entry {
            Entry::Message(message) => Ok(message.into()),
            Entry::Comment(_) => Err(anyhow!(
                "Standalone comments are ignored - only use parser's built-in comment association"
            )),
            Entry::GroupComment(_) | Entry::ResourceComment(_) => {
                Err(anyhow!("Ignore group and resource comments for now"))
            }
            Entry::Term(_) => Err(anyhow!("Handle terms if needed in the future")),
            Entry::Junk { .. } => Err(anyhow!("Ignore junk entries")),
        }
    }
}

impl From<fluent_syntax::ast::Message<&str>> for FluentMessage {
    fn from(message: fluent_syntax::ast::Message<&str>) -> Self {
        let message_id = message.id.name.to_string();

        // Only use comments directly associated with the message by the fluent-syntax parser
        let comment = message
            .comment
            .map(|msg_comment| msg_comment.content.join("\n"));

        Self {
            id: message_id,
            value: message.value.as_ref().map(Into::into),
            attributes: FluentResourceParser::convert_attributes(message.attributes),
            comment,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FluentPattern {
    pub elements: Vec<FluentElement>,
}

impl From<&Pattern<&str>> for FluentPattern {
    fn from(pattern: &Pattern<&str>) -> Self {
        Self {
            elements: pattern.elements.iter().map(Into::into).collect(),
        }
    }
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

impl From<&Expression<&str>> for FluentElement {
    fn from(expression: &Expression<&str>) -> Self {
        match expression {
            Expression::Inline(InlineExpression::VariableReference { id }) => {
                FluentElement::Variable(id.name.to_string())
            }
            Expression::Select { selector, variants } => {
                FluentResourceParser::convert_select_expression(selector, variants)
            }
            _ => FluentElement::Text(UNSUPPORTED_PLACEHOLDER.to_string()),
        }
    }
}

impl From<&PatternElement<&str>> for FluentElement {
    fn from(element: &PatternElement<&str>) -> Self {
        match element {
            PatternElement::TextElement { value } => FluentElement::Text(value.to_string()),
            PatternElement::Placeable { expression } => expression.into(),
        }
    }
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
        FluentResourceParser::parse_source(source)
    }

    pub fn to_source(&self) -> String {
        let writer = FluentResourceWriter::new();
        writer.write_resource(self)
    }
}

/// Extract plain text from a FluentPattern for use in PO conversion
pub fn extract_pattern_text(pattern: &FluentPattern) -> String {
    pattern
        .elements
        .iter()
        .map(extract_element_text)
        .collect::<Vec<_>>()
        .join("")
}

fn extract_element_text(element: &FluentElement) -> String {
    match element {
        FluentElement::Text(text) => text.clone(),
        FluentElement::Variable(var) => format!("{{${}}}", var),
        FluentElement::Plural { selector, .. } => format!("{{ ${} }}", selector),
    }
}

/// Helper function to parse text content as a Fluent pattern with proper multiline formatting
///
/// This function takes raw text and formats it properly for Fluent syntax by:
/// - Adding proper indentation to multiline text (4 spaces for continuation lines)
/// - Parsing the formatted text through the Fluent parser to ensure valid syntax
/// - Returning a FluentPattern that can be used in message values
pub fn parse_string_value_as_fluent_pattern(key: &str, text: &str) -> FluentPattern {
    let formatted_text = format_string_value_as_multiline_fluent_text(text);
    let fluent_content = format!("{} = {}", key, formatted_text);
    match FluentResource::from_source(&fluent_content) {
        Ok(resource) => resource
            .messages
            .first()
            .and_then(|message| message.value.clone())
            .unwrap_or_else(|| FluentPattern {
                elements: vec![FluentElement::Text(text.to_string())],
            }),
        Err(_) => FluentPattern {
            elements: vec![FluentElement::Text(text.to_string())],
        },
    }
}

pub fn format_string_value_as_multiline_fluent_text(text: &str) -> String {
    if text.contains('\n') {
        text.lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    line.to_string()
                } else {
                    format!("    {}", line) // Indent continuation lines with 4 spaces
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        text.to_string()
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
    fn test_parse_string_value_as_fluent_pattern() {
        // Test simple single-line text
        let pattern = parse_string_value_as_fluent_pattern("test", "Hello World");
        assert_eq!(pattern.elements.len(), 1);
        if let FluentElement::Text(text) = &pattern.elements[0] {
            assert_eq!(text, "Hello World");
        } else {
            panic!("Expected text element");
        }

        // Test multiline text
        let multiline_text = "This is line one\nThis is line two\nThis is line three";
        let pattern = parse_string_value_as_fluent_pattern("test", multiline_text);

        // Should have 3 text elements (one per line) - this is the correct internal representation
        assert_eq!(pattern.elements.len(), 3);

        if let FluentElement::Text(text) = &pattern.elements[0] {
            assert_eq!(text, "This is line one\n");
        } else {
            panic!("Expected text element");
        }

        if let FluentElement::Text(text) = &pattern.elements[1] {
            assert_eq!(text, "This is line two\n");
        } else {
            panic!("Expected text element");
        }

        if let FluentElement::Text(text) = &pattern.elements[2] {
            assert_eq!(text, "This is line three");
        } else {
            panic!("Expected text element");
        }

        // Test that when this pattern is converted to Fluent source, it has proper indentation
        let temp_resource = FluentResource {
            messages: vec![FluentMessage {
                id: "test-multiline".to_string(),
                value: Some(pattern),
                attributes: HashMap::new(),
                comment: None,
            }],
        };

        let fluent_content = temp_resource.to_source();

        // The generated Fluent should have proper indentation
        assert!(fluent_content.contains("test-multiline ="));
        assert!(fluent_content.contains("This is line one"));
        assert!(fluent_content.contains("    This is line two"));
        assert!(fluent_content.contains("    This is line three"));
    }
}
