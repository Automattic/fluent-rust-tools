use anyhow::Result;
use quick_xml::{events::Event, Reader, Writer};
use regex::Regex;
use std::collections::HashMap;

// Constants for XML element and attribute names
const XML_STRING: &[u8] = b"string";
const XML_PLURALS: &[u8] = b"plurals";
const XML_ITEM: &[u8] = b"item";
const ATTR_NAME: &[u8] = b"name";
const ATTR_TRANSLATABLE: &[u8] = b"translatable";
const ATTR_QUANTITY: &[u8] = b"quantity";

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
        let mut reader = Reader::from_str(xml_content);
        reader.trim_text(true);

        let mut resources = AndroidResources::new();
        let mut current_comment = None;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    match e.name().as_ref() {
                        XML_STRING => {
                            let string_item = parse_string_element(&mut reader, e, &current_comment)?;
                            resources.strings.push(string_item);
                            current_comment = None;
                        }
                        XML_PLURALS => {
                            let plural_item = parse_plurals_element(&mut reader, e, &current_comment)?;
                            resources.plurals.push(plural_item);
                            current_comment = None;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Comment(comment)) => {
                    current_comment = Some(String::from_utf8_lossy(&comment).trim().to_string());
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(e.into()),
                _ => {}
            }
            buf.clear();
        }

        Ok(resources)
    }

    pub fn to_xml(&self) -> Result<String> {
        let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
        
        // Write XML declaration
        writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new("1.0", Some("utf-8"), None)))?;
        
        // Start resources element
        writer.write_event(Event::Start(quick_xml::events::BytesStart::new("resources")))?;

        // Write strings and plurals
        for string in &self.strings {
            write_string_to_xml(&mut writer, string)?;
        }
        for plural in &self.plurals {
            write_plural_to_xml(&mut writer, plural)?;
        }

        // End resources element
        writer.write_event(Event::End(quick_xml::events::BytesEnd::new("resources")))?;

        Ok(String::from_utf8(writer.into_inner())?)
    }
}

// Helper function to extract attribute value by name
fn get_attribute_value(element: &quick_xml::events::BytesStart, attr_name: &[u8]) -> Option<String> {
    element.attributes()
        .filter_map(|attr| attr.ok())
        .find(|attr| attr.key.as_ref() == attr_name)
        .map(|attr| String::from_utf8_lossy(&attr.value).to_string())
}

// Helper function to read text content until end tag
fn read_text_content(reader: &mut Reader<&[u8]>, end_tag: &[u8]) -> Result<String> {
    let mut buf = Vec::new();
    let mut content = String::new();
    
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(text)) => {
                content.push_str(&String::from_utf8_lossy(&text));
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == end_tag => break,
            Ok(_) => {}
            Err(e) => return Err(e.into()),
        }
        buf.clear();
    }
    
    Ok(content)
}

fn parse_string_element(
    reader: &mut Reader<&[u8]>,
    start_element: &quick_xml::events::BytesStart,
    comment: &Option<String>,
) -> Result<AndroidString> {
    let name = get_attribute_value(start_element, ATTR_NAME)
        .unwrap_or_default();
    
    let translatable = get_attribute_value(start_element, ATTR_TRANSLATABLE)
        .map(|val| val == "true");

    let value = read_text_content(reader, XML_STRING)?;
    let variable_mapping = parse_variable_mapping(comment)?;

    Ok(AndroidString {
        name,
        value,
        translatable,
        comment: comment.clone(),
        variable_mapping,
    })
}

fn parse_plurals_element(
    reader: &mut Reader<&[u8]>,
    start_element: &quick_xml::events::BytesStart,
    comment: &Option<String>,
) -> Result<AndroidPlural> {
    let name = get_attribute_value(start_element, ATTR_NAME)
        .unwrap_or_default();

    let mut items = HashMap::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == XML_ITEM => {
                let (quantity, value) = parse_plural_item(reader, e)?;
                items.insert(quantity, value);
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == XML_PLURALS => break,
            Ok(_) => {}
            Err(e) => return Err(e.into()),
        }
        buf.clear();
    }

    let variable_mapping = parse_variable_mapping(comment)?;

    Ok(AndroidPlural {
        name,
        items,
        comment: comment.clone(),
        variable_mapping,
    })
}

fn parse_plural_item(
    reader: &mut Reader<&[u8]>,
    start_element: &quick_xml::events::BytesStart,
) -> Result<(String, String)> {
    let quantity = get_attribute_value(start_element, ATTR_QUANTITY)
        .unwrap_or_default();

    let value = read_text_content(reader, XML_ITEM)?;

    Ok((quantity, value))
}

fn parse_variable_mapping(comment: &Option<String>) -> Result<HashMap<String, String>> {
    let mut mapping = HashMap::new();
    
    if let Some(comment_text) = comment {
        // Parse patterns like "%s = {$message}" or "%1$d = {$num_downloads}"
        let re = Regex::new(r"(%\d*\$?[sdif])\s*=\s*\{\$(\w+)\}").unwrap();
        
        for captures in re.captures_iter(comment_text) {
            if let (Some(placeholder), Some(variable)) = (captures.get(1), captures.get(2)) {
                mapping.insert(placeholder.as_str().to_string(), variable.as_str().to_string());
            }
        }
    }
    
    Ok(mapping)
}

fn write_string_to_xml(writer: &mut Writer<Vec<u8>>, string: &AndroidString) -> Result<()> {
    // Write comment if present
    if let Some(comment) = &string.comment {
        writer.write_event(Event::Comment(quick_xml::events::BytesText::new(comment)))?;
    }

    // Create string element with attributes
    let mut elem = quick_xml::events::BytesStart::new("string");
    elem.push_attribute(("name", string.name.as_str()));
    
    if let Some(false) = string.translatable {
        elem.push_attribute(("translatable", "false"));
    }

    writer.write_event(Event::Start(elem))?;
    writer.write_event(Event::Text(quick_xml::events::BytesText::new(&string.value)))?;
    writer.write_event(Event::End(quick_xml::events::BytesEnd::new("string")))?;

    Ok(())
}

fn write_plural_to_xml(writer: &mut Writer<Vec<u8>>, plural: &AndroidPlural) -> Result<()> {
    // Write comment if present
    if let Some(comment) = &plural.comment {
        writer.write_event(Event::Comment(quick_xml::events::BytesText::new(comment)))?;
    }

    // Create plurals element
    let mut elem = quick_xml::events::BytesStart::new("plurals");
    elem.push_attribute(("name", plural.name.as_str()));

    writer.write_event(Event::Start(elem))?;

    // Write items in a consistent order
    let quantities = ["zero", "one", "two", "few", "many", "other"];
    for quantity in &quantities {
        if let Some(value) = plural.items.get(*quantity) {
            let mut item_elem = quick_xml::events::BytesStart::new("item");
            item_elem.push_attribute(("quantity", *quantity));
            
            writer.write_event(Event::Start(item_elem))?;
            writer.write_event(Event::Text(quick_xml::events::BytesText::new(value)))?;
            writer.write_event(Event::End(quick_xml::events::BytesEnd::new("item")))?;
        }
    }

    writer.write_event(Event::End(quick_xml::events::BytesEnd::new("plurals")))?;

    Ok(())
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
        assert_eq!(resources.strings[0].variable_mapping.get("%s"), Some(&"name".to_string()));
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
        assert_eq!(resources.plurals[0].items.get("one"), Some(&"%d item".to_string()));
        assert_eq!(resources.plurals[0].items.get("other"), Some(&"%d items".to_string()));
        assert_eq!(resources.plurals[0].variable_mapping.get("%d"), Some(&"count".to_string()));
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
        assert!(xml.contains("<!--%s = {$name}-->"));
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
}
