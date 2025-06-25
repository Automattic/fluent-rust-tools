use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::android::android_format::{AndroidResources, AndroidString, AndroidPlural};
use crate::shared::fluent_parser::{FluentResource, FluentMessage, FluentPattern, FluentElement, format_string_value_as_multiline_fluent_text};

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
    let unescaped_text = unescape_android_string(android_text);
    let formatted_text = format_string_value_as_multiline_fluent_text(&unescaped_text);

    // Regex to check if the text has placeholders
    let re = Regex::new(ANDROID_PLACEHOLDER_REGEX).unwrap();

    let mut elements = Vec::new();
    
    let mut last_end = 0;
    let mut var_index = 0;
    
    for mat in re.find_iter(&formatted_text) {
        // Add text before placeholder
        add_text_element_if_not_empty(&mut elements, &formatted_text[last_end..mat.start()]);
        
        // Add variable element
        let placeholder = mat.as_str();
        let var_name = determine_variable_name(placeholder, variable_mapping, original_variables, var_index);
        elements.push(FluentElement::Variable(var_name));
        var_index += 1;
        
        last_end = mat.end();
    }
    
    // Add remaining text
    add_text_element_if_not_empty(&mut elements, &formatted_text[last_end..]);
    
    // Handle case with no placeholders
    if elements.is_empty() {
        elements.push(FluentElement::Text(formatted_text));
    }

    Ok(FluentPattern { elements })
}

// Helper functions
fn create_placeholder(counter: u32) -> String {
    format!("%{}$s", counter)
}

fn add_text_element_if_not_empty(elements: &mut Vec<FluentElement>, text: &str) {
    if !text.is_empty() {
        elements.push(FluentElement::Text(text.to_string()));
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
    // For Android XML, we only need to escape actual control characters
    // Quotes and apostrophes will be handled by the XML writer
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn unescape_android_string(text: &str) -> String {
    text.replace("\\\\", "\\")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\u0024", "$") // Handle unicode escapes like \u0024 for $
        // Handle HTML entities that might appear in Android XML
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&") // This should be last to avoid double-unescaping
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
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_fluent_to_android_file_simple() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let output_path = temp_dir.path().join("output.xml");

        let fluent_content = "hello = Hello World";
        fs::write(&input_path, fluent_content).unwrap();

        let result = fluent_to_android(&input_path, &output_path);
        assert!(result.is_ok());

        let xml_content = fs::read_to_string(&output_path).unwrap();
        assert!(xml_content.contains(r#"<string name="hello">Hello World</string>"#));
    }

    #[test]
    fn test_android_to_fluent_file_simple() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.xml");
        let output_path = temp_dir.path().join("output.ftl");

        let xml_content = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
  <string name="hello">Hello World</string>
</resources>"#;
        fs::write(&input_path, xml_content).unwrap();

        let result = android_to_fluent(&input_path, &output_path);
        assert!(result.is_ok());

        let fluent_content = fs::read_to_string(&output_path).unwrap();
        assert!(fluent_content.contains("hello = Hello World"));
    }

    #[test]
    fn test_round_trip_conversion_file() {
        let temp_dir = tempdir().unwrap();
        let original_ftl_path = temp_dir.path().join("original.ftl");
        let xml_path = temp_dir.path().join("intermediate.xml");
        let final_ftl_path = temp_dir.path().join("final.ftl");

        let original_content = r#"
# General greeting
hello = Hello World
# Greeting with a variable
greeting = Hello, {$name}!
"#;
        fs::write(&original_ftl_path, original_content).unwrap();

        // Fluent to Android
        assert!(fluent_to_android(&original_ftl_path, &xml_path).is_ok());
        
        // Android to Fluent with original context for better variable name preservation
        assert!(android_to_fluent_with_original(&xml_path, &final_ftl_path, &original_ftl_path).is_ok());

        let final_content = fs::read_to_string(&final_ftl_path).unwrap();
        assert!(final_content.contains("hello = Hello World"));
        assert!(final_content.contains("greeting = Hello, { $name }!")); // Built-in serializer normalizes formatting
    }

    #[test]
    fn test_round_trip_conversion_with_plurals_file() {
        let temp_dir = tempdir().unwrap();
        let original_ftl_path = temp_dir.path().join("original.ftl");
        let xml_path = temp_dir.path().join("intermediate.xml");
        let final_ftl_path = temp_dir.path().join("final.ftl");

        let original_content = r#"
# A pluralized message
item_count = {$count ->
    [one] { $count } item
   *[other] { $count } items
}
"#;
        fs::write(&original_ftl_path, original_content).unwrap();

        // Fluent to Android
        assert!(fluent_to_android(&original_ftl_path, &xml_path).is_ok());

        // Android to Fluent with context
        assert!(android_to_fluent_with_original(&xml_path, &final_ftl_path, &original_ftl_path).is_ok());

        let final_content = fs::read_to_string(final_ftl_path).unwrap();
        // Built-in serializer uses multiline formatting for plurals
        assert!(final_content.contains("item_count ="));
        assert!(final_content.contains("{ $count ->"));
        assert!(final_content.contains("[one] { $count } item"));
        assert!(final_content.contains("*[other] { $count } items"));
    }
    
    #[test]
    fn test_fluent_to_android_with_comments_file() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let output_path = temp_dir.path().join("output.xml");

        let fluent_content = r#"# This is a comment for a simple string
hello = Hello World
# This is a comment for a plural message
# with multiple lines
item_count = {$count ->
    [one] { $count } item
   *[other] { $count } items
}
"#;
        fs::write(&input_path, fluent_content).unwrap();

        assert!(fluent_to_android(&input_path, &output_path).is_ok());

        let xml_content = fs::read_to_string(&output_path).unwrap();
        
        // Check that comments are preserved in the XML output
        // Note: The comment extraction captures the last comment before each message
        assert!(xml_content.contains("This is a comment for a simple string"));
        assert!(xml_content.contains("with multiple lines")); // Last line of multi-line comment
    }

    #[test]
    fn test_escape_android_string() {
        // Quotes and apostrophes are no longer escaped - XML writer handles them
        assert_eq!(escape_android_string("Hello \"World\""), "Hello \"World\"");
        assert_eq!(escape_android_string("Line1\nLine2"), "Line1\\nLine2");
        assert_eq!(escape_android_string("Tab\tHere"), "Tab\\tHere");
        assert_eq!(escape_android_string("Don't"), "Don't");
    }

    #[test]
    fn test_unescape_android_string() {
        // Test HTML entity unescaping (main case now)
        assert_eq!(unescape_android_string("Hello &quot;World&quot;"), "Hello \"World\"");
        assert_eq!(unescape_android_string("Line1\\nLine2"), "Line1\nLine2");
        assert_eq!(unescape_android_string("Tab\\tHere"), "Tab\tHere");
        assert_eq!(unescape_android_string("Don&apos;t"), "Don't");
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

    #[test]
    fn test_multiline_android_to_fluent_formatting() {
        // Test that multiline Android strings are correctly formatted with proper indentation
        let variable_mapping = HashMap::new();
        
        // Test multiline Android text (using \n as Android would store it)
        let android_text = "This is line one\\nThis is line two\\nThis is line three";
        
        let pattern = convert_android_text_to_fluent_pattern(
            android_text,
            &variable_mapping,
            None,
        ).unwrap();
        
        // The pattern should have 1 text element containing the properly formatted multiline text
        assert_eq!(pattern.elements.len(), 1);
        
        // Check that the element contains the expected multiline text with proper formatting
        if let FluentElement::Text(text) = &pattern.elements[0] {
            // The text should be formatted with proper indentation for Fluent
            let expected_text = "This is line one\n    This is line two\n    This is line three";
            assert_eq!(text, expected_text);
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
        
        // Verify the generated Fluent can be parsed back without errors
        let reparsed = FluentResource::from_source(&fluent_content);
        assert!(reparsed.is_ok(), "Generated multiline Fluent should be parseable without errors");
        
        let reparsed_resource = reparsed.unwrap();
        assert_eq!(reparsed_resource.messages.len(), 1);
        assert_eq!(reparsed_resource.messages[0].id, "test-multiline");
    }

    #[test]
    fn test_plural_conversion_with_mixed_variables() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let output_path = temp_dir.path().join("output.xml");

        // Fluent with plural selector variable and other variables
        let fluent_content = r#"shared-photos = {$photoCount ->
    [one] {$userName} added {$photoCount} new photo to {$album}.
   *[other] {$userName} added {$photoCount} new photos to {$album}.
}"#;
        fs::write(&input_path, fluent_content).unwrap();

        let result = fluent_to_android(&input_path, &output_path);
        assert!(result.is_ok());

        let xml_content = fs::read_to_string(&output_path).unwrap();
        
        // Verify the entire Android XML structure as a string
        let expected_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
  <plurals name="shared-photos">
    <item quantity="one">%1$s added %d new photo to %2$s.</item>
    <item quantity="other">%1$s added %d new photos to %2$s.</item>
  </plurals>
</resources>"#;
        assert_eq!(xml_content, expected_xml);
    }
}
