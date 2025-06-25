use fluent_syntax::serializer;
use std::collections::HashMap;
use crate::shared::fluent_data::{FluentMessage, FluentPattern, FluentElement, FluentResource};

// Type aliases for better readability of AST types
type AstMessage = fluent_syntax::ast::Message<String>;
type AstPattern = fluent_syntax::ast::Pattern<String>;
type AstPatternElement = fluent_syntax::ast::PatternElement<String>;
type AstVariant = fluent_syntax::ast::Variant<String>;
type AstIdentifier = fluent_syntax::ast::Identifier<String>;

pub struct FluentResourceWriter {
}

impl FluentResourceWriter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn write_resource(&self, resource: &FluentResource) -> String {
        let mut output = String::new();

        if resource.messages.is_empty() {
            return output;
        }
        
        // At this point we could serialize the entire structure with the library, but that will generate a
        // Fluent file without empty lines between strings
        for (i, message) in resource.messages.iter().enumerate() {
            // Add spacing before each message (except the first)
            if i > 0 {
                output.push('\n');
            }
            
            // Generate each message individually using the built-in serializer
            let temp_resource = FluentResource {
                messages: vec![message.clone()],
            };

            let ast_resource = self.to_fluent_syntax_ast(&temp_resource);
            let serialized = serializer::serialize(&ast_resource);
            output.push_str(&serialized);
        }
        
        output
    }

    /// Convert our custom FluentResource back to fluent_syntax AST for serialization
    fn to_fluent_syntax_ast(&self, resource: &FluentResource) -> fluent_syntax::ast::Resource<String> {
        let entries: Vec<fluent_syntax::ast::Entry<String>> = resource.messages
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