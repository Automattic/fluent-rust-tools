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
