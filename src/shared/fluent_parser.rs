use anyhow::Result;
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use fluent_syntax::parser::parse;
use std::collections::HashMap;

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
    pub fn from_source(source: &str) -> Result<Self> {
        let resource = match parse(source) {
            Ok(resource) => resource,
            Err((resource, errors)) => {
                if !errors.is_empty() {
                    return Err(anyhow::anyhow!("Parse errors: {:?}", errors));
                }
                resource
            }
        };
        
        // Extract comments from source manually
        let comments_map = extract_comments_from_source(source);
        
        let mut messages = Vec::new();

        for entry in resource.body {
            match entry {
                Entry::Message(message) => {
                    let message_id = message.id.name.to_string();
                    let comment = comments_map.get(&message_id).cloned();
                    
                    let fluent_message = FluentMessage {
                        id: message_id,
                        value: message.value.map(|pattern| convert_pattern(&pattern)),
                        attributes: message
                            .attributes
                            .into_iter()
                            .map(|attr| (attr.id.name.to_string(), convert_pattern(&attr.value)))
                            .collect(),
                        comment,
                    };
                    messages.push(fluent_message);
                }
                _ => {
                    // Handle other entry types if needed
                }
            }
        }

        Ok(FluentResource { messages })
    }

    pub fn to_source(&self) -> String {
        let mut output = String::new();

        for (i, message) in self.messages.iter().enumerate() {
            // Add blank line before each message except the first
            if i > 0 {
                output.push('\n');
            }
            
            // Write comment if present
            if let Some(comment) = &message.comment {
                output.push_str(&format!("# {}\n", comment));
            }
            
            output.push_str(&message.id);
            
            if let Some(value) = &message.value {
                output.push_str(" = ");
                output.push_str(&pattern_to_string(value));
            }
            
            output.push('\n');

            for (attr_name, attr_value) in &message.attributes {
                output.push_str(&format!("    .{} = {}\n", attr_name, pattern_to_string(attr_value)));
            }
            
            if !message.attributes.is_empty() {
                output.push('\n');
            }
        }

        output
    }
}

/// Extract comments from Fluent source and associate them with message IDs
fn extract_comments_from_source(source: &str) -> HashMap<String, String> {
    let mut comments_map = HashMap::new();
    let lines: Vec<&str> = source.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        
        // Check if this line is a comment
        if trimmed.starts_with('#') && !trimmed.is_empty() {
            let comment_text = trimmed.trim_start_matches('#').trim();
            
            // Look for the next non-empty, non-comment line to find the message ID
            for j in (i + 1)..lines.len() {
                let next_line = lines[j].trim();
                if next_line.is_empty() || next_line.starts_with('#') {
                    continue;
                }
                
                // Check if this line contains a message definition
                if let Some(equals_pos) = next_line.find('=') {
                    let message_id = next_line[..equals_pos].trim();
                    if !message_id.is_empty() && message_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                        comments_map.insert(message_id.to_string(), comment_text.to_string());
                    }
                }
                break;
            }
        }
    }
    
    comments_map
}

fn convert_pattern(pattern: &Pattern<&str>) -> FluentPattern {
    let mut elements = Vec::new();

    for element in &pattern.elements {
        match element {
            PatternElement::TextElement { value } => {
                elements.push(FluentElement::Text(value.to_string()));
            }
            PatternElement::Placeable { expression } => {
                match expression {
                    Expression::Inline(InlineExpression::VariableReference { id }) => {
                        elements.push(FluentElement::Variable(id.name.to_string()));
                    }
                    Expression::Select { selector, variants } => {
                        if let InlineExpression::VariableReference { id } = selector {
                            let selector_name = id.name.to_string();
                            let mut variant_map = HashMap::new();

                            for variant in variants {
                                let key = variant_key_to_string(&variant.key);
                                let pattern = convert_pattern(&variant.value);
                                variant_map.insert(key, pattern);
                            }

                            elements.push(FluentElement::Plural {
                                selector: selector_name,
                                variants: variant_map,
                            });
                        }
                    }
                    _ => {
                        // For unsupported expressions, we'll just add a placeholder
                        elements.push(FluentElement::Text("{unsupported}".to_string()));
                    }
                }
            }
        }
    }

    FluentPattern { elements }
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
    let mut result = String::new();

    for element in &pattern.elements {
        match element {
            FluentElement::Text(text) => {
                // Handle multiline text with proper indentation
                if text.contains('\n') {
                    let lines: Vec<&str> = text.split('\n').collect();
                    result.push_str(lines[0]); // First line without indentation
                    for line in &lines[1..] {
                        result.push_str("\n    "); // Indent continuation lines
                        result.push_str(line);
                    }
                } else {
                    result.push_str(text);
                }
            },
            FluentElement::Variable(var) => result.push_str(&format!("{{${}}}", var)),
            FluentElement::Plural { selector, variants } => {
                result.push_str(&format!("{{${} ->\n", selector));
                
                // Output variants in a specific order, but skip 'other' for now
                let order = ["zero", "one", "two", "few", "many"];
                for key in &order {
                    if let Some(variant_pattern) = variants.get(*key) {
                        result.push_str(&format!("    [{}] {}\n", key, pattern_to_string(variant_pattern)));
                    }
                }
                
                // Add any remaining variants not in the standard order (except 'other')
                for (key, variant_pattern) in variants {
                    if !order.contains(&key.as_str()) && key != "other" {
                        result.push_str(&format!("    [{}] {}\n", key, pattern_to_string(variant_pattern)));
                    }
                }
                
                // Always add 'other' at the end with * as the default
                if let Some(other_pattern) = variants.get("other") {
                    result.push_str(&format!("   *[other] {}\n", pattern_to_string(other_pattern)));
                } else {
                    result.push_str("   *[other] (missing)\n");
                }
                
                result.push_str("}");
            }
        }
    }

    result
}

/// Extract plain text from a FluentPattern for use in PO conversion
pub fn extract_pattern_text(pattern: &FluentPattern) -> String {
    let mut result = String::new();
    
    for element in &pattern.elements {
        match element {
            FluentElement::Text(text) => {
                result.push_str(text);
            }
            FluentElement::Variable(var) => {
                result.push_str(&format!("{{${}}}", var));
            }
            FluentElement::Plural { selector, .. } => {
                // For plurals, we'll include the selector variable for now
                // This is a simplified approach - a full implementation would
                // need more sophisticated handling
                result.push_str(&format!("{{ ${} }}", selector));
            }
        }
    }
    
    result
}

/// Simple function to parse Fluent content using the comprehensive parser
pub fn parse_fluent(content: &str) -> Result<FluentResource> {
    FluentResource::from_source(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_message() {
        let ftl = "hello = Hello World";
        let resource = FluentResource::from_source(ftl).unwrap();
        
        assert_eq!(resource.messages.len(), 1);
        assert_eq!(resource.messages[0].id, "hello");
        assert!(resource.messages[0].value.is_some());
        
        let pattern = resource.messages[0].value.as_ref().unwrap();
        assert_eq!(pattern.elements.len(), 1);
        
        if let FluentElement::Text(text) = &pattern.elements[0] {
            assert_eq!(text, "Hello World");
        } else {
            panic!("Expected text element");
        }
    }

    #[test]
    fn test_parse_message_with_variable() {
        let ftl = "greeting = Hello, {$name}!";
        let resource = FluentResource::from_source(ftl).unwrap();
        
        assert_eq!(resource.messages.len(), 1);
        let pattern = resource.messages[0].value.as_ref().unwrap();
        assert_eq!(pattern.elements.len(), 3);
        
        if let FluentElement::Text(text) = &pattern.elements[0] {
            assert_eq!(text, "Hello, ");
        } else {
            panic!("Expected text element");
        }
        
        if let FluentElement::Variable(var) = &pattern.elements[1] {
            assert_eq!(var, "name");
        } else {
            panic!("Expected variable element");
        }
        
        if let FluentElement::Text(text) = &pattern.elements[2] {
            assert_eq!(text, "!");
        } else {
            panic!("Expected text element");
        }
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
            assert!(variants.contains_key("one"));
            assert!(variants.contains_key("other"));
        } else {
            panic!("Expected plural element");
        }
    }

    #[test]
    fn test_generate_fluent_source() {
        let mut resource = FluentResource { messages: Vec::new() };
        
        // Simple message
        resource.messages.push(FluentMessage {
            id: "hello".to_string(),
            value: Some(FluentPattern {
                elements: vec![FluentElement::Text("Hello World".to_string())],
            }),
            attributes: HashMap::new(),
            comment: None,
        });
        
        // Message with variable
        resource.messages.push(FluentMessage {
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
        });
        
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
        let key = fluent_syntax::ast::VariantKey::NumberLiteral { value: "1" };
        assert_eq!(variant_key_to_string(&key), "1");
        
        let key = fluent_syntax::ast::VariantKey::NumberLiteral { value: "2" };
        assert_eq!(variant_key_to_string(&key), "2");
        
        let key = fluent_syntax::ast::VariantKey::Identifier { name: "few" };
        assert_eq!(variant_key_to_string(&key), "few");
    }
}
