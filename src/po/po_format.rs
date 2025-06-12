use polib::po_file;
use polib::catalog::Catalog;
use polib::metadata::CatalogMetadata;
use polib::message::{Message as PoMessage, MessageView};

use anyhow::Result;
use std::path::Path;
use crate::shared::fluent_parser::{FluentResource, FluentMessage, FluentPattern, FluentElement, extract_pattern_text};
use crate::shared::error::ConversionError;

// PO file parsing
pub fn parse_po_file(input_path: &Path) -> Result<Catalog> {
    po_file::parse(input_path)
        .map_err(|e| ConversionError::PoParseError(format!("Failed to parse PO file: {}", e)).into())
}

// PO file writing
pub fn write_po_file(catalog: &Catalog, output_path: &Path) -> Result<()> {
    po_file::write(catalog, output_path)
        .map_err(|e| ConversionError::PoWriteError(format!("Failed to write PO file: {}", e)))?;
    Ok(())
}

// Fluent to PO conversion
pub fn fluent_to_po_catalog(resource: FluentResource, locale: &str) -> Result<Catalog> {
    let mut metadata = CatalogMetadata::default();
    
    // Set catalog metadata properly
    metadata.content_type = "text/plain; charset=UTF-8".to_string();
    metadata.language = locale.to_string();
    metadata.mime_version = "1.0".to_string();
    metadata.content_transfer_encoding = "8bit".to_string();
    
    let mut catalog = Catalog::new(metadata);
    
    for message in resource.messages {
        convert_message_to_po(&mut catalog, &message)?;
    }
    
    Ok(catalog)
}

// PO to Fluent conversion
pub fn po_catalog_to_fluent(catalog: Catalog) -> Result<String> {
    let mut content = String::new();
    
    for message in catalog.messages() {
        // Add comments from extracted comments if present
        if !message.comments().is_empty() {
            for comment_line in message.comments().lines() {
                content.push_str(&format!("# {}\n", comment_line));
            }
        }
        
        // Generate Fluent key from msgctxt or msgid
        let key = if !message.msgctxt().is_empty() {
            message.msgctxt().to_string()
        } else {
            message.msgid().replace(' ', "-").replace('"', "").to_lowercase()
        };
        
        // Convert message based on whether it's plural or singular
        if message.is_plural() {
            convert_plural_message_to_fluent(&mut content, &key, message)?;
        } else {
            convert_singular_message_to_fluent(&mut content, &key, message)?;
        }
        
        content.push('\n');
    }
    
    Ok(content)
}

fn convert_message_to_po(catalog: &mut Catalog, message: &FluentMessage) -> Result<()> {
    let msgctxt = message.id.clone();
    
    // Handle main message value
    if let Some(pattern) = &message.value {
        let msgid = extract_pattern_text(pattern);
        
        // Extract comment if present
        let extracted_comments = message.comment.as_ref().unwrap_or(&String::new()).clone();
        
        // Check if this is a plural pattern
        if let Some(plural_info) = extract_plural_info(pattern) {
            // Find the appropriate msgid and msgid_plural
            let (msgid, msgid_plural, msgstr_forms) = create_po_plural_forms(&plural_info);
            
            // Create a plural message
            let mut msg_builder = PoMessage::build_plural();
            msg_builder
                .with_msgctxt(msgctxt)
                .with_msgid(msgid)
                .with_msgid_plural(msgid_plural)
                .with_msgstr_plural(msgstr_forms);
            
            // Store selector information in comments to preserve it for roundtrip
            let selector_comment = format!("FLUENT_SELECTOR:{}", plural_info.selector);
            let combined_comments = if extracted_comments.is_empty() {
                selector_comment
            } else {
                format!("{}\n{}", extracted_comments, selector_comment)
            };
            msg_builder.with_comments(combined_comments);
            
            let message = msg_builder.done();
            catalog.append_or_update(message);
        } else {
            // Create a singular message
            let mut msg_builder = PoMessage::build_singular();
            msg_builder
                .with_msgctxt(msgctxt)
                .with_msgid(msgid.clone())
                .with_msgstr(msgid); // For now, we use the same text as the translation
            
            if !extracted_comments.is_empty() {
                msg_builder.with_comments(extracted_comments);
            }
            
            let message = msg_builder.done();
            catalog.append_or_update(message);
        }
    }
    
    // Handle attributes
    for (attr_name, attr_pattern) in &message.attributes {
        let attr_msgctxt = format!("{}.{}", message.id, attr_name);
        let attr_msgid = extract_pattern_text(attr_pattern);
        
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt(attr_msgctxt)
            .with_msgid(attr_msgid.clone())
            .with_msgstr(attr_msgid); // For now, we use the same text as the translation
        
        let message = msg_builder.done();
        catalog.append_or_update(message);
    }
    
    Ok(())
}

fn convert_singular_message_to_fluent(content: &mut String, key: &str, message: &dyn MessageView) -> Result<()> {
    let msgstr = message.msgstr()?;
    
    if msgstr.contains('\n') {
        // Multi-line message
        content.push_str(&format!("{} =\n", key));
        for line in msgstr.lines() {
            content.push_str(&format!("    {}\n", unescape_fluent_value(line)));
        }
    } else {
        // Single-line message
        content.push_str(&format!("{} = {}\n", key, unescape_fluent_value(msgstr)));
    }
    
    Ok(())
}

fn convert_plural_message_to_fluent(content: &mut String, key: &str, message: &dyn MessageView) -> Result<()> {
    if let Ok(msgstr_plural) = message.msgstr_plural() {
        // Extract selector from comments, fallback to "count"
        let selector = extract_selector_from_comments(message.comments()).unwrap_or_else(|| "count".to_string());
        
        // Create a select expression for plurals
        content.push_str(&format!("{} = {{${} ->\n", key, selector));
        
        let mut has_other = false;
        
        for msgstr in msgstr_plural.iter() {
            let cleaned_msgstr = unescape_fluent_value(msgstr);
            
            // Parse our special markers to reconstruct the original Fluent structure
            if let Some(colon_pos) = cleaned_msgstr.find(':') {
                let (marker, text) = cleaned_msgstr.split_at(colon_pos);
                let text = &text[1..]; // Remove the ':' character
                
                match marker {
                    "FLUENT_ZERO" => {
                        content.push_str(&format!("    [0] {}\n", text));
                    }
                    "FLUENT_ONE" => {
                        content.push_str(&format!("    [one] {}\n", text));
                    }
                    "FLUENT_OTHER" => {
                        if !has_other {
                            content.push_str(&format!("   *[other] {}\n", text));
                            has_other = true;
                        }
                    }
                    other_marker if other_marker.starts_with("FLUENT_") => {
                        let key_part = &other_marker[7..]; // Remove "FLUENT_" prefix
                        let key_lower = key_part.to_lowercase();
                        if key_lower == "other" && !has_other {
                            content.push_str(&format!("   *[other] {}\n", text));
                            has_other = true;
                        } else {
                            // Handle numeric or other special keys
                            content.push_str(&format!("    [{}] {}\n", key_lower, text));
                        }
                    }
                    _ => {
                        // Fallback for malformed markers - treat as other
                        if !has_other {
                            content.push_str(&format!("   *[other] {}\n", text));
                            has_other = true;
                        }
                    }
                }
            } else {
                // Fallback for messages without markers - treat as other
                if !has_other {
                    content.push_str(&format!("   *[other] {}\n", cleaned_msgstr));
                    has_other = true;
                }
            }
        }
        
        // Ensure we always have an *[other] case
        if !has_other {
            content.push_str(&format!("   *[other] {}\n", message.msgid_plural().unwrap_or("")));
        }
        
        content.push_str("}\n");
    }
    
    Ok(())
}

struct PluralInfo {
    selector: String, // The selector variable name (e.g., "count")
    forms: Vec<(String, String)>, // (key, text) pairs for all plural forms
}

fn extract_plural_info(pattern: &FluentPattern) -> Option<PluralInfo> {
    // Look for plural elements in the pattern
    for element in &pattern.elements {
        if let FluentElement::Plural { selector, variants } = element {
            let mut forms = Vec::new();
            
            for (key, variant_pattern) in variants {
                let text = extract_pattern_text(variant_pattern);
                forms.push((key.clone(), text));
            }
            
            if !forms.is_empty() {
                return Some(PluralInfo { 
                    selector: selector.clone(),
                    forms 
                });
            }
        }
    }
    None
}

fn create_po_plural_forms(plural_info: &PluralInfo) -> (String, String, Vec<String>) {
    let mut zero_form = None;
    let mut one_form = None;
    let mut other_form = None;
    let mut numeric_forms = std::collections::HashMap::new();
    
    // Categorize all the forms
    for (key, text) in &plural_info.forms {
        match key.as_str() {
            "0" => zero_form = Some(text.clone()),
            "1" => { numeric_forms.insert("1".to_string(), text.clone()); },
            "one" => one_form = Some(text.clone()),
            "other" => other_form = Some(text.clone()),
            _num if key.chars().all(|c| c.is_ascii_digit()) => {
                numeric_forms.insert(key.clone(), text.clone());
            },
            _ => {} // Handle other cases like "few", "many" later if needed
        }
    }
    
    // For msgid (singular), prefer [one] form, fallback to [0] or first available
    let msgid = one_form.as_ref()
        .or(zero_form.as_ref())
        .or(numeric_forms.get("1"))
        .or_else(|| plural_info.forms.first().map(|(_, text)| text))
        .unwrap_or(&"".to_string())
        .clone();
        
    // For msgid_plural, use [other] form
    let msgid_plural = other_form.as_ref()
        .or_else(|| plural_info.forms.iter().find(|(key, _)| key != "one" && key != "0" && key != "1").map(|(_, text)| text))
        .unwrap_or(&"".to_string())
        .clone();
    
    // Create msgstr array that preserves all forms with special markers
    // This allows us to reconstruct the original Fluent structure
    let mut msgstr_forms = Vec::new();
    
    // Include all forms in order with their type markers
    if let Some(zero) = &zero_form {
        msgstr_forms.push(format!("FLUENT_ZERO:{}", zero));
    }
    
    if let Some(one) = &one_form {
        msgstr_forms.push(format!("FLUENT_ONE:{}", one));
    }
    
    if let Some(other) = &other_form {
        msgstr_forms.push(format!("FLUENT_OTHER:{}", other));
    }
    
    // If we don't have standard forms, preserve original order
    if msgstr_forms.is_empty() {
        for (key, text) in &plural_info.forms {
            msgstr_forms.push(format!("FLUENT_{}:{}", key.to_uppercase(), text));
        }
    }
    
    // PO requires at least 2 forms
    while msgstr_forms.len() < 2 {
        msgstr_forms.push(format!("FLUENT_OTHER:{}", msgid_plural));
    }
    
    (msgid, msgid_plural, msgstr_forms)
}

fn extract_selector_from_comments(comments: &str) -> Option<String> {
    // Look for the FLUENT_SELECTOR: marker in the comments
    for line in comments.lines() {
        if let Some(selector_part) = line.trim().strip_prefix("FLUENT_SELECTOR:") {
            return Some(selector_part.trim().to_string());
        }
    }
    None
}

fn unescape_fluent_value(value: &str) -> String {
    // Reverse the escaping that was applied during Fluent->PO conversion
    // Only unescape things that shouldn't be escaped in Fluent
    value
        .replace("\\{", "{")
        .replace("\\}", "}")
        .replace("\\\\", "\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_fluent_to_po_catalog_simple() {
        // Create a simple Fluent resource
        let fluent_messages = vec![
            FluentMessage {
                id: "hello".to_string(),
                value: Some(FluentPattern {
                    elements: vec![FluentElement::Text("Hello World".to_string())],
                }),
                attributes: HashMap::new(),
                comment: Some("A simple greeting".to_string()),
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
        ];
        
        let fluent_resource = FluentResource {
            messages: fluent_messages,
        };
        
        let result = fluent_to_po_catalog(fluent_resource, "en");
        assert!(result.is_ok());
        
        let catalog = result.unwrap();
        assert_eq!(catalog.messages().count(), 2);
        
        // Check metadata
        assert_eq!(catalog.metadata.language, "en");
        assert_eq!(catalog.metadata.content_type, "text/plain; charset=UTF-8");
    }

    #[test]
    fn test_fluent_to_po_catalog_with_plurals() {
        // Create a Fluent resource with plurals
        let mut variants = HashMap::new();
        variants.insert("one".to_string(), FluentPattern {
            elements: vec![
                FluentElement::Variable("count".to_string()),
                FluentElement::Text(" item".to_string()),
            ],
        });
        variants.insert("other".to_string(), FluentPattern {
            elements: vec![
                FluentElement::Variable("count".to_string()),
                FluentElement::Text(" items".to_string()),
            ],
        });
        
        let fluent_messages = vec![
            FluentMessage {
                id: "item_count".to_string(),
                value: Some(FluentPattern {
                    elements: vec![FluentElement::Plural {
                        selector: "count".to_string(),
                        variants,
                    }],
                }),
                attributes: HashMap::new(),
                comment: Some("Item counter".to_string()),
            },
        ];
        
        let fluent_resource = FluentResource {
            messages: fluent_messages,
        };
        
        let result = fluent_to_po_catalog(fluent_resource, "en");
        assert!(result.is_ok());
        
        let catalog = result.unwrap();
        assert_eq!(catalog.messages().count(), 1);
        
        // Check that it's a plural message
        let message = catalog.messages().next().unwrap();
        assert!(message.is_plural());
        assert!(message.comments().contains("FLUENT_SELECTOR:count"));
    }

    #[test]
    fn test_po_catalog_to_fluent_simple() {
        // Create a simple PO catalog
        let mut metadata = CatalogMetadata::default();
        metadata.language = "en".to_string();
        let mut catalog = Catalog::new(metadata);
        
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("hello".to_string())
            .with_msgid("Hello World".to_string())
            .with_msgstr("Hello World".to_string())
            .with_comments("A simple greeting".to_string());
        
        let message = msg_builder.done();
        catalog.append_or_update(message);
        
        let result = po_catalog_to_fluent(catalog);
        assert!(result.is_ok());
        
        let fluent_content = result.unwrap();
        assert!(fluent_content.contains("# A simple greeting"));
        assert!(fluent_content.contains("hello = Hello World"));
    }

    #[test]
    fn test_po_catalog_to_fluent_with_plurals() {
        // Create a PO catalog with plurals
        let mut metadata = CatalogMetadata::default();
        metadata.language = "en".to_string();
        let mut catalog = Catalog::new(metadata);
        
        let mut msg_builder = PoMessage::build_plural();
        msg_builder
            .with_msgctxt("item_count".to_string())
            .with_msgid("{$count} item".to_string())
            .with_msgid_plural("{$count} items".to_string())
            .with_msgstr_plural(vec![
                "FLUENT_ONE:{$count} item".to_string(),
                "FLUENT_OTHER:{$count} items".to_string(),
            ])
            .with_comments("FLUENT_SELECTOR:count\nItem counter".to_string());
        
        let message = msg_builder.done();
        catalog.append_or_update(message);
        
        let result = po_catalog_to_fluent(catalog);
        assert!(result.is_ok());
        
        let fluent_content = result.unwrap();
        assert!(fluent_content.contains("item_count = {$count ->"));
        assert!(fluent_content.contains("[one] {$count} item"));
        assert!(fluent_content.contains("*[other] {$count} items"));
        assert!(fluent_content.contains("# Item counter"));
    }

    #[test]
    fn test_extract_plural_info() {
        // Test with a plural pattern
        let mut variants = HashMap::new();
        variants.insert("one".to_string(), FluentPattern {
            elements: vec![FluentElement::Text("one item".to_string())],
        });
        variants.insert("other".to_string(), FluentPattern {
            elements: vec![FluentElement::Text("many items".to_string())],
        });
        
        let pattern = FluentPattern {
            elements: vec![FluentElement::Plural {
                selector: "count".to_string(),
                variants,
            }],
        };
        
        let plural_info = extract_plural_info(&pattern);
        assert!(plural_info.is_some());
        
        let info = plural_info.unwrap();
        assert_eq!(info.selector, "count");
        assert_eq!(info.forms.len(), 2);
        assert!(info.forms.iter().any(|(key, _)| key == "one"));
        assert!(info.forms.iter().any(|(key, _)| key == "other"));
    }

    #[test]
    fn test_extract_plural_info_none_for_simple() {
        // Test with a simple pattern (no plurals)
        let pattern = FluentPattern {
            elements: vec![FluentElement::Text("Hello World".to_string())],
        };
        
        let plural_info = extract_plural_info(&pattern);
        assert!(plural_info.is_none());
    }

    #[test]
    fn test_create_po_plural_forms() {
        let plural_info = PluralInfo {
            selector: "count".to_string(),
            forms: vec![
                ("one".to_string(), "{$count} item".to_string()),
                ("other".to_string(), "{$count} items".to_string()),
            ],
        };
        
        let (msgid, msgid_plural, msgstr_forms) = create_po_plural_forms(&plural_info);
        
        assert_eq!(msgid, "{$count} item");
        assert_eq!(msgid_plural, "{$count} items");
        assert_eq!(msgstr_forms.len(), 2);
        assert!(msgstr_forms.contains(&"FLUENT_ONE:{$count} item".to_string()));
        assert!(msgstr_forms.contains(&"FLUENT_OTHER:{$count} items".to_string()));
    }

    #[test]
    fn test_create_po_plural_forms_with_numeric() {
        let plural_info = PluralInfo {
            selector: "count".to_string(),
            forms: vec![
                ("0".to_string(), "no items".to_string()),
                ("1".to_string(), "one item".to_string()),
                ("other".to_string(), "many items".to_string()),
            ],
        };
        
        let (msgid, msgid_plural, msgstr_forms) = create_po_plural_forms(&plural_info);
        
        assert_eq!(msgid, "no items"); // Prefers [0] form for msgid
        assert_eq!(msgid_plural, "many items");
        // We expect at least 2 forms since PO requires minimum 2
        assert!(msgstr_forms.len() >= 2);
        assert!(msgstr_forms.contains(&"FLUENT_ZERO:no items".to_string()));
        assert!(msgstr_forms.contains(&"FLUENT_OTHER:many items".to_string()));
    }

    #[test]
    fn test_extract_selector_from_comments() {
        let comments = "This is a test\nFLUENT_SELECTOR:count\nAnother line";
        let selector = extract_selector_from_comments(comments);
        assert_eq!(selector, Some("count".to_string()));
        
        let comments_no_selector = "This is a test\nAnother line";
        let selector = extract_selector_from_comments(comments_no_selector);
        assert_eq!(selector, None);
        
        let comments_with_spaces = "FLUENT_SELECTOR:  item_count  ";
        let selector = extract_selector_from_comments(comments_with_spaces);
        assert_eq!(selector, Some("item_count".to_string()));
    }

    #[test]
    fn test_unescape_fluent_value() {
        assert_eq!(unescape_fluent_value("Hello \\{name\\}"), "Hello {name}");
        assert_eq!(unescape_fluent_value("Path\\\\to\\\\file"), "Path\\to\\file");
        assert_eq!(unescape_fluent_value("Normal text"), "Normal text");
        assert_eq!(unescape_fluent_value("\\{$var\\} text"), "{$var} text");
    }

    #[test]
    fn test_round_trip_conversion_simple() {
        // Create original Fluent resource
        let fluent_messages = vec![
            FluentMessage {
                id: "hello".to_string(),
                value: Some(FluentPattern {
                    elements: vec![FluentElement::Text("Hello World".to_string())],
                }),
                attributes: HashMap::new(),
                comment: Some("A greeting".to_string()),
            },
        ];
        
        let original_fluent = FluentResource {
            messages: fluent_messages,
        };
        
        // Convert to PO
        let po_catalog = fluent_to_po_catalog(original_fluent, "en").unwrap();
        
        // Convert back to Fluent
        let converted_fluent = po_catalog_to_fluent(po_catalog).unwrap();
        
        // Check that the content is preserved
        assert!(converted_fluent.contains("hello = Hello World"));
        assert!(converted_fluent.contains("# A greeting"));
    }

    #[test]
    fn test_round_trip_conversion_with_variables() {
        // Create Fluent resource with variables
        let fluent_messages = vec![
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
        ];
        
        let original_fluent = FluentResource {
            messages: fluent_messages,
        };
        
        // Convert to PO
        let po_catalog = fluent_to_po_catalog(original_fluent, "en").unwrap();
        
        // Convert back to Fluent
        let converted_fluent = po_catalog_to_fluent(po_catalog).unwrap();
        
        // Check that variables are preserved
        assert!(converted_fluent.contains("greeting = Hello, {$name}!"));
    }

    #[test]
    fn test_write_and_parse_po_file() {
        let temp_dir = tempdir().unwrap();
        let po_path = temp_dir.path().join("test.po");
        
        // Create a catalog
        let mut metadata = CatalogMetadata::default();
        metadata.language = "en".to_string();
        metadata.content_type = "text/plain; charset=UTF-8".to_string();
        
        let mut catalog = Catalog::new(metadata);
        
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("test".to_string())
            .with_msgid("Test message".to_string())
            .with_msgstr("Test message".to_string());
        
        let message = msg_builder.done();
        catalog.append_or_update(message);
        
        // Write the file
        let write_result = write_po_file(&catalog, &po_path);
        assert!(write_result.is_ok());
        
        // Verify file exists
        assert!(po_path.exists());
        
        // Parse it back
        let parse_result = parse_po_file(&po_path);
        assert!(parse_result.is_ok());
        
        let parsed_catalog = parse_result.unwrap();
        assert_eq!(parsed_catalog.messages().count(), 1);
        
        let parsed_message = parsed_catalog.messages().next().unwrap();
        assert_eq!(parsed_message.msgctxt(), "test");
        assert_eq!(parsed_message.msgid(), "Test message");
    }
}
