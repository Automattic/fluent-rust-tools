use anyhow::Result;
use std::path::Path;
use std::fs;

use crate::shared::fluent_parser;
use crate::po::po_format::{write_po_file, fluent_to_po_catalog, parse_po_file, po_catalog_to_fluent};

pub fn fluent_to_po(input_path: &Path, output_path: &Path, locale: &str) -> Result<()> {
    // Read Fluent file
    let fluent_content = fs::read_to_string(input_path)?;
    
    // Parse Fluent
    let fluent_resource = fluent_parser::parse_fluent(&fluent_content)?;
    
    // Convert to PO
    let po_catalog = fluent_to_po_catalog(fluent_resource, locale)?;
    
    // Write PO file
    write_po_file(&po_catalog, output_path)?;
    
    Ok(())
}

pub fn po_to_fluent(input_path: &Path, output_path: &Path) -> Result<()> {
    // Parse PO file
    let po_catalog = parse_po_file(input_path)?;
    
    // Convert to Fluent
    let fluent_content = po_catalog_to_fluent(po_catalog)?;
    
    // Write Fluent file
    fs::write(output_path, fluent_content)?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_fluent_to_po_simple() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let output_path = temp_dir.path().join("output.po");
        
        // Create a simple Fluent file
        let fluent_content = r#"hello = Hello World
greeting = Hello, {$name}!
"#;
        fs::write(&input_path, fluent_content).unwrap();
        
        // Convert to PO
        let result = fluent_to_po(&input_path, &output_path, "en");
        assert!(result.is_ok());
        
        // Verify PO file was created
        assert!(output_path.exists());
        let po_content = fs::read_to_string(&output_path).unwrap();
        assert!(po_content.contains("msgid \"Hello World\""));
        assert!(po_content.contains("msgctxt \"hello\""));
        assert!(po_content.contains("msgid \"Hello, {$name}!\""));
        assert!(po_content.contains("msgctxt \"greeting\""));
    }

    #[test]
    fn test_fluent_to_po_with_plurals() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let output_path = temp_dir.path().join("output.po");
        
        // Create a Fluent file with plurals
        let fluent_content = r#"count = {$num ->
    [one] {$num} item
   *[other] {$num} items
}
"#;
        fs::write(&input_path, fluent_content).unwrap();
        
        // Convert to PO
        let result = fluent_to_po(&input_path, &output_path, "en");
        assert!(result.is_ok());
        
        // Verify PO file was created with plural forms
        assert!(output_path.exists());
        let po_content = fs::read_to_string(&output_path).unwrap();
        assert!(po_content.contains("msgid_plural"));
        assert!(po_content.contains("FLUENT_SELECTOR:num"));
    }

    #[test]
    fn test_po_to_fluent_simple() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.po");
        let output_path = temp_dir.path().join("output.ftl");

        use polib::catalog::Catalog;
        use polib::metadata::CatalogMetadata;
        use polib::message::Message as PoMessage;
        use crate::po::po_format::write_po_file;

        let mut metadata = CatalogMetadata::default();
        metadata.language = "en".to_string();
        metadata.content_type = "text/plain; charset=UTF-8".to_string();
        
        let mut catalog = Catalog::new(metadata);
        
        // Add hello message
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("hello".to_string())
            .with_msgid("Hello World".to_string())
            .with_msgstr("Hello World".to_string());
        let message = msg_builder.done();
        catalog.append_or_update(message);
        
        // Add greeting message
        let mut msg_builder = PoMessage::build_singular();
        msg_builder
            .with_msgctxt("greeting".to_string())
            .with_msgid("Hello, {$name}!".to_string())
            .with_msgstr("Hello, {$name}!".to_string());
        let message = msg_builder.done();
        catalog.append_or_update(message);
        
        // Write the catalog to file
        write_po_file(&catalog, &input_path).unwrap();
        
        // Convert to Fluent
        let result = po_to_fluent(&input_path, &output_path);
        assert!(result.is_ok());
        
        // Verify Fluent file was created with expected content
        let fluent_content = fs::read_to_string(&output_path).unwrap();
        assert!(fluent_content.contains("hello = Hello World"));
        assert!(fluent_content.contains("greeting = Hello, {$name}!"));
    }

    #[test]
    fn test_round_trip_conversion() {
        let temp_dir = tempdir().unwrap();
        let original_ftl = temp_dir.path().join("original.ftl");
        let po_file = temp_dir.path().join("intermediate.po");
        let converted_ftl = temp_dir.path().join("converted.ftl");
        
        // Original Fluent content
        let original_content = r#"hello = Hello World
greeting = Hello, {$name}!
# This is a comment
farewell = Goodbye, {$name}!
"#;
        fs::write(&original_ftl, original_content).unwrap();
        
        // Convert Fluent to PO
        let result1 = fluent_to_po(&original_ftl, &po_file, "en");
        assert!(result1.is_ok());
        
        // Convert PO back to Fluent
        let result2 = po_to_fluent(&po_file, &converted_ftl);
        assert!(result2.is_ok());
        
        // Read the converted content
        let converted_content = fs::read_to_string(&converted_ftl).unwrap();
        
        // Check that key messages are preserved
        assert!(converted_content.contains("hello = Hello World"));
        assert!(converted_content.contains("greeting = Hello, {$name}!"));
        assert!(converted_content.contains("farewell = Goodbye, {$name}!"));
    }

    #[test]
    fn test_round_trip_conversion_with_plurals() {
        let temp_dir = tempdir().unwrap();
        let original_ftl = temp_dir.path().join("original.ftl");
        let po_file = temp_dir.path().join("intermediate.po");
        let converted_ftl = temp_dir.path().join("converted.ftl");
        
        // Original Fluent content with plurals
        let original_content = r#"item_count = {$count ->
    [one] {$count} item
   *[other] {$count} items
}"#;
        fs::write(&original_ftl, original_content).unwrap();
        
        // Convert Fluent to PO
        let result1 = fluent_to_po(&original_ftl, &po_file, "en");
        assert!(result1.is_ok());
        
        // Convert PO back to Fluent
        let result2 = po_to_fluent(&po_file, &converted_ftl);
        assert!(result2.is_ok());
        
        // Read the converted content
        let converted_content = fs::read_to_string(&converted_ftl).unwrap();
        
        // Check that the core message structure is preserved
        // Note: PO conversion adds FLUENT_SELECTOR comments for round-trip preservation
        assert!(converted_content.contains("item_count = {$count ->"));
        assert!(converted_content.contains("[one] {$count} item"));
        assert!(converted_content.contains("*[other] {$count} items"));
    }

    #[test]
    fn test_fluent_to_po_with_comments() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let output_path = temp_dir.path().join("output.po");
        
        // Create a Fluent file with comments
        let fluent_content = r#"# This is a greeting message
hello = Hello World
# Welcome message for users
greeting = Hello, {$name}!
"#;
        fs::write(&input_path, fluent_content).unwrap();
        
        // Convert to PO
        let result = fluent_to_po(&input_path, &output_path, "en");
        assert!(result.is_ok());
        
        // Verify PO file contains comments
        let po_content = fs::read_to_string(&output_path).unwrap();
        assert!(po_content.contains("This is a greeting message"));
        assert!(po_content.contains("Welcome message for users"));
    }

    #[test]
    fn test_fluent_to_po_invalid_input() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("nonexistent.ftl");
        let output_path = temp_dir.path().join("output.po");
        
        // Try to convert non-existent file
        let result = fluent_to_po(&input_path, &output_path, "en");
        assert!(result.is_err());
    }

    #[test]
    fn test_po_to_fluent_invalid_input() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("nonexistent.po");
        let output_path = temp_dir.path().join("output.ftl");
        
        // Try to convert non-existent file
        let result = po_to_fluent(&input_path, &output_path);
        assert!(result.is_err());
    }
}
