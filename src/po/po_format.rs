use crate::{
    po::cldr_plural_rules::{
        CLDR_OTHER_CATEGORY, DEFAULT_PLURAL_FORMS, get_plural_forms_for_locale, is_other_category,
        is_singular_category, map_cldr_categories_to_po_indices_for_locale,
        map_po_indices_to_cldr_categories_for_locale,
    },
    shared::{
        error::ConversionError,
        fluent_data::{
            FluentElement, FluentMessage, FluentPattern, FluentResource, extract_pattern_text,
            parse_string_value_as_fluent_pattern,
        },
    },
};
use anyhow::Result;
use polib::{
    catalog::Catalog,
    message::{Message as PoMessage, MessageView},
    metadata::CatalogMetadata,
    po_file,
};
use std::{collections::HashMap, path::Path};

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
const FLUENT_SELECTOR_PREFIX: &str = "FLUENT_SELECTOR:";

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

    // Catch panics from malformed PO content that might cause polib to panic
    let parse_result = std::panic::catch_unwind(|| po_file::parse(temp_file.path()));

    match parse_result {
        Ok(result) => result.map_err(|e| {
            ConversionError::InputFileParseError(format!("Failed to parse PO file: {}", e)).into()
        }),
        Err(_) => Err(ConversionError::InputFileParseError(
            "Failed to parse PO file: malformed content caused parser panic".to_string(),
        )
        .into()),
    }
}

/// Preprocess PO content to ensure all required metadata fields are present
pub fn preprocess_po_content(content: &str) -> Result<String> {
    let mut processor = PoContentProcessor::new(content);
    processor.process()
}

/// Convert a Fluent resource to a PO catalog
pub fn fluent_to_po_catalog(
    target_resource: FluentResource,
    locale: &str,
    source_resource: Option<FluentResource>,
) -> Result<Catalog> {
    let metadata = create_po_metadata(locale);
    let mut catalog = Catalog::new(metadata);

    let source_lookup = create_source_message_lookup(source_resource.as_ref());

    for message in target_resource.messages {
        let source_message = source_lookup.get(&message.id).copied();
        convert_fluent_message_to_po(&mut catalog, &message, source_message, locale)?;
    }

    Ok(catalog)
}

/// Convert a PO catalog to FluentResource
pub fn po_catalog_to_fluent(catalog: Catalog) -> Result<FluentResource> {
    let mut fluent_messages = Vec::new();

    // Convert each PO message to Fluent message
    for message in catalog.messages() {
        let key = generate_fluent_key_from_message(message);

        // Convert PO message to Fluent message
        let fluent_message = if message.is_plural() {
            convert_plural_po_message_to_fluent_message(&key, message, &catalog.metadata.language)?
        } else {
            convert_singular_po_message_to_fluent_message(&key, message)?
        };

        // Skip empty messages (untranslated entries)
        if let Some(fluent_message) = fluent_message {
            fluent_messages.push(fluent_message);
        }
    }

    Ok(FluentResource {
        messages: fluent_messages,
    })
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
        self.result.append(&mut self.header_lines);
    }

    fn add_missing_metadata_fields(&mut self) {
        for &field in REQUIRED_METADATA_FIELDS {
            if !self.found_metadata.contains_key(field) {
                let default_value = get_default_metadata_value(field);
                self.header_lines
                    .push(format!("\"{}: {}\\n\"", field, default_value));
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

// =============================================================================
// Core Conversion Functions
// =============================================================================

fn convert_fluent_message_to_po(
    catalog: &mut Catalog,
    message: &FluentMessage,
    source_message: Option<&FluentMessage>,
    locale: &str,
) -> Result<()> {
    // Convert main message value
    if let Some(pattern) = &message.value {
        convert_main_message_value(catalog, message, pattern, source_message, locale)?;
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
    locale: &str,
) -> Result<()> {
    let target_text = extract_pattern_text(pattern);
    let comments = message.comment.as_ref().unwrap_or(&String::new()).clone();

    if let Some(plural_info) = extract_plural_info(pattern) {
        convert_plural_message(
            catalog,
            message,
            &plural_info,
            source_message,
            &comments,
            locale,
        )?;
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
    locale: &str,
) -> Result<()> {
    // Simplified logic: directly get plural forms without multiple wrapper functions
    let (msgid, msgid_plural, msgstr_forms) = if let Some(source_msg) = source_message {
        // Check if source has plural info
        if let Some(source_value) = &source_msg.value {
            if let Some(source_plural_info) = extract_plural_info(source_value) {
                // Use source for msgid, target for msgstr
                let (source_msgid, source_msgid_plural, _) =
                    create_po_plural_forms(&source_plural_info, locale);
                let (_, _, target_msgstr_forms) = create_po_plural_forms(plural_info, locale);
                (source_msgid, source_msgid_plural, target_msgstr_forms)
            } else {
                // Source doesn't have plural, use target for both
                create_po_plural_forms(plural_info, locale)
            }
        } else {
            // Source has no value, use target for both
            create_po_plural_forms(plural_info, locale)
        }
    } else {
        // No source message, use target for both
        create_po_plural_forms(plural_info, locale)
    };

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
            .map(extract_pattern_text);

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

fn convert_singular_po_message_to_fluent_message(
    key: &str,
    message: &dyn MessageView,
) -> Result<Option<FluentMessage>> {
    let msgstr = message.msgstr()?;

    // Skip entries with empty msgstr values as they represent untranslated strings
    if msgstr.trim().is_empty() {
        return Ok(None);
    }

    // Simply try to parse the unescaped content as a Fluent message value
    let unescaped_msgstr = unescape_fluent_value(msgstr);
    let pattern = parse_string_value_as_fluent_pattern(key, &unescaped_msgstr);

    // Extract comments (excluding FLUENT_SELECTOR comments which are handled separately)
    let comment = extract_filtered_comments(message.comments());

    Ok(Some(FluentMessage {
        id: key.to_string(),
        value: Some(pattern),
        attributes: HashMap::new(),
        comment,
    }))
}

fn convert_plural_po_message_to_fluent_message(
    key: &str,
    message: &dyn MessageView,
    locale: &str,
) -> Result<Option<FluentMessage>> {
    let msgstr_plural = message.msgstr_plural()?;
    let selector =
        extract_selector_from_comments(message.comments()).unwrap_or_else(|| "count".to_string());

    // Build variants directly using Fluent structures
    let mut variants = HashMap::new();
    let mut has_other = false;

    // Map standard PO msgstr[n] entries to Fluent plural forms using CLDR mapping
    let cldr_categories = map_po_indices_to_cldr_categories_for_locale(msgstr_plural.len(), locale);

    for (index, msgstr) in msgstr_plural.iter().enumerate() {
        let cleaned_msgstr = unescape_fluent_value(msgstr);

        if !cleaned_msgstr.trim().is_empty() {
            // Parse each variant text as a Fluent pattern
            let variant_pattern = parse_string_value_as_fluent_pattern(key, &cleaned_msgstr);

            // Map PO plural index to Fluent variant key using CLDR categories
            let variant_key = if index < cldr_categories.len() {
                let cldr_key = cldr_categories[index];
                if is_other_category(cldr_key) {
                    has_other = true;
                }
                cldr_key.to_string()
            } else {
                // Additional forms for complex languages - use numeric fallback
                format!("{}", index)
            };

            variants.insert(variant_key, variant_pattern);
        }
    }

    // Ensure we have at least an 'other' form (required by Fluent)
    if !has_other && !variants.is_empty() {
        // If we don't have an explicit "other" form, use the last available form
        if let Some((last_key, last_pattern)) = variants.iter().last() {
            if !is_other_category(last_key) {
                variants.insert(CLDR_OTHER_CATEGORY.to_string(), last_pattern.clone());
                has_other = true;
            }
        }
    }

    if !has_other {
        let fallback_text = message.msgid_plural().unwrap_or("items");
        variants.insert(
            CLDR_OTHER_CATEGORY.to_string(),
            FluentPattern {
                elements: vec![FluentElement::Text(fallback_text.to_string())],
            },
        );
    }

    if variants.is_empty() {
        return Ok(None);
    }

    // Create the plural pattern directly using Fluent structures
    let pattern = FluentPattern {
        elements: vec![FluentElement::Plural { selector, variants }],
    };

    let comment = extract_filtered_comments(message.comments());

    Ok(Some(FluentMessage {
        id: key.to_string(),
        value: Some(pattern),
        attributes: HashMap::new(),
        comment,
    }))
}

/// Extract comments excluding FLUENT_SELECTOR directives
fn extract_filtered_comments(comments: &str) -> Option<String> {
    if comments.is_empty() {
        return None;
    }

    let filtered_comments: Vec<&str> = comments
        .lines()
        .filter(|line| !line.trim().starts_with(FLUENT_SELECTOR_PREFIX))
        .collect();

    if filtered_comments.is_empty() {
        None
    } else {
        Some(filtered_comments.join("\n"))
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn create_po_metadata(locale: &str) -> CatalogMetadata {
    let project_id_version = option_env!("CARGO_PKG_VERSION")
        .map(|v| format!("wordpress-rs {}", v))
        .unwrap_or_else(|| "wordpress-rs".to_string());

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M%z").to_string();

    // Get the correct CLDR plural forms for the locale
    let plural_forms = get_plural_forms_for_locale(locale);

    // Construct the metadata string with proper plural forms
    let metadata_str = format!(
        "Project-Id-Version: {}\n\
         POT-Creation-Date: {}\n\
         PO-Revision-Date: {}\n\
         Last-Translator: \n\
         Language-Team: \n\
         MIME-Version: {}\n\
         Content-Type: {}\n\
         Content-Transfer-Encoding: {}\n\
         Language: {}\n\
         Plural-Forms: {}\n",
        project_id_version,
        now,
        now,
        DEFAULT_MIME_VERSION,
        DEFAULT_CHARSET,
        DEFAULT_ENCODING,
        locale,
        plural_forms
    );

    // Parse the metadata string to create CatalogMetadata with proper plural rules
    CatalogMetadata::parse(&metadata_str).expect("Failed to parse metadata string")
}

fn create_source_message_lookup(
    source_resource: Option<&FluentResource>,
) -> HashMap<String, &FluentMessage> {
    source_resource
        .map(|sr| {
            sr.messages
                .iter()
                .map(|msg| (msg.id.clone(), msg))
                .collect()
        })
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

fn generate_fluent_key_from_message(message: &dyn MessageView) -> String {
    if !message.msgctxt().is_empty() {
        message.msgctxt().to_string()
    } else {
        message
            .msgid()
            .replace(' ', "-")
            .replace('"', "")
            .to_lowercase()
    }
}

fn get_source_text_or_target(source_message: Option<&FluentMessage>, target_text: &str) -> String {
    source_message
        .and_then(|sm| sm.value.as_ref())
        .map(extract_pattern_text)
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
                    forms,
                });
            }
        }
    }
    None
}

fn create_po_plural_forms(plural_info: &PluralInfo, locale: &str) -> (String, String, Vec<String>) {
    // Use CLDR mapping for msgstr forms (already implemented and working well)
    let msgstr_forms = map_cldr_categories_to_po_indices_for_locale(&plural_info.forms, locale);

    // Simplified msgid/msgid_plural selection logic
    let msgid = find_singular_form(&plural_info.forms);
    let msgid_plural = find_plural_form(&plural_info.forms);

    (msgid, msgid_plural, msgstr_forms)
}

// Simplified helper functions for picking msgid/msgid_plural from Fluent forms
fn find_singular_form(forms: &[(String, String)]) -> String {
    // Look for standard singular forms in order of preference
    if let Some((_, text)) = forms.iter().find(|(key, _)| is_singular_category(key)) {
        return text.clone();
    }

    // Fallback to first available form
    forms
        .first()
        .map(|(_, text)| text.clone())
        .unwrap_or_default()
}

fn find_plural_form(forms: &[(String, String)]) -> String {
    // Look for "other" first (universal plural form)
    if let Some((_, text)) = forms.iter().find(|(key, _)| is_other_category(key)) {
        return text.clone();
    }

    // Find any form that's not singular
    if let Some((_, text)) = forms.iter().find(|(key, _)| !is_singular_category(key)) {
        return text.clone();
    }

    // Final fallback - use second form if available, otherwise first
    forms
        .get(1)
        .or_else(|| forms.first())
        .map(|(_, text)| text.clone())
        .unwrap_or_default()
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
        variants.insert(
            "one".to_string(),
            FluentPattern {
                elements: vec![
                    FluentElement::Variable("count".to_string()),
                    FluentElement::Text(" item".to_string()),
                ],
            },
        );
        variants.insert(
            "other".to_string(),
            FluentPattern {
                elements: vec![
                    FluentElement::Variable("count".to_string()),
                    FluentElement::Text(" items".to_string()),
                ],
            },
        );

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
        let plural_message = catalog
            .messages()
            .find(|m| m.msgctxt() == "item_count")
            .unwrap();
        assert!(plural_message.is_plural());
        assert!(plural_message.comments().contains("FLUENT_SELECTOR:count"));
    }

    #[test]
    fn test_po_catalog_to_fluent() {
        // Test both simple and plural messages
        let metadata = CatalogMetadata {
            language: "en".to_string(),
            ..Default::default()
        };
        let mut catalog = Catalog::new(metadata);

        // Simple message
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("hello".to_string())
            .with_msgid("Hello World".to_string())
            .with_msgstr("Hello World".to_string())
            .with_comments("A simple greeting".to_string());
        catalog.append_or_update(msg_builder.done());

        // Plural message using standard PO format (no FLUENT_ markers)
        let mut msg_builder = PoMessage::build_plural();
        msg_builder
            .with_msgctxt("item_count".to_string())
            .with_msgid("{$count} item".to_string())
            .with_msgid_plural("{$count} items".to_string())
            .with_msgstr_plural(vec![
                "{$count} item".to_string(),
                "{$count} items".to_string(),
            ])
            .with_comments("FLUENT_SELECTOR:count\nItem counter".to_string());
        catalog.append_or_update(msg_builder.done());

        let result = po_catalog_to_fluent(catalog);
        assert!(result.is_ok());

        let fluent_resource = result.unwrap();

        // Convert to string for testing using the same logic as converter.rs
        let fluent_content = fluent_resource.to_source();

        assert!(fluent_content.contains("# A simple greeting"));
        assert!(fluent_content.contains("hello = Hello World"));
        assert!(fluent_content.contains("item_count ="));
        assert!(fluent_content.contains("{ $count ->"));
        assert!(fluent_content.contains("[one] { $count } item"));
        assert!(fluent_content.contains("*[other] { $count } items"));
        assert!(fluent_content.contains("# Item counter"));
    }

    #[test]
    fn test_extract_plural_info() {
        // Test with a plural pattern
        let mut variants = HashMap::new();
        variants.insert(
            "one".to_string(),
            FluentPattern {
                elements: vec![FluentElement::Text("one item".to_string())],
            },
        );
        variants.insert(
            "other".to_string(),
            FluentPattern {
                elements: vec![FluentElement::Text("many items".to_string())],
            },
        );

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

        let (msgid, msgid_plural, msgstr_forms) = create_po_plural_forms(&plural_info, "en");

        assert_eq!(msgid, "{$count} item");
        assert_eq!(msgid_plural, "{$count} items");
        assert_eq!(msgstr_forms.len(), 2);
        // New standard PO format without FLUENT_ markers
        assert!(msgstr_forms.contains(&"{$count} item".to_string()));
        assert!(msgstr_forms.contains(&"{$count} items".to_string()));

        // Test with numeric forms
        let plural_info_numeric = PluralInfo {
            selector: "count".to_string(),
            forms: vec![
                ("0".to_string(), "no items".to_string()),
                ("1".to_string(), "one item".to_string()),
                ("other".to_string(), "many items".to_string()),
            ],
        };

        let (msgid, msgid_plural, msgstr_forms) =
            create_po_plural_forms(&plural_info_numeric, "en");

        assert_eq!(msgid, "one item");
        assert_eq!(msgid_plural, "many items");
        assert!(msgstr_forms.len() >= 2);
        // New standard PO format without FLUENT_ markers
        assert!(msgstr_forms.contains(&"no items".to_string()));
        assert!(msgstr_forms.contains(&"many items".to_string()));
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
        assert_eq!(
            unescape_fluent_value("Path\\\\to\\\\file"),
            "Path\\to\\file"
        );
        assert_eq!(unescape_fluent_value("Normal text"), "Normal text");
        assert_eq!(unescape_fluent_value("\\{$var\\} text"), "{$var} text");
    }

    #[test]
    fn test_write_and_parse_po_file() {
        let temp_dir = tempdir().unwrap();
        let po_path = temp_dir.path().join("test.po");

        // Create a catalog
        let metadata = CatalogMetadata {
            language: "en".to_string(),
            content_type: DEFAULT_CHARSET.to_string(),
            ..Default::default()
        };

        let mut catalog = Catalog::new(metadata);

        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("test".to_string())
            .with_msgid("Test message".to_string())
            .with_msgstr("Test message".to_string());

        let message = msg_builder.done();
        catalog.append_or_update(message);

        // Write the file
        let write_result = po_file::write(&catalog, &po_path);
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
        let metadata = CatalogMetadata {
            language: "en".to_string(),
            content_type: "text/plain; charset=UTF-8".to_string(),
            ..Default::default()
        };

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

        let fluent_resource = result.unwrap();

        // Convert to string for testing
        let fluent_content = fluent_resource.to_source();

        // Empty and whitespace-only entries should be omitted
        assert!(!fluent_content.contains("empty-key"));
        assert!(!fluent_content.contains("whitespace-key"));

        // Valid entries should be present
        assert!(fluent_content.contains("first = translated first"));
        assert!(fluent_content.contains("second = translated second"));

        // Should not contain any invalid "key = " entries
        assert!(!fluent_content.contains("= \n"));
        assert!(!fluent_content.contains("=  "));

        // Check that messages are present (the formatting is now handled in converter.rs)
        assert_eq!(fluent_resource.messages.len(), 2);
    }

    #[test]
    fn test_multiline_po_to_fluent_formatting() {
        // Test that multiline PO messages are correctly formatted with proper indentation
        let metadata = CatalogMetadata {
            language: "en".to_string(),
            content_type: "text/plain".to_string(),
            ..Default::default()
        };

        let mut catalog = Catalog::new(metadata);

        // Add multiline message
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("multiline-message".to_string())
            .with_msgid("Multi line source".to_string())
            .with_msgstr("This is line one\nThis is line two\nThis is line three".to_string());
        catalog.append_or_update(msg_builder.done());

        // Convert to Fluent
        let result = po_catalog_to_fluent(catalog);
        assert!(result.is_ok());

        let fluent_resource = result.unwrap();

        // Convert to string and verify proper multiline formatting
        let fluent_content = fluent_resource.to_source();

        // The message should be formatted with proper indentation
        assert!(fluent_content.contains("multiline-message ="));
        assert!(fluent_content.contains("This is line one"));
        assert!(fluent_content.contains("    This is line two"));
        assert!(fluent_content.contains("    This is line three"));

        // Verify the generated Fluent can be parsed back without errors
        let reparsed = FluentResource::from_source(&fluent_content);
        assert!(
            reparsed.is_ok(),
            "Generated multiline Fluent should be parseable without errors"
        );

        let reparsed_resource = reparsed.unwrap();
        assert_eq!(reparsed_resource.messages.len(), 1);
        assert_eq!(reparsed_resource.messages[0].id, "multiline-message");
    }
}
