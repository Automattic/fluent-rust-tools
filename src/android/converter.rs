use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::android::android_format::{AndroidResources, AndroidString, AndroidPlural};
use crate::shared::fluent_parser::{FluentResource, FluentMessage, FluentPattern, FluentElement};

// Constants
const COUNT_PLACEHOLDER: &str = "%d";
const DEFAULT_COUNT_VARIABLE: &str = "count";
const ANDROID_PLACEHOLDER_REGEX: &str = r"%(\d*)\$?[sdif]";
const NUMBERED_COUNT_REGEX: &str = r"(%\d+\$d)";

pub fn fluent_to_android(input_path: &Path, output_path: &Path) -> Result<()> {
    let fluent_content = fs::read_to_string(input_path)?;
    let fluent_resource = FluentResource::from_source(&fluent_content)?;
    
    let android_resources = convert_fluent_to_android(&fluent_resource)?;
    let xml_content = android_resources.to_xml()?;
    
    fs::write(output_path, xml_content)?;
    Ok(())
}

pub fn android_to_fluent(input_path: &Path, output_path: &Path) -> Result<()> {
    let xml_content = fs::read_to_string(input_path)?;
    let android_resources = AndroidResources::from_xml(&xml_content)?;
    
    let conversion_context = ConversionContext::default();
    let fluent_resource = convert_android_to_fluent(&android_resources, &conversion_context)?;
    let fluent_content = fluent_resource.to_source();
    
    fs::write(output_path, fluent_content)?;
    Ok(())
}

pub fn android_to_fluent_with_original(
    xml_input_path: &Path, 
    fluent_output_path: &Path, 
    original_fluent_path: &Path
) -> Result<()> {
    let xml_content = fs::read_to_string(xml_input_path)?;
    let android_resources = AndroidResources::from_xml(&xml_content)?;
    
    // Parse the original Fluent file to extract variable mappings and comments
    let original_fluent_content = fs::read_to_string(original_fluent_path)?;
    let original_fluent_resource = FluentResource::from_source(&original_fluent_content)?;
    
    let conversion_context = ConversionContext::from_original_fluent(&original_fluent_resource)?;
    let fluent_resource = convert_android_to_fluent(&android_resources, &conversion_context)?;
    let fluent_content = fluent_resource.to_source();
    
    fs::write(fluent_output_path, fluent_content)?;
    Ok(())
}

#[derive(Debug, Default)]
struct ConversionContext {
    plural_selectors: HashMap<String, String>, // message_id -> selector_variable_name
    string_variables: HashMap<String, Vec<String>>, // message_id -> list of variable names
    original_comments: HashMap<String, String>, // message_id -> comment
}

impl ConversionContext {
    fn from_original_fluent(fluent: &FluentResource) -> Result<Self> {
        let mut plural_selectors = HashMap::new();
        let mut string_variables = HashMap::new();
        let mut original_comments = HashMap::new();
        
        for message in &fluent.messages {
            // Store original comments
            if let Some(comment) = &message.comment {
                original_comments.insert(message.id.clone(), comment.clone());
            }
            
            if let Some(value) = &message.value {
                match classify_pattern(value) {
                    PatternType::Plural => {
                        // Find the selector variable in plural patterns
                        for element in &value.elements {
                            if let FluentElement::Plural { selector, .. } = element {
                                plural_selectors.insert(message.id.clone(), selector.clone());
                                break;
                            }
                        }
                    }
                    PatternType::Simple => {
                        // Extract variable names from simple patterns
                        let vars: Vec<String> = value.elements
                            .iter()
                            .filter_map(|element| {
                                if let FluentElement::Variable(var_name) = element {
                                    Some(var_name.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        
                        if !vars.is_empty() {
                            string_variables.insert(message.id.clone(), vars);
                        }
                    }
                }
            }
        }
        
        Ok(Self {
            plural_selectors,
            string_variables,
            original_comments,
        })
    }
    
    fn get_original_comment(&self, id: &str) -> Option<&String> {
        self.original_comments.get(id)
    }
    
    fn get_string_variables(&self, id: &str) -> Option<&Vec<String>> {
        self.string_variables.get(id)
    }
    
    fn get_plural_selector(&self, id: &str) -> Option<&String> {
        self.plural_selectors.get(id)
    }
}

#[derive(Debug)]
enum PatternType {
    Simple,
    Plural,
}

fn classify_pattern(pattern: &FluentPattern) -> PatternType {
    pattern.elements
        .iter()
        .find_map(|element| {
            if matches!(element, FluentElement::Plural { .. }) {
                Some(PatternType::Plural)
            } else {
                None
            }
        })
        .unwrap_or(PatternType::Simple)
}

fn convert_fluent_to_android(fluent: &FluentResource) -> Result<AndroidResources> {
    let mut android_resources = AndroidResources::new();

    for message in &fluent.messages {
        if let Some(value) = &message.value {
            match classify_pattern(value) {
                PatternType::Simple => {
                    let android_string = convert_simple_pattern_to_android(message, value)?;
                    android_resources.strings.push(android_string);
                }
                PatternType::Plural => {
                    let android_plural = convert_plural_pattern_to_android(message, value)?;
                    android_resources.plurals.push(android_plural);
                }
            }
        }
    }

    Ok(android_resources)
}

fn convert_android_to_fluent(
    android: &AndroidResources, 
    context: &ConversionContext
) -> Result<FluentResource> {
    let mut fluent_messages = Vec::new();

    // Convert simple strings
    for string in &android.strings {
        let fluent_message = convert_android_string_to_fluent(string, context)?;
        fluent_messages.push(fluent_message);
    }

    // Convert plurals
    for plural in &android.plurals {
        let fluent_message = convert_android_plural_to_fluent(plural, context)?;
        fluent_messages.push(fluent_message);
    }

    Ok(FluentResource {
        messages: fluent_messages,
    })
}

fn convert_simple_pattern_to_android(
    message: &FluentMessage,
    pattern: &FluentPattern,
) -> Result<AndroidString> {
    let mut android_value = String::new();
    let mut variable_mapping = HashMap::new();
    let mut placeholder_counter = 1;

    for element in &pattern.elements {
        match element {
            FluentElement::Text(text) => {
                android_value.push_str(&escape_android_string(text));
            }
            FluentElement::Variable(var_name) => {
                let placeholder = create_placeholder(placeholder_counter);
                android_value.push_str(&placeholder);
                variable_mapping.insert(placeholder, var_name.clone());
                placeholder_counter += 1;
            }
            FluentElement::Plural { .. } => {
                return Err(anyhow::anyhow!("Unexpected plural in simple pattern"));
            }
        }
    }

    Ok(AndroidString {
        name: message.id.clone(),
        value: android_value,
        translatable: Some(true),
        comment: message.comment.clone(),
        variable_mapping,
    })
}

fn convert_plural_pattern_to_android(
    message: &FluentMessage,
    pattern: &FluentPattern,
) -> Result<AndroidPlural> {
    let plural_element = pattern.elements
        .iter()
        .find_map(|element| {
            if let FluentElement::Plural { selector, variants } = element {
                Some((selector, variants))
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No plural found in pattern"))?;

    let (selector, variants) = plural_element;
    let mut android_items = HashMap::new();
    let mut variable_mapping = HashMap::new();

    for (quantity, variant_pattern) in variants {
        let (android_value, variant_mappings) = convert_pattern_to_android_text(variant_pattern, selector)?;
        android_items.insert(map_fluent_to_android_quantity(quantity), android_value);
        variable_mapping.extend(variant_mappings);
    }

    Ok(AndroidPlural {
        name: message.id.clone(),
        items: android_items,
        comment: message.comment.clone(),
        variable_mapping,
    })
}

fn convert_pattern_to_android_text(
    pattern: &FluentPattern, 
    selector: &str
) -> Result<(String, HashMap<String, String>)> {
    let mut android_value = String::new();
    let mut variable_mapping = HashMap::new();
    let mut placeholder_counter = 1;

    for element in &pattern.elements {
        match element {
            FluentElement::Text(text) => {
                android_value.push_str(&escape_android_string(text));
            }
            FluentElement::Variable(var_name) => {
                let placeholder = if var_name == selector {
                    COUNT_PLACEHOLDER.to_string()
                } else {
                    create_placeholder(placeholder_counter)
                };
                
                android_value.push_str(&placeholder);
                variable_mapping.insert(placeholder, var_name.clone());
                
                if var_name != selector {
                    placeholder_counter += 1;
                }
            }
            FluentElement::Plural { .. } => {
                return Err(anyhow::anyhow!("Nested plurals not supported"));
            }
        }
    }

    Ok((android_value, variable_mapping))
}

fn convert_android_string_to_fluent(
    android_string: &AndroidString, 
    context: &ConversionContext
) -> Result<FluentMessage> {
    let original_variables = context.get_string_variables(&android_string.name);
    let fluent_pattern = convert_android_text_to_fluent_pattern(
        &android_string.value,
        &android_string.variable_mapping,
        original_variables,
    )?;

    let comment = context.get_original_comment(&android_string.name)
        .cloned()
        .or_else(|| android_string.comment.clone());

    Ok(FluentMessage {
        id: android_string.name.clone(),
        value: Some(fluent_pattern),
        attributes: HashMap::new(),
        comment,
    })
}

fn convert_android_plural_to_fluent(
    android_plural: &AndroidPlural, 
    context: &ConversionContext
) -> Result<FluentMessage> {
    let selector = determine_plural_selector(android_plural, context);
    let effective_mapping = create_effective_mapping(android_plural, &selector);
    
    let mut variants = HashMap::new();
    for (quantity, android_text) in &android_plural.items {
        let variant_pattern = convert_android_text_to_fluent_pattern(
            android_text,
            &effective_mapping,
            None, // Plurals don't use original variable lists
        )?;
        let fluent_quantity = map_android_to_fluent_quantity(quantity);
        variants.insert(fluent_quantity, variant_pattern);
    }

    let plural_element = FluentElement::Plural { 
        selector: selector.clone(), 
        variants 
    };
    let pattern = FluentPattern {
        elements: vec![plural_element],
    };

    let comment = context.get_original_comment(&android_plural.name)
        .cloned()
        .or_else(|| android_plural.comment.clone());

    Ok(FluentMessage {
        id: android_plural.name.clone(),
        value: Some(pattern),
        attributes: HashMap::new(),
        comment,
    })
}

fn determine_plural_selector(
    android_plural: &AndroidPlural, 
    context: &ConversionContext
) -> String {
    // Try to get from context first
    if let Some(selector) = context.get_plural_selector(&android_plural.name) {
        return selector.clone();
    }
    
    // Find from variable mapping
    android_plural
        .variable_mapping
        .iter()
        .find(|(placeholder, _)| placeholder.contains('d'))
        .map(|(_, var)| var.clone())
        .unwrap_or_else(|| DEFAULT_COUNT_VARIABLE.to_string())
}

fn create_effective_mapping(
    android_plural: &AndroidPlural, 
    selector: &str
) -> HashMap<String, String> {
    let mut effective_mapping = android_plural.variable_mapping.clone();
    
    // Ensure count mapping exists
    let has_count_mapping = effective_mapping.iter()
        .any(|(placeholder, _)| placeholder.contains('d'));
    
    if !has_count_mapping {
        // Find and map count placeholder
        for android_text in android_plural.items.values() {
            if android_text.contains(COUNT_PLACEHOLDER) {
                effective_mapping.insert(COUNT_PLACEHOLDER.to_string(), selector.to_string());
                break;
            } else if let Some(captures) = Regex::new(NUMBERED_COUNT_REGEX).unwrap().captures(android_text) {
                if let Some(placeholder) = captures.get(1) {
                    effective_mapping.insert(placeholder.as_str().to_string(), selector.to_string());
                    break;
                }
            }
        }
    }
    
    effective_mapping
}

fn convert_android_text_to_fluent_pattern(
    android_text: &str,
    variable_mapping: &HashMap<String, String>,
    original_variables: Option<&Vec<String>>,
) -> Result<FluentPattern> {
    let mut elements = Vec::new();
    let re = Regex::new(ANDROID_PLACEHOLDER_REGEX).unwrap();
    
    let mut last_end = 0;
    let mut var_index = 0;
    
    for mat in re.find_iter(android_text) {
        // Add text before placeholder
        add_text_element_if_not_empty(&mut elements, &android_text[last_end..mat.start()]);
        
        // Add variable element
        let placeholder = mat.as_str();
        let var_name = determine_variable_name(placeholder, variable_mapping, original_variables, var_index);
        elements.push(FluentElement::Variable(var_name));
        var_index += 1;
        
        last_end = mat.end();
    }
    
    // Add remaining text
    add_text_element_if_not_empty(&mut elements, &android_text[last_end..]);
    
    // Handle case with no placeholders
    if elements.is_empty() {
        elements.push(FluentElement::Text(unescape_android_string(android_text)));
    }

    Ok(FluentPattern { elements })
}

// Helper functions
fn create_placeholder(counter: u32) -> String {
    format!("%{}$s", counter)
}

fn add_text_element_if_not_empty(elements: &mut Vec<FluentElement>, text: &str) {
    let unescaped = unescape_android_string(text);
    if !unescaped.is_empty() {
        elements.push(FluentElement::Text(unescaped));
    }
}

fn determine_variable_name(
    placeholder: &str,
    variable_mapping: &HashMap<String, String>,
    original_variables: Option<&Vec<String>>,
    var_index: usize,
) -> String {
    // Try variable mapping first
    if let Some(var_name) = variable_mapping.get(placeholder) {
        return var_name.clone();
    }
    
    // Try original variables
    if let Some(vars) = original_variables {
        if var_index < vars.len() {
            return vars[var_index].clone();
        }
    }
    
    // Fallback to generated name
    format!("var{}", var_index + 1)
}

fn escape_android_string(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn unescape_android_string(text: &str) -> String {
    text.replace("\\\\", "\\")
        .replace("\\\"", "\"")
        .replace("\\'", "'")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\u0024", "$") // Handle unicode escapes like \u0024 for $
}

/// Map Fluent quantity keys to Android XML quantity names
fn map_fluent_to_android_quantity(fluent_quantity: &str) -> String {
    match fluent_quantity {
        // Map numeric forms to Android quantity names
        "0" => "zero",
        "1" => "one",
        "2" => "two",
        // Keep named forms as-is (they should already be valid Android quantities)
        "zero" | "one" | "two" | "few" | "many" | "other" => fluent_quantity,
        // For any other numeric values or unknown quantities, map to "other"
        _ => if fluent_quantity.chars().all(|c| c.is_ascii_digit()) {
            "other"
        } else {
            fluent_quantity
        }
    }.to_string()
}

/// Map Android quantity names back to Fluent quantity keys for round-trip conversion
fn map_android_to_fluent_quantity(android_quantity: &str) -> String {
    match android_quantity {
        "zero" => "0",
        "one" => "one", // Preserve 'one' as named form for better round-trip
        "two" => "2",
        "few" | "many" | "other" => android_quantity,
        _ => android_quantity,
    }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_escape_android_string() {
        assert_eq!(escape_android_string("Hello \"World\""), r#"Hello \"World\""#);
        assert_eq!(escape_android_string("Line1\nLine2"), "Line1\\nLine2");
        assert_eq!(escape_android_string("Tab\tHere"), "Tab\\tHere");
        assert_eq!(escape_android_string("Don't"), "Don\\'t");
    }

    #[test]
    fn test_unescape_android_string() {
        assert_eq!(unescape_android_string(r#"Hello \"World\""#), "Hello \"World\"");
        assert_eq!(unescape_android_string("Line1\\nLine2"), "Line1\nLine2");
        assert_eq!(unescape_android_string("Tab\\tHere"), "Tab\tHere");
        assert_eq!(unescape_android_string("Don\\'t"), "Don't");
        assert_eq!(unescape_android_string("\\u0024100"), "$100");
    }

    #[test]
    fn test_convert_simple_pattern_to_android() {
        use crate::shared::fluent_parser::{FluentMessage, FluentPattern, FluentElement};
        
        let message = FluentMessage {
            id: "greeting".to_string(),
            value: Some(FluentPattern {
                elements: vec![
                    FluentElement::Text("Hello, ".to_string()),
                    FluentElement::Variable("name".to_string()),
                    FluentElement::Text("!".to_string()),
                ],
            }),
            attributes: HashMap::new(),
            comment: Some("This is a greeting message".to_string()),
        };
        
        let pattern = message.value.as_ref().unwrap();
        let android_string = convert_simple_pattern_to_android(&message, pattern).unwrap();
        
        assert_eq!(android_string.name, "greeting");
        assert_eq!(android_string.value, "Hello, %1$s!");
        assert_eq!(android_string.variable_mapping.get("%1$s"), Some(&"name".to_string()));
        assert_eq!(android_string.comment, Some("This is a greeting message".to_string()));
    }

    #[test]
    fn test_convert_android_text_to_fluent_pattern() {
        let mut variable_mapping = HashMap::new();
        variable_mapping.insert("%1$s".to_string(), "name".to_string());
        variable_mapping.insert("%d".to_string(), "count".to_string());
        
        let pattern = convert_android_text_to_fluent_pattern(
            "Hello %1$s, you have %d items",
            &variable_mapping,
            None,
        ).unwrap();
        
        assert_eq!(pattern.elements.len(), 5);
        
        // Check the structure: "Hello " + {$name} + ", you have " + {$count} + " items"
        if let FluentElement::Text(text) = &pattern.elements[0] {
            assert_eq!(text, "Hello ");
        } else {
            panic!("Expected text element");
        }
        
        if let FluentElement::Variable(var) = &pattern.elements[1] {
            assert_eq!(var, "name");
        } else {
            panic!("Expected variable element");
        }
        
        if let FluentElement::Text(text) = &pattern.elements[2] {
            assert_eq!(text, ", you have ");
        } else {
            panic!("Expected text element");
        }
        
        if let FluentElement::Variable(var) = &pattern.elements[3] {
            assert_eq!(var, "count");
        } else {
            panic!("Expected variable element");
        }
        
        if let FluentElement::Text(text) = &pattern.elements[4] {
            assert_eq!(text, " items");
        } else {
            panic!("Expected text element");
        }
    }

    #[test]
    fn test_round_trip_conversion() {
        use crate::shared::fluent_parser::FluentResource;
        
        // Start with Fluent
        let original_ftl = r#"hello = Hello World
greeting = Hello, {$name}!
count = {$num ->
    [one] {$num} item
   *[other] {$num} items
}"#;
        
        // Convert to Android XML
        let fluent_resource = FluentResource::from_source(original_ftl).unwrap();
        let android_resources = convert_fluent_to_android(&fluent_resource).unwrap();
        
        // Convert back to Fluent
        let conversion_context = ConversionContext::default();
        let converted_fluent = convert_android_to_fluent(&android_resources, &conversion_context).unwrap();
        
        // Check that we have the same number of messages
        assert_eq!(converted_fluent.messages.len(), 3);
        
        // Check specific messages exist
        let hello_msg = converted_fluent.messages.iter().find(|m| m.id == "hello").unwrap();
        assert!(hello_msg.value.is_some());
        
        let greeting_msg = converted_fluent.messages.iter().find(|m| m.id == "greeting").unwrap();
        assert!(greeting_msg.value.is_some());
        
        let count_msg = converted_fluent.messages.iter().find(|m| m.id == "count").unwrap();
        assert!(count_msg.value.is_some());
    }

    #[test]
    fn test_classify_pattern() {
        use crate::shared::fluent_parser::{FluentPattern, FluentElement};
        
        // Simple pattern
        let simple_pattern = FluentPattern {
            elements: vec![FluentElement::Text("Hello".to_string())],
        };
        assert!(matches!(classify_pattern(&simple_pattern), PatternType::Simple));
        
        // Plural pattern
        let plural_pattern = FluentPattern {
            elements: vec![FluentElement::Plural {
                selector: "count".to_string(),
                variants: HashMap::new(),
            }],
        };
        assert!(matches!(classify_pattern(&plural_pattern), PatternType::Plural));
    }

    #[test]
    fn test_positional_parameters_fluent_to_android() {
        use crate::shared::fluent_parser::{FluentMessage, FluentPattern, FluentElement};
        
        // Test with multiple variables to ensure all use positional parameters
        let message = FluentMessage {
            id: "multi_vars".to_string(),
            value: Some(FluentPattern {
                elements: vec![
                    FluentElement::Text("Welcome ".to_string()),
                    FluentElement::Variable("name".to_string()),
                    FluentElement::Text(", you have ".to_string()),
                    FluentElement::Variable("count".to_string()),
                    FluentElement::Text(" messages in ".to_string()),
                    FluentElement::Variable("folder".to_string()),
                    FluentElement::Text("!".to_string()),
                ],
            }),
            attributes: HashMap::new(),
            comment: None,
        };
        
        let pattern = message.value.as_ref().unwrap();
        let android_string = convert_simple_pattern_to_android(&message, pattern).unwrap();
        
        assert_eq!(android_string.name, "multi_vars");
        assert_eq!(android_string.value, "Welcome %1$s, you have %2$s messages in %3$s!");
        assert_eq!(android_string.variable_mapping.get("%1$s"), Some(&"name".to_string()));
        assert_eq!(android_string.variable_mapping.get("%2$s"), Some(&"count".to_string()));
        assert_eq!(android_string.variable_mapping.get("%3$s"), Some(&"folder".to_string()));
    }

    #[test]
    fn test_positional_parameters_android_to_fluent() {
        let mut variable_mapping = HashMap::new();
        variable_mapping.insert("%1$s".to_string(), "name".to_string());
        variable_mapping.insert("%2$s".to_string(), "count".to_string());
        variable_mapping.insert("%3$s".to_string(), "folder".to_string());
        
        let pattern = convert_android_text_to_fluent_pattern(
            "Welcome %1$s, you have %2$s messages in %3$s!",
            &variable_mapping,
            None,
        ).unwrap();
        
        assert_eq!(pattern.elements.len(), 7);
        
        // Check the structure
        if let FluentElement::Text(text) = &pattern.elements[0] {
            assert_eq!(text, "Welcome ");
        } else {
            panic!("Expected text element");
        }
        
        if let FluentElement::Variable(var) = &pattern.elements[1] {
            assert_eq!(var, "name");
        } else {
            panic!("Expected variable element");
        }
        
        if let FluentElement::Text(text) = &pattern.elements[2] {
            assert_eq!(text, ", you have ");
        } else {
            panic!("Expected text element");
        }
        
        if let FluentElement::Variable(var) = &pattern.elements[3] {
            assert_eq!(var, "count");
        } else {
            panic!("Expected variable element");
        }
        
        if let FluentElement::Text(text) = &pattern.elements[4] {
            assert_eq!(text, " messages in ");
        } else {
            panic!("Expected text element");
        }
        
        if let FluentElement::Variable(var) = &pattern.elements[5] {
            assert_eq!(var, "folder");
        } else {
            panic!("Expected variable element");
        }
        
        if let FluentElement::Text(text) = &pattern.elements[6] {
            assert_eq!(text, "!");
        } else {
            panic!("Expected text element");
        }
    }
}
