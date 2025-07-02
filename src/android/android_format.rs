use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Serde structures for XML parsing
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "resources")]
pub struct XmlResources {
    #[serde(rename = "$value", default)]
    pub items: Vec<XmlResource>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum XmlResource {
    String(XmlString),
    Plurals(XmlPlurals),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct XmlString {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(
        rename = "@translatable",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub translatable: Option<String>,

    #[serde(rename = "$text")]
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct XmlPlurals {
    #[serde(rename = "@name")]
    pub name: String,

    #[serde(rename = "item")]
    pub items: Vec<XmlPluralItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct XmlPluralItem {
    #[serde(rename = "@quantity")]
    pub quantity: String,

    #[serde(rename = "$text")]
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct AndroidString {
    pub name: String,
    pub value: String,
    pub translatable: Option<bool>,
    pub comment: Option<String>,
    pub variable_mapping: HashMap<String, String>, // Android placeholder -> Fluent variable
}

#[derive(Debug, Clone)]
pub struct AndroidPlural {
    pub name: String,
    pub items: HashMap<String, String>, // quantity -> value
    pub comment: Option<String>,
    pub variable_mapping: HashMap<String, String>,
}

#[derive(Debug)]
pub struct AndroidResources {
    pub strings: Vec<AndroidString>,
    pub plurals: Vec<AndroidPlural>,
}

impl AndroidResources {
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            plurals: Vec::new(),
        }
    }

    pub fn from_xml(xml_content: &str) -> Result<Self> {
        // Extract comments using regex before serde parsing
        let comments = extract_comments_from_xml(xml_content)?;

        // Deserialize the main structure using pure serde
        let xml_resources: XmlResources = quick_xml::de::from_str(xml_content)?;

        let mut resources = AndroidResources::new();

        // Process each resource item
        for (index, item) in xml_resources.items.iter().enumerate() {
            let comment = comments.get(&index).cloned();
            let variable_mapping = parse_variable_mapping(&comment)?;

            match item {
                XmlResource::String(xml_string) => {
                    let translatable = xml_string.translatable.as_ref().map(|t| t == "true");

                    resources.strings.push(AndroidString {
                        name: xml_string.name.clone(),
                        value: xml_string.value.clone(),
                        translatable,
                        comment,
                        variable_mapping,
                    });
                }
                XmlResource::Plurals(xml_plurals) => {
                    let mut items = HashMap::new();

                    for item in &xml_plurals.items {
                        items.insert(item.quantity.clone(), item.value.clone());
                    }

                    resources.plurals.push(AndroidPlural {
                        name: xml_plurals.name.clone(),
                        items,
                        comment,
                        variable_mapping,
                    });
                }
            }
        }

        Ok(resources)
    }

    // `serde` does not parse / write comments; use `quick_xml` for writing
    pub fn to_xml(&self) -> Result<String> {
        use quick_xml::{Writer, events::Event};

        let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);

        // Write XML declaration
        writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new(
            "1.0",
            Some("utf-8"),
            None,
        )))?;

        // Start resources element
        writer.write_event(Event::Start(quick_xml::events::BytesStart::new(
            "resources",
        )))?;

        // Write strings with comments
        for string in &self.strings {
            write_string_with_comment(&mut writer, string)?;
        }

        // Write plural strings with comments
        for plural in &self.plurals {
            write_plural_with_comment(&mut writer, plural)?;
        }

        // End resources element
        writer.write_event(Event::End(quick_xml::events::BytesEnd::new("resources")))?;

        Ok(String::from_utf8(writer.into_inner())?)
    }
}

// Write a string element with its comment using the library's proper comment support
fn write_string_with_comment(
    writer: &mut quick_xml::Writer<Vec<u8>>,
    string: &AndroidString,
) -> Result<()> {
    use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};

    // Write comment if present using the library's Event::Comment
    if let Some(comment) = &string.comment {
        let formatted_comment = format_comment_for_xml(comment);
        writer.write_event(Event::Comment(BytesText::new(&formatted_comment)))?;
    }

    // Create string element with attributes
    let mut elem = BytesStart::new("string");
    elem.push_attribute(("name", string.name.as_str()));

    // Only add translatable attribute when explicitly set to false
    if let Some(false) = string.translatable {
        elem.push_attribute(("translatable", "false"));
    }

    writer.write_event(Event::Start(elem))?;
    writer.write_event(Event::Text(BytesText::new(&string.value)))?;
    writer.write_event(Event::End(BytesEnd::new("string")))?;

    Ok(())
}

// Write a plurals element with its comment using the library's proper comment support
fn write_plural_with_comment(
    writer: &mut quick_xml::Writer<Vec<u8>>,
    plural: &AndroidPlural,
) -> Result<()> {
    use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};

    // Write comment if present using the library's Event::Comment
    if let Some(comment) = &plural.comment {
        let formatted_comment = format_comment_for_xml(comment);
        writer.write_event(Event::Comment(BytesText::new(&formatted_comment)))?;
    }

    // Create plurals element
    let mut elem = BytesStart::new("plurals");
    elem.push_attribute(("name", plural.name.as_str()));

    writer.write_event(Event::Start(elem))?;

    // Write items in consistent order
    let quantities = ["zero", "one", "two", "few", "many", "other"];
    for quantity in &quantities {
        if let Some(value) = plural.items.get(*quantity) {
            let mut item_elem = BytesStart::new("item");
            item_elem.push_attribute(("quantity", *quantity));

            writer.write_event(Event::Start(item_elem))?;
            writer.write_event(Event::Text(BytesText::new(value)))?;
            writer.write_event(Event::End(BytesEnd::new("item")))?;
        }
    }

    writer.write_event(Event::End(BytesEnd::new("plurals")))?;

    Ok(())
}

// Format a comment for XML output with proper spacing and indentation
fn format_comment_for_xml(comment: &str) -> String {
    // Add space at start, replace newlines with newline + indentation, add space at end
    format!(" {} ", comment.replace('\n', "\n     "))
}

// Extract comments from XML using proper XML parsing and associate them with resource elements
// `serde` does not parse comments; use `quick_xml` event-based parsing to extract the comments
fn extract_comments_from_xml(xml_content: &str) -> Result<HashMap<usize, String>> {
    use quick_xml::{Reader, events::Event};

    fn is_resource_element(name: &[u8]) -> bool {
        matches!(name, b"string" | b"plurals")
    }

    let mut comments = HashMap::new();
    let mut reader = Reader::from_str(xml_content);
    let mut buf = Vec::new();
    let mut pending_comment = None;
    let mut resource_index = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Comment(comment)) => {
                let comment_text = String::from_utf8_lossy(&comment);
                // Only trim leading/trailing whitespace, preserve internal structure
                let trimmed = comment_text.trim();
                if !trimmed.is_empty() {
                    pending_comment = Some(trimmed.to_string());
                }
            }
            Ok(Event::Start(element)) if is_resource_element(element.name().as_ref()) => {
                if let Some(comment) = pending_comment.take() {
                    comments.insert(resource_index, comment);
                }
                resource_index += 1;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("Error parsing XML for comments: {:?}", e)),
            _ => {} // Ignore other events
        }
        buf.clear();
    }

    Ok(comments)
}

fn parse_variable_mapping(comment: &Option<String>) -> Result<HashMap<String, String>> {
    let mut mapping = HashMap::new();

    if let Some(comment_text) = comment {
        // Parse patterns like "%s = {$message}" or "%1$d = {$num_downloads}"
        // The $ should only be present if a \d+ is also present
        let re = Regex::new(r"(%(?:\d+\$)?[sdif])\s*=\s*\{\$(\w+)\}").unwrap();

        for captures in re.captures_iter(comment_text) {
            if let (Some(placeholder), Some(variable)) = (captures.get(1), captures.get(2)) {
                mapping.insert(
                    placeholder.as_str().to_string(),
                    variable.as_str().to_string(),
                );
            }
        }
    }

    Ok(mapping)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_string() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="hello">Hello World</string>
</resources>"#;

        let resources = AndroidResources::from_xml(xml).unwrap();
        assert_eq!(resources.strings.len(), 1);
        assert_eq!(resources.strings[0].name, "hello");
        assert_eq!(resources.strings[0].value, "Hello World");
        assert_eq!(resources.strings[0].translatable, None);
    }

    #[test]
    fn test_parse_string_with_comment_mapping() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <!-- %s = {$name} -->
    <string name="greeting">Hello %s</string>
</resources>"#;

        let resources = AndroidResources::from_xml(xml).unwrap();
        assert_eq!(resources.strings.len(), 1);
        assert_eq!(resources.strings[0].name, "greeting");
        assert_eq!(resources.strings[0].value, "Hello %s");
        assert_eq!(
            resources.strings[0].variable_mapping.get("%s"),
            Some(&"name".to_string())
        );
    }

    #[test]
    fn test_parse_plurals() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <!-- %d = {$count} -->
    <plurals name="items">
        <item quantity="one">%d item</item>
        <item quantity="other">%d items</item>
    </plurals>
</resources>"#;

        let resources = AndroidResources::from_xml(xml).unwrap();
        assert_eq!(resources.plurals.len(), 1);
        assert_eq!(resources.plurals[0].name, "items");
        assert_eq!(
            resources.plurals[0].items.get("one"),
            Some(&"%d item".to_string())
        );
        assert_eq!(
            resources.plurals[0].items.get("other"),
            Some(&"%d items".to_string())
        );
        assert_eq!(
            resources.plurals[0].variable_mapping.get("%d"),
            Some(&"count".to_string())
        );
    }

    #[test]
    fn test_parse_translatable_false() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="reference" translatable="false">@string/other</string>
</resources>"#;

        let resources = AndroidResources::from_xml(xml).unwrap();
        assert_eq!(resources.strings.len(), 1);
        assert_eq!(resources.strings[0].translatable, Some(false));
    }

    #[test]
    fn test_parse_variable_mapping_regex() {
        let comment = "%s = {$name}, %1$d = {$count}";
        let mapping = parse_variable_mapping(&Some(comment.to_string())).unwrap();

        assert_eq!(mapping.get("%s"), Some(&"name".to_string()));
        assert_eq!(mapping.get("%1$d"), Some(&"count".to_string()));
    }

    #[test]
    fn test_parse_variable_mapping_regex_edge_cases() {
        // Test various Android placeholder formats
        let comment = "%s = {$name}, %d = {$count}, %2$s = {$message}, %1$f = {$price}";
        let mapping = parse_variable_mapping(&Some(comment.to_string())).unwrap();

        assert_eq!(mapping.get("%s"), Some(&"name".to_string()));
        assert_eq!(mapping.get("%d"), Some(&"count".to_string()));
        assert_eq!(mapping.get("%2$s"), Some(&"message".to_string()));
        assert_eq!(mapping.get("%1$f"), Some(&"price".to_string()));

        // Test that invalid patterns are not matched
        let invalid_comment = "%$ = {$invalid}, %abc = {$wrong}";
        let invalid_mapping = parse_variable_mapping(&Some(invalid_comment.to_string())).unwrap();
        assert!(invalid_mapping.is_empty());
    }

    #[test]
    fn test_generate_xml() {
        let mut resources = AndroidResources::new();

        let mut variable_mapping = HashMap::new();
        variable_mapping.insert("%s".to_string(), "name".to_string());

        resources.strings.push(AndroidString {
            name: "greeting".to_string(),
            value: "Hello %s".to_string(),
            translatable: Some(true),
            comment: Some("%s = {$name}".to_string()),
            variable_mapping,
        });

        let xml = resources.to_xml().unwrap();
        assert!(xml.contains(r#"<string name="greeting">Hello %s</string>"#));
        assert!(xml.contains("<!-- %s = {$name} -->"));
    }

    #[test]
    fn test_unicode_escape() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="price">\u0024%1$.2f/week</string>
</resources>"#;

        let resources = AndroidResources::from_xml(xml).unwrap();
        assert_eq!(resources.strings[0].value, r"\u0024%1$.2f/week");
    }

    #[test]
    fn test_comment_extraction() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<resources>
    <!-- This is a general comment -->
    
    <!-- Variable mapping: %s = {$name} -->
    <string name="greeting">Hello %s</string>
    
    <!-- No variable mapping needed -->
    <string name="simple">Simple text</string>
    
    <!-- Multi-line comment
         with variable mapping: %d = {$count} -->
    <plurals name="items">
        <item quantity="one">%d item</item>
        <item quantity="other">%d items</item>
    </plurals>
    
    <!-- Some comment not associated with anything -->
</resources>"#;

        let resources = AndroidResources::from_xml(xml).unwrap();

        // Check that we properly extracted 2 strings and 1 plural
        assert_eq!(resources.strings.len(), 2);
        assert_eq!(resources.plurals.len(), 1);

        // Check comment association
        assert!(resources.strings[0].comment.is_some());
        assert_eq!(
            resources.strings[0].comment.as_ref().unwrap(),
            "Variable mapping: %s = {$name}"
        );

        assert!(resources.strings[1].comment.is_some());
        assert_eq!(
            resources.strings[1].comment.as_ref().unwrap(),
            "No variable mapping needed"
        );

        assert!(resources.plurals[0].comment.is_some());
        assert!(
            resources.plurals[0]
                .comment
                .as_ref()
                .unwrap()
                .contains("Multi-line comment")
        );

        // Check variable mapping was extracted correctly
        assert_eq!(
            resources.strings[0].variable_mapping.get("%s"),
            Some(&"name".to_string())
        );
        assert_eq!(
            resources.plurals[0].variable_mapping.get("%d"),
            Some(&"count".to_string())
        );
    }

    #[test]
    fn test_multiline_comment_formatting() {
        let mut resources = AndroidResources::new();

        // Create a resource with a multiline comment
        resources.strings.push(AndroidString {
            name: "long-description".to_string(),
            value: "This is a longer description".to_string(),
            translatable: None,
            comment: Some("Multi-line string with indentation\nwith a multi-line comment\nas comments also are being parsed".to_string()),
            variable_mapping: HashMap::new(),
        });

        let xml = resources.to_xml().unwrap();

        // Check that the comment has proper spacing and indentation
        assert!(xml.contains("<!-- Multi-line string with indentation"));
        assert!(xml.contains("     with a multi-line comment"));
        assert!(xml.contains("     as comments also are being parsed -->"));

        // Verify it can round-trip correctly
        let parsed_resources = AndroidResources::from_xml(&xml).unwrap();
        assert_eq!(parsed_resources.strings.len(), 1);
        assert!(parsed_resources.strings[0].comment.is_some());
        let comment = parsed_resources.strings[0].comment.as_ref().unwrap();
        assert!(comment.contains("Multi-line string with indentation"));
        assert!(comment.contains("with a multi-line comment"));
        assert!(comment.contains("as comments also are being parsed"));
    }
}
