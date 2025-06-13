use anyhow::Result;
use std::path::Path;
use std::fs;

use crate::shared::fluent_parser;
use crate::po::po_format::{write_po_file, fluent_to_po_catalog, parse_po_file, po_catalog_to_fluent};

pub fn fluent_to_po(input_path: &Path, output_path: &Path, locale: &str, original_language_input: Option<&Path>) -> Result<()> {
    // Read target Fluent file
    let fluent_content = fs::read_to_string(input_path)?;
    
    // Parse target Fluent
    let fluent_resource = fluent_parser::parse_fluent(&fluent_content)?;
    
    // Read and parse source language file if provided
    let source_resource = if let Some(source_path) = original_language_input {
        let source_content = fs::read_to_string(source_path)?;
        Some(fluent_parser::parse_fluent(&source_content)?)
    } else {
        None
    };
    
    // Convert to PO
    let po_catalog = fluent_to_po_catalog(fluent_resource, locale, source_resource)?;
    
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
        let result = fluent_to_po(&input_path, &output_path, "en", None);
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
        let result = fluent_to_po(&input_path, &output_path, "en", None);
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
        let result1 = fluent_to_po(&original_ftl, &po_file, "en", None);
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
        let result1 = fluent_to_po(&original_ftl, &po_file, "en", None);
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
        let result = fluent_to_po(&input_path, &output_path, "en", None);
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
        let result = fluent_to_po(&input_path, &output_path, "en", None);
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

    #[test]
    fn test_po_with_missing_metadata_headers() {
        // Test conversion of PO files with missing required metadata headers
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("problematic.po");
        let output_path = temp_dir.path().join("output.ftl");

        // Create a PO file missing required metadata fields (POT-Creation-Date, Last-Translator, Language-Team)
        let problematic_po_content = r#"# Translation of Mobile - wordpress-rs in Portuguese (Brazil)
# This file is distributed under the same license as the Mobile - wordpress-rs package.
msgid ""
msgstr ""
"PO-Revision-Date: 2025-06-05 15:08:46+0000\n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Plural-Forms: nplurals=2; plural=(n > 1);\n"
"X-Generator: GlotPress/2.4.0-alpha\n"
"Language: pt_BR\n"
"Project-Id-Version: Mobile - wordpress-rs\n"

msgctxt "parse_api_root_failure_reason_wordfence_blocking_access"
msgid "Wordfence is blocking access to the site's API. Please check your Wordfence configuration."
msgstr ""

msgctxt "application_passwords_not_supported"
msgid "The site does not support Application Passwords."
msgstr ""
"#;
        
        fs::write(&input_path, problematic_po_content).unwrap();
        
        // This should succeed with our preprocessing fallback mechanism
        let result = po_to_fluent(&input_path, &output_path);
        
        match result {
            Ok(_) => {
                // Verify the output contains expected content
                let fluent_content = fs::read_to_string(&output_path).unwrap();
                assert!(fluent_content.contains("parse_api_root_failure_reason_wordfence_blocking_access"));
                assert!(fluent_content.contains("application_passwords_not_supported"));
            }
            Err(e) => {
                panic!("Expected successful conversion with preprocessing fallback, but got error: {}", e);
            }
        }
    }

    #[test]
    fn test_fluent_to_po_with_source_language() {
        let temp_dir = tempdir().unwrap();
        let source_path = temp_dir.path().join("source.ftl");
        let target_path = temp_dir.path().join("target.ftl");
        let output_path = temp_dir.path().join("output.po");
        
        // Create source language Fluent file (English)
        let source_content = r#"hello = Hello World
greeting = Hello, {$name}!
"#;
        fs::write(&source_path, source_content).unwrap();
        
        // Create target language Fluent file (French)
        let target_content = r#"hello = Bonjour le monde
greeting = Bonjour, {$name}!
"#;
        fs::write(&target_path, target_content).unwrap();
        
        // Convert target to PO with source language for msgid
        let result = fluent_to_po(&target_path, &output_path, "fr", Some(&source_path));
        assert!(result.is_ok());
        
        // Verify PO file was created
        assert!(output_path.exists());
        
        // Read the PO content and verify msgid contains source language text
        let po_content = fs::read_to_string(&output_path).unwrap();
        
        // Check that msgid contains English text (source) and msgstr contains French text (target)
        assert!(po_content.contains("msgid \"Hello World\""));
        assert!(po_content.contains("msgstr \"Bonjour le monde\""));
        assert!(po_content.contains("msgid \"Hello, {$name}!\""));
        assert!(po_content.contains("msgstr \"Bonjour, {$name}!\""));
        
        // Verify the msgctxt is present
        assert!(po_content.contains("msgctxt \"hello\""));
        assert!(po_content.contains("msgctxt \"greeting\""));
    }

    #[test]
    fn test_fluent_to_po_includes_required_metadata() {
        // Test that PO files we generate include all required metadata fields
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let po_path = temp_dir.path().join("output.po");
        let roundtrip_path = temp_dir.path().join("roundtrip.ftl");
        
        // Create a simple Fluent file
        let fluent_content = r#"hello = Hello World
greeting = Hello, {$name}!
"#;
        fs::write(&input_path, fluent_content).unwrap();
        
        // Convert Fluent to PO
        let result = fluent_to_po(&input_path, &po_path, "en", None);
        assert!(result.is_ok());
        
        // Verify PO file was created
        assert!(po_path.exists());
        
        // Most importantly, verify we can parse our own generated PO file
        // without any preprocessing (this would fail if required metadata is missing)
        let direct_parse_result = std::panic::catch_unwind(|| {
            polib::po_file::parse(&po_path)
        });
        
        // This should succeed without panic or preprocessing
        match direct_parse_result {
            Ok(Ok(catalog)) => {
                // Success! Our generated PO file has all required metadata
                println!("Generated PO file parsed successfully without preprocessing");
                assert!(catalog.messages().count() > 0);
            }
            Ok(Err(e)) => {
                panic!("Generated PO file failed to parse: {}", e);
            }
            Err(_) => {
                panic!("Generated PO file caused a panic - missing required metadata fields");
            }
        }
        
        // Also test the round-trip conversion works
        let roundtrip_result = po_to_fluent(&po_path, &roundtrip_path);
        assert!(roundtrip_result.is_ok());
        
        let roundtrip_content = fs::read_to_string(&roundtrip_path).unwrap();
        assert!(roundtrip_content.contains("hello = Hello World"));
        assert!(roundtrip_content.contains("greeting = Hello, {$name}!"));
    }
}
