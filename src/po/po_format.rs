use polib::po_file;
use polib::catalog::Catalog;
use polib::metadata::CatalogMetadata;
use polib::message::{Message as PoMessage, MessageView};

use anyhow::Result;
use std::path::Path;
use crate::shared::fluent_parser::{FluentResource, FluentMessage, FluentPattern, FluentElement, extract_pattern_text};
use crate::shared::error::ConversionError;
use std::collections::HashMap;

// =============================================================================
// Constants
// =============================================================================

const REQUIRED_METADATA_FIELDS: &[&str] = &[
    "Project-Id-Version",
    "POT-Creation-Date", 
    "PO-Revision-Date",
    "Last-Translator",
    "Language-Team",
    "MIME-Version",
    "Content-Type",
    "Content-Transfer-Encoding",
    "Language",
    "Plural-Forms",
];

const DEFAULT_CHARSET: &str = "text/plain; charset=UTF-8";
const DEFAULT_ENCODING: &str = "8bit";
const DEFAULT_MIME_VERSION: &str = "1.0";
const DEFAULT_LANGUAGE: &str = "en";
const DEFAULT_PLURAL_FORMS: &str = "nplurals=1; plural=0;";
const FLUENT_SELECTOR_PREFIX: &str = "FLUENT_SELECTOR:";
const FLUENT_MARKER_PREFIX: &str = "FLUENT_";

// Fluent to PO plural form markers
const FLUENT_ZERO_MARKER: &str = "FLUENT_ZERO";
const FLUENT_ONE_MARKER: &str = "FLUENT_ONE";
const FLUENT_OTHER_MARKER: &str = "FLUENT_OTHER";

// =============================================================================
// Public API Functions
// =============================================================================

/// Parse a PO file with robust metadata handling
pub fn parse_po_file(input_path: &Path) -> Result<Catalog> {
    let content = std::fs::read_to_string(input_path)?;
    let preprocessed_content = preprocess_po_content(&content)?;
    
    // Write preprocessed content to temporary file for parsing
    let temp_file = tempfile::NamedTempFile::new()?;
    std::fs::write(temp_file.path(), preprocessed_content)?;
    
    po_file::parse(temp_file.path())
        .map_err(|e| ConversionError::PoParseError(format!("Failed to parse PO file: {}", e)).into())
}

/// Preprocess PO content to ensure all required metadata fields are present
pub fn preprocess_po_content(content: &str) -> Result<String> {
    let mut processor = PoContentProcessor::new(content);
    processor.process()
}

/// Write a PO catalog to file
pub fn write_po_file(catalog: &Catalog, output_path: &Path) -> Result<()> {
    po_file::write(catalog, output_path)
        .map_err(|e| ConversionError::PoWriteError(format!("Failed to write PO file: {}", e)))?;
    Ok(())
}

/// Convert a Fluent resource to a PO catalog
pub fn fluent_to_po_catalog(
    target_resource: FluentResource, 
    locale: &str, 
    source_resource: Option<FluentResource>
) -> Result<Catalog> {
    let metadata = create_po_metadata(locale);
    let mut catalog = Catalog::new(metadata);
    
    let source_lookup = create_source_message_lookup(source_resource.as_ref());
    
    for message in target_resource.messages {
        let source_message = source_lookup.get(&message.id).copied();
        convert_fluent_message_to_po(&mut catalog, &message, source_message)?;
    }
    
    Ok(catalog)
}

/// Convert a PO catalog to Fluent format
pub fn po_catalog_to_fluent(catalog: Catalog) -> Result<String> {
    let mut content = String::new();
    
    for message in catalog.messages() {
        let initial_length = content.len();
        
        add_comments_to_fluent(&mut content, message.comments());
        
        let key = generate_fluent_key_from_message(message);
        
        if message.is_plural() {
            convert_plural_po_message_to_fluent(&mut content, &key, message)?;
            content.push('\n');
        } else {
            convert_singular_po_message_to_fluent(&mut content, &key, message)?;
            // Only add newline if content was actually added
            if content.len() > initial_length {
                content.push('\n');
            }
        }
    }
    
    Ok(content)
}

// =============================================================================
// Helper Structures
// =============================================================================

/// Helper struct for processing PO content
struct PoContentProcessor<'a> {
    lines: Vec<&'a str>,
    result: Vec<String>,
    found_metadata: HashMap<String, bool>,
    header_section: bool,
    header_lines: Vec<String>,
}

impl<'a> PoContentProcessor<'a> {
    fn new(content: &'a str) -> Self {
        Self {
            lines: content.lines().collect(),
            result: Vec::new(),
            found_metadata: HashMap::new(),
            header_section: false,
            header_lines: Vec::new(),
        }
    }
    
    fn process(&mut self) -> Result<String> {
        let lines = self.lines.clone();
        for line in &lines {
            self.process_line(line);
        }
        Ok(self.result.join("\n"))
    }
    
    fn process_line(&mut self, line: &str) {
        match line.trim() {
            "msgid \"\"" => self.start_header_section(line),
            "msgstr \"\"" if self.header_section => self.add_header_line(line),
            _ => self.handle_other_line(line),
        }
    }
    
    fn start_header_section(&mut self, line: &str) {
        self.header_section = true;
        self.add_header_line(line);
    }
    
    fn handle_other_line(&mut self, line: &str) {
        if self.header_section && self.is_metadata_line(line) {
            self.process_metadata_line(line);
        } else if self.header_section && self.is_end_of_header(line) {
            self.finalize_header();
            self.add_line(line);
        } else {
            self.add_line(line);
        }
    }
    
    fn is_metadata_line(&self, line: &str) -> bool {
        line.starts_with('"') && line.ends_with('"')
    }
    
    fn is_end_of_header(&self, line: &str) -> bool {
        !line.starts_with('"') || line.trim().is_empty()
    }
    
    fn process_metadata_line(&mut self, line: &str) {
        self.header_lines.push(line.to_string());
        
        if let Some(colon_pos) = line.find(':') {
            let field_name = &line[1..colon_pos]; // Remove starting quote
            self.found_metadata.insert(field_name.to_string(), true);
        }
    }
    
    fn finalize_header(&mut self) {
        self.header_section = false;
        self.add_missing_metadata_fields();
        self.result.extend(self.header_lines.drain(..));
    }
    
    fn add_missing_metadata_fields(&mut self) {
        for &field in REQUIRED_METADATA_FIELDS {
            if !self.found_metadata.contains_key(field) {
                let default_value = get_default_metadata_value(field);
                self.header_lines.push(format!("\"{}: {}\\n\"", field, default_value));
            }
        }
    }
    
    fn add_line(&mut self, line: &str) {
        self.result.push(line.to_string());
    }
    
    fn add_header_line(&mut self, line: &str) {
        self.header_lines.push(line.to_string());
    }
}

/// Information about plural forms in Fluent messages
#[derive(Debug, Clone)]
struct PluralInfo {
    selector: String,
    forms: Vec<(String, String)>, // (key, text) pairs
}

/// Categorized plural forms for easier processing
#[derive(Default)]
struct CategorizedPluralForms {
    zero_form: Option<String>,
    one_form: Option<String>,
    other_form: Option<String>,
    numeric_forms: HashMap<String, String>,
}

// =============================================================================
// Core Conversion Functions
// =============================================================================

fn convert_fluent_message_to_po(
    catalog: &mut Catalog, 
    message: &FluentMessage, 
    source_message: Option<&FluentMessage>
) -> Result<()> {
    // Convert main message value
    if let Some(pattern) = &message.value {
        convert_main_message_value(catalog, message, pattern, source_message)?;
    }
    
    // Convert attributes
    convert_message_attributes(catalog, message, source_message)?;
    
    Ok(())
}

fn convert_main_message_value(
    catalog: &mut Catalog,
    message: &FluentMessage,
    pattern: &FluentPattern,
    source_message: Option<&FluentMessage>,
) -> Result<()> {
    let target_text = extract_pattern_text(pattern);
    let comments = message.comment.as_ref().unwrap_or(&String::new()).clone();
    
        if let Some(plural_info) = extract_plural_info(pattern) {
        convert_plural_message(catalog, message, &plural_info, source_message, &comments)?;
    } else {
        convert_singular_message(catalog, message, &target_text, source_message, &comments)?;
    }
    
    Ok(())
}

fn convert_plural_message(
    catalog: &mut Catalog,
    message: &FluentMessage,
    plural_info: &PluralInfo,
    source_message: Option<&FluentMessage>,
    comments: &str,
) -> Result<()> {
    let (msgid, msgid_plural, msgstr_forms) = get_plural_forms(plural_info, source_message)?;
    
            let mut msg_builder = PoMessage::build_plural();
            msg_builder
        .with_msgctxt(message.id.clone())
                .with_msgid(msgid)
                .with_msgid_plural(msgid_plural)
                .with_msgstr_plural(msgstr_forms);
            
    let combined_comments = create_combined_comments(comments, &plural_info.selector);
    if !combined_comments.is_empty() {
            msg_builder.with_comments(combined_comments);
    }
    
    catalog.append_or_update(msg_builder.done());
    Ok(())
}

fn convert_singular_message(
    catalog: &mut Catalog,
    message: &FluentMessage,
    target_text: &str,
    source_message: Option<&FluentMessage>,
    comments: &str,
) -> Result<()> {
    let msgid = get_source_text_or_target(source_message, target_text);
    
            let mut msg_builder = PoMessage::build_singular();
            msg_builder
        .with_msgctxt(message.id.clone())
        .with_msgid(msgid)
        .with_msgstr(target_text.to_string());
    
    if !comments.is_empty() {
        msg_builder.with_comments(comments.to_string());
    }
    
    catalog.append_or_update(msg_builder.done());
    Ok(())
}

fn convert_message_attributes(
    catalog: &mut Catalog,
    message: &FluentMessage,
    source_message: Option<&FluentMessage>,
) -> Result<()> {
    for (attr_name, attr_pattern) in &message.attributes {
        let attr_msgctxt = format!("{}.{}", message.id, attr_name);
        let target_attr_text = extract_pattern_text(attr_pattern);
        
        let source_attr_text = source_message
            .and_then(|sm| sm.attributes.get(attr_name))
            .map(|sp| extract_pattern_text(sp));
        
        let msgid = source_attr_text.unwrap_or_else(|| target_attr_text.clone());
        
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt(attr_msgctxt)
            .with_msgid(msgid)
            .with_msgstr(target_attr_text);
        
        catalog.append_or_update(msg_builder.done());
    }
    
    Ok(())
}

fn convert_singular_po_message_to_fluent(
    content: &mut String, 
    key: &str, 
    message: &dyn MessageView
) -> Result<()> {
    let msgstr = message.msgstr()?;
    
    // Skip entries with empty msgstr values as they represent untranslated strings
    if msgstr.trim().is_empty() {
        return Ok(());
    }
    
    if msgstr.contains('\n') {
        write_multiline_fluent_message(content, key, &msgstr);
    } else {
        write_singleline_fluent_message(content, key, &msgstr);
    }
    
    Ok(())
}

fn convert_plural_po_message_to_fluent(
    content: &mut String, 
    key: &str, 
    message: &dyn MessageView
) -> Result<()> {
    let msgstr_plural = message.msgstr_plural()?;
    let selector = extract_selector_from_comments(message.comments())
        .unwrap_or_else(|| "count".to_string());
    
        content.push_str(&format!("{} = {{${} ->\n", key, selector));
        
        let mut has_other = false;
        for msgstr in msgstr_plural.iter() {
        has_other = process_plural_form(content, msgstr, has_other);
    }
    
    ensure_other_form_exists(content, message, has_other);
    content.push_str("}\n");
    
    Ok(())
}

// =============================================================================
// Helper Functions
// =============================================================================

fn create_po_metadata(locale: &str) -> CatalogMetadata {
    let mut metadata = CatalogMetadata::default();
    
    metadata.project_id_version = option_env!("CARGO_PKG_VERSION")
        .map(|v| format!("wordpress-rs {}", v))
        .unwrap_or_else(|| "wordpress-rs".to_string());
    
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M%z").to_string();
    metadata.pot_creation_date = now.clone();
    metadata.po_revision_date = now;
    
    metadata.last_translator = String::new();
    metadata.language_team = String::new();
    metadata.mime_version = DEFAULT_MIME_VERSION.to_string();
    metadata.content_type = DEFAULT_CHARSET.to_string();
    metadata.content_transfer_encoding = DEFAULT_ENCODING.to_string();
    metadata.language = locale.to_string();
    
    metadata
}

fn create_source_message_lookup(source_resource: Option<&FluentResource>) -> HashMap<String, &FluentMessage> {
    source_resource
        .map(|sr| sr.messages.iter().map(|msg| (msg.id.clone(), msg)).collect())
        .unwrap_or_default()
}

fn get_default_metadata_value(field: &str) -> &'static str {
    match field {
        "MIME-Version" => DEFAULT_MIME_VERSION,
        "Content-Type" => DEFAULT_CHARSET,
        "Content-Transfer-Encoding" => DEFAULT_ENCODING,
        "Language" => DEFAULT_LANGUAGE,
        "Plural-Forms" => DEFAULT_PLURAL_FORMS,
        _ => "",
    }
}

fn add_comments_to_fluent(content: &mut String, comments: &str) {
    if !comments.is_empty() {
        for comment_line in comments.lines() {
            content.push_str(&format!("# {}\n", comment_line));
        }
    }
}

fn generate_fluent_key_from_message(message: &dyn MessageView) -> String {
    if !message.msgctxt().is_empty() {
        message.msgctxt().to_string()
    } else {
        message.msgid()
            .replace(' ', "-")
            .replace('"', "")
            .to_lowercase()
    }
}

fn get_source_text_or_target(source_message: Option<&FluentMessage>, target_text: &str) -> String {
    source_message
        .and_then(|sm| sm.value.as_ref())
        .map(|sp| extract_pattern_text(sp))
        .unwrap_or_else(|| target_text.to_string())
}

fn create_combined_comments(existing_comments: &str, selector: &str) -> String {
    let selector_comment = format!("{}{}", FLUENT_SELECTOR_PREFIX, selector);
    
    if existing_comments.is_empty() {
        selector_comment
    } else {
        format!("{}\n{}", existing_comments, selector_comment)
    }
}

fn get_plural_forms(
    plural_info: &PluralInfo, 
    source_message: Option<&FluentMessage>
) -> Result<(String, String, Vec<String>)> {
    if let Some(source_plural_info) = get_source_plural_info(source_message) {
        // Use source for msgid, target for msgstr
        let (source_msgid, source_msgid_plural, _) = create_po_plural_forms(&source_plural_info);
        let (_, _, target_msgstr_forms) = create_po_plural_forms(plural_info);
        Ok((source_msgid, source_msgid_plural, target_msgstr_forms))
    } else {
        // Use target for both
        Ok(create_po_plural_forms(plural_info))
    }
}

fn get_source_plural_info(source_message: Option<&FluentMessage>) -> Option<PluralInfo> {
    source_message
        .and_then(|sm| sm.value.as_ref())
        .and_then(|sp| extract_plural_info(sp))
}

fn write_multiline_fluent_message(content: &mut String, key: &str, msgstr: &str) {
    let lines: Vec<&str> = msgstr.lines().collect();
    
    if lines.is_empty() {
        content.push_str(&format!("{} =\n", key));
        return;
    }
    
    // First line goes on the same line as the key = 
    content.push_str(&format!("{} = {}\n", key, unescape_fluent_value(lines[0])));
    
    // Subsequent lines are indented
    for line in &lines[1..] {
        content.push_str(&format!("    {}\n", unescape_fluent_value(line)));
    }
}

fn write_singleline_fluent_message(content: &mut String, key: &str, msgstr: &str) {
    content.push_str(&format!("{} = {}\n", key, unescape_fluent_value(msgstr)));
}

fn process_plural_form(content: &mut String, msgstr: &str, mut has_other: bool) -> bool {
            let cleaned_msgstr = unescape_fluent_value(msgstr);
            
            if let Some(colon_pos) = cleaned_msgstr.find(':') {
                let (marker, text) = cleaned_msgstr.split_at(colon_pos);
                let text = &text[1..]; // Remove the ':' character
                
                match marker {
            FLUENT_ZERO_MARKER => {
                        content.push_str(&format!("    [0] {}\n", text));
                    }
            FLUENT_ONE_MARKER => {
                        content.push_str(&format!("    [one] {}\n", text));
                    }
            FLUENT_OTHER_MARKER => {
                        if !has_other {
                            content.push_str(&format!("   *[other] {}\n", text));
                            has_other = true;
                        }
                    }
            other_marker if other_marker.starts_with(FLUENT_MARKER_PREFIX) => {
                has_other = handle_other_fluent_marker(content, other_marker, text, has_other);
                    }
                    _ => {
                // Fallback for malformed markers
                        if !has_other {
                            content.push_str(&format!("   *[other] {}\n", text));
                            has_other = true;
                        }
                    }
                }
    } else if !has_other {
        // Fallback for messages without markers
                    content.push_str(&format!("   *[other] {}\n", cleaned_msgstr));
                    has_other = true;
    }
    
    has_other
}

fn handle_other_fluent_marker(
    content: &mut String, 
    marker: &str, 
    text: &str, 
    mut has_other: bool
) -> bool {
    let key_part = &marker[FLUENT_MARKER_PREFIX.len()..];
    let key_lower = key_part.to_lowercase();
    
    if key_lower == "other" && !has_other {
        content.push_str(&format!("   *[other] {}\n", text));
        has_other = true;
    } else {
        content.push_str(&format!("    [{}] {}\n", key_lower, text));
    }
    
    has_other
}

fn ensure_other_form_exists(content: &mut String, message: &dyn MessageView, has_other: bool) {
    if !has_other {
        let fallback_text = message.msgid_plural().unwrap_or("");
        content.push_str(&format!("   *[other] {}\n", fallback_text));
    }
}

fn extract_plural_info(pattern: &FluentPattern) -> Option<PluralInfo> {
    for element in &pattern.elements {
        if let FluentElement::Plural { selector, variants } = element {
            let forms: Vec<(String, String)> = variants
                .iter()
                .map(|(key, variant_pattern)| {
                let text = extract_pattern_text(variant_pattern);
                    (key.clone(), text)
                })
                .collect();
            
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
    let categorized_forms = categorize_plural_forms(&plural_info.forms);
    
    let msgid = determine_msgid(&categorized_forms, &plural_info.forms);
    let msgid_plural = determine_msgid_plural(&categorized_forms, &plural_info.forms);
    let msgstr_forms = create_msgstr_forms(plural_info, &categorized_forms, &msgid_plural);
    
    (msgid, msgid_plural, msgstr_forms)
}

fn categorize_plural_forms(forms: &[(String, String)]) -> CategorizedPluralForms {
    let mut categorized = CategorizedPluralForms::default();
    
    for (key, text) in forms {
        match key.as_str() {
            "0" => categorized.zero_form = Some(text.clone()),
            "1" => { categorized.numeric_forms.insert("1".to_string(), text.clone()); },
            "one" => categorized.one_form = Some(text.clone()),
            "other" => categorized.other_form = Some(text.clone()),
            _num if key.chars().all(|c| c.is_ascii_digit()) => {
                categorized.numeric_forms.insert(key.clone(), text.clone());
            },
            _ => {} // Handle other cases like "few", "many" later if needed
        }
    }
    
    categorized
}

fn determine_msgid(categorized: &CategorizedPluralForms, all_forms: &[(String, String)]) -> String {
    categorized.one_form.as_ref()
        .or(categorized.zero_form.as_ref())
        .or_else(|| categorized.numeric_forms.get("1"))
        .or_else(|| all_forms.first().map(|(_, text)| text))
        .unwrap_or(&String::new())
        .clone()
}

fn determine_msgid_plural(categorized: &CategorizedPluralForms, all_forms: &[(String, String)]) -> String {
    categorized.other_form.as_ref()
        .or_else(|| all_forms.iter()
            .find(|(key, _)| key != "one" && key != "0" && key != "1")
            .map(|(_, text)| text))
        .unwrap_or(&String::new())
        .clone()
}

fn create_msgstr_forms(
    plural_info: &PluralInfo,
    categorized: &CategorizedPluralForms, 
    msgid_plural: &str
) -> Vec<String> {
    let mut msgstr_forms = Vec::new();
    
    // Add forms with markers to preserve original structure
    if let Some(ref zero) = categorized.zero_form {
        msgstr_forms.push(format!("{}:{}", FLUENT_ZERO_MARKER, zero));
    }
    
    if let Some(ref one) = categorized.one_form {
        msgstr_forms.push(format!("{}:{}", FLUENT_ONE_MARKER, one));
    }
    
    if let Some(ref other) = categorized.other_form {
        msgstr_forms.push(format!("{}:{}", FLUENT_OTHER_MARKER, other));
    }
    
    // If no standard forms, preserve all original forms
    if msgstr_forms.is_empty() {
        for (key, text) in &plural_info.forms {
            msgstr_forms.push(format!("{}{}:{}", FLUENT_MARKER_PREFIX, key.to_uppercase(), text));
        }
    }
    
    // PO requires at least 2 forms
    while msgstr_forms.len() < 2 {
        msgstr_forms.push(format!("{}:{}", FLUENT_OTHER_MARKER, msgid_plural));
    }
    
    msgstr_forms
}

fn extract_selector_from_comments(comments: &str) -> Option<String> {
    for line in comments.lines() {
        if let Some(selector_part) = line.trim().strip_prefix(FLUENT_SELECTOR_PREFIX) {
            return Some(selector_part.trim().to_string());
        }
    }
    None
}

fn unescape_fluent_value(value: &str) -> String {
    // Reverse the escaping that was applied during Fluent->PO conversion
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
    fn test_fluent_to_po_catalog() {
        // Test both simple and complex messages in one test
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
            // Simple message
            FluentMessage {
                id: "hello".to_string(),
                value: Some(FluentPattern {
                    elements: vec![FluentElement::Text("Hello World".to_string())],
                }),
                attributes: HashMap::new(),
                comment: Some("A simple greeting".to_string()),
            },
            // Message with variables
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
            // Plural message
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
        
        let result = fluent_to_po_catalog(fluent_resource, "en", None);
        assert!(result.is_ok());
        
        let catalog = result.unwrap();
        assert_eq!(catalog.messages().count(), 3);
        
        // Check metadata
        assert_eq!(catalog.metadata.language, "en");
        assert_eq!(catalog.metadata.content_type, DEFAULT_CHARSET);
        
        // Verify plural message
        let plural_message = catalog.messages().find(|m| m.msgctxt() == "item_count").unwrap();
        assert!(plural_message.is_plural());
        assert!(plural_message.comments().contains("FLUENT_SELECTOR:count"));
    }

    #[test]
    fn test_po_catalog_to_fluent() {
        // Test both simple and plural messages
        let mut metadata = CatalogMetadata::default();
        metadata.language = "en".to_string();
        let mut catalog = Catalog::new(metadata);
        
        // Simple message
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("hello".to_string())
            .with_msgid("Hello World".to_string())
            .with_msgstr("Hello World".to_string())
            .with_comments("A simple greeting".to_string());
        catalog.append_or_update(msg_builder.done());
        
        // Plural message
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
        catalog.append_or_update(msg_builder.done());
        
        let result = po_catalog_to_fluent(catalog);
        assert!(result.is_ok());
        
        let fluent_content = result.unwrap();
        assert!(fluent_content.contains("# A simple greeting"));
        assert!(fluent_content.contains("hello = Hello World"));
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
        
        // Test with a simple pattern (no plurals)
        let simple_pattern = FluentPattern {
            elements: vec![FluentElement::Text("Hello World".to_string())],
        };
        
        let plural_info = extract_plural_info(&simple_pattern);
        assert!(plural_info.is_none());
    }

    #[test]
    fn test_create_po_plural_forms() {
        // Test with standard forms
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
        
        // Test with numeric forms
        let plural_info_numeric = PluralInfo {
            selector: "count".to_string(),
            forms: vec![
                ("0".to_string(), "no items".to_string()),
                ("1".to_string(), "one item".to_string()),
                ("other".to_string(), "many items".to_string()),
            ],
        };
        
        let (msgid, msgid_plural, msgstr_forms) = create_po_plural_forms(&plural_info_numeric);
        
        assert_eq!(msgid, "no items"); // Prefers [0] form for msgid
        assert_eq!(msgid_plural, "many items");
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
    fn test_write_and_parse_po_file() {
        let temp_dir = tempdir().unwrap();
        let po_path = temp_dir.path().join("test.po");
        
        // Create a catalog
        let mut metadata = CatalogMetadata::default();
        metadata.language = "en".to_string();
        metadata.content_type = DEFAULT_CHARSET.to_string();
        
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

    #[test]
    fn test_po_to_fluent_empty_msgstr_edge_cases() {
        // Test that empty msgstr values are omitted and don't create extra empty lines
        let mut metadata = CatalogMetadata::default();
        metadata.language = "en".to_string();
        metadata.content_type = "text/plain; charset=UTF-8".to_string();
        
        let mut catalog = Catalog::new(metadata);
        
        // Add message with empty msgstr
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("empty-key".to_string())
            .with_msgid("source text".to_string())
            .with_msgstr("".to_string());
        catalog.append_or_update(msg_builder.done());
        
        // Add message with whitespace-only msgstr
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("whitespace-key".to_string())
            .with_msgid("source text 2".to_string())
            .with_msgstr("   ".to_string());
        catalog.append_or_update(msg_builder.done());
        
        // Add valid translated messages to test line handling
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("first".to_string())
            .with_msgid("first message".to_string())
            .with_msgstr("translated first".to_string());
        catalog.append_or_update(msg_builder.done());
        
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("second".to_string())
            .with_msgid("second message".to_string())
            .with_msgstr("translated second".to_string());
        catalog.append_or_update(msg_builder.done());
        
        // Convert to Fluent
        let result = po_catalog_to_fluent(catalog);
        assert!(result.is_ok());
        
        let fluent_content = result.unwrap();
        
        // Empty and whitespace-only entries should be omitted
        assert!(!fluent_content.contains("empty-key"));
        assert!(!fluent_content.contains("whitespace-key"));
        
        // Valid entries should be present
        assert!(fluent_content.contains("first = translated first"));
        assert!(fluent_content.contains("second = translated second"));
        
        // Should not contain any invalid "key = " entries
        assert!(!fluent_content.contains("= \n"));
        assert!(!fluent_content.contains("=  "));
        
        // Check that there are no excessive empty lines
        let lines: Vec<&str> = fluent_content.lines().collect();
        let mut consecutive_empty = 0;
        
        for line in lines {
            if line.trim().is_empty() {
                consecutive_empty += 1;
                assert!(consecutive_empty <= 1, "Found {} consecutive empty lines", consecutive_empty);
            } else {
                consecutive_empty = 0;
            }
        }
    }
}
