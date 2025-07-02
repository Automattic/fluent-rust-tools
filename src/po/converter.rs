use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::po::po_format::{fluent_to_po_catalog, parse_po_file, po_catalog_to_fluent};
use crate::shared::fluent_data;
use polib::po_file;

pub fn fluent_to_po(
    input_path: &Path,
    output_path: &Path,
    locale: &str,
    original_language_input: Option<&Path>,
) -> Result<()> {
    // Read target Fluent file
    let fluent_content = fs::read_to_string(input_path)?;

    // Parse target Fluent
    let fluent_resource = fluent_data::parse_fluent(&fluent_content)?;

    // Read and parse source language file if provided
    let source_resource = if let Some(source_path) = original_language_input {
        let source_content = fs::read_to_string(source_path)?;
        Some(fluent_data::parse_fluent(&source_content)?)
    } else {
        None
    };

    // Convert to PO
    let po_catalog = fluent_to_po_catalog(fluent_resource, locale, source_resource)?;

    // Write PO file with proper CLDR plural forms
    po_file::write(&po_catalog, output_path)
        .map_err(|e| anyhow::anyhow!("Failed to write PO file: {}", e))?;

    Ok(())
}

pub fn po_to_fluent(input_path: &Path, output_path: &Path) -> Result<()> {
    // Parse PO file
    let po_catalog = parse_po_file(input_path)?;

    // Convert to FluentResource
    let fluent_resource = po_catalog_to_fluent(po_catalog)?;

    // Format as properly spaced Fluent content
    let fluent_content = fluent_resource.to_source();

    // Write Fluent file
    fs::write(output_path, fluent_content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

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

        // Verify we have the expected metadata header structure
        assert!(
            po_content.contains("Project-Id-Version:"),
            "PO file should contain required metadata headers"
        );
        assert!(
            po_content.contains("Content-Type: text/plain; charset=UTF-8"),
            "PO file should contain proper content type header"
        );

        // Verify complete PO entry blocks with proper pairing of msgctxt, msgid, and msgstr

        // Check for "hello" entry block
        let hello_block = r#"msgctxt "hello"
msgid "Hello World"
msgstr "Hello World""#;
        assert!(
            po_content.contains(hello_block),
            "PO content should contain complete 'hello' entry block with properly paired msgctxt, msgid, and msgstr"
        );

        // Check for "greeting" entry block
        let greeting_block = r#"msgctxt "greeting"
msgid "Hello, {$name}!"
msgstr "Hello, {$name}!""#;
        assert!(
            po_content.contains(greeting_block),
            "PO content should contain complete 'greeting' entry block with properly paired msgctxt, msgid, and msgstr"
        );
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

        // Verify header metadata contains plural forms information
        assert!(
            po_content.contains("Plural-Forms:"),
            "Should contain Plural-Forms header for proper plural handling"
        );

        // Verify complete plural entry block with proper structure using standard PO format
        let plural_block = r#"#. FLUENT_SELECTOR:num
msgctxt "count"
msgid "{$num} item"
msgid_plural "{$num} items"
msgstr[0] "{$num} item"
msgstr[1] "{$num} items""#;
        assert!(
            po_content.contains(plural_block),
            "PO content should contain complete plural entry block with properly formatted FLUENT_SELECTOR comment, msgctxt, msgid, msgid_plural, and standard msgstr entries without FLUENT_ markers"
        );
    }

    #[test]
    fn test_po_to_fluent_simple() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.po");
        let output_path = temp_dir.path().join("output.ftl");

        // Create a simple PO file with hardcoded content
        let po_content = r#"msgid ""
msgstr ""
"Project-Id-Version: test 1.0\n"
"POT-Creation-Date: 2025-01-01 12:00+0000\n"
"PO-Revision-Date: 2025-01-01 12:00+0000\n"
"Last-Translator: Test\n"
"Language-Team: English\n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Language: en\n"

msgctxt "hello"
msgid "Hello World"
msgstr "Hello World"

msgctxt "greeting"
msgid "Hello, {$name}!"
msgstr "Hello, {$name}!"
"#;
        fs::write(&input_path, po_content).unwrap();

        // Convert to Fluent
        let result = po_to_fluent(&input_path, &output_path);
        assert!(result.is_ok());

        // Verify Fluent file was created with expected content
        let fluent_content = fs::read_to_string(&output_path).unwrap();
        assert!(fluent_content.contains("hello = Hello World"));
        assert!(fluent_content.contains("greeting = Hello, { $name }!"));
    }

    #[test]
    fn test_round_trip_conversion() {
        let temp_dir = tempdir().unwrap();
        let original_ftl = temp_dir.path().join("original.ftl");
        let po_file = temp_dir.path().join("intermediate.po");
        let converted_ftl = temp_dir.path().join("converted.ftl");

        // Original Fluent content with various complex cases including multiline values
        let original_content = r#"hello = Hello World
greeting = Hello, {$name}!
# This is a comment
farewell = Goodbye, {$name}!
# Multiline message for testing round-trip preservation
description = This is the first line of a longer description.
    This is the second line with proper indentation.
    And this is the third line that should be preserved.
# Another multiline with variables
# and multiline comments
instructions = Step 1: Click the {$button} button
    Step 2: Enter your {$username} in the field
    Step 3: Save your changes
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

        // Check that all messages are preserved, including multiline formatting
        assert!(converted_content.contains("hello = Hello World"));
        assert!(converted_content.contains("greeting = Hello, { $name }!"));
        assert!(converted_content.contains(
            r#"# This is a comment
farewell = Goodbye, { $name }!"#
        ));

        // Verify multiline content is preserved (without manual formatting expectations)
        // Note: The simplified approach relies on Fluent parser behavior
        assert!(converted_content.contains("description ="));
        assert!(converted_content.contains("This is the first line of a longer description."));
        assert!(converted_content.contains("This is the second line with proper indentation."));
        assert!(converted_content.contains("And this is the third line that should be preserved."));

        // Verify multiline content with variables is preserved
        // Note: Variables are normalized with spaces by the built-in serializer
        assert!(converted_content.contains("instructions ="));
        assert!(converted_content.contains("Step 1: Click the { $button } button"));
        assert!(converted_content.contains("Step 2: Enter your { $username } in the field"));
        assert!(converted_content.contains("Step 3: Save your changes"));
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
        assert!(converted_content.contains("item_count ="));
        assert!(converted_content.contains("{ $count ->"));
        assert!(converted_content.contains("[one] { $count } item"));
        assert!(converted_content.contains("*[other] { $count } items"));
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

        // Verify PO file contains comments with proper format
        let po_content = fs::read_to_string(&output_path).unwrap();

        // Verify complete comment blocks with proper PO extracted comment format (#.)
        let hello_comment_block = r#"#. This is a greeting message
msgctxt "hello"
msgid "Hello World"
msgstr "Hello World""#;
        assert!(
            po_content.contains(hello_comment_block),
            "Should contain complete 'hello' entry with properly formatted extracted comment using '#.' prefix"
        );

        let greeting_comment_block = r#"#. Welcome message for users
msgctxt "greeting"
msgid "Hello, {$name}!"
msgstr "Hello, {$name}!""#;
        assert!(
            po_content.contains(greeting_comment_block),
            "Should contain complete 'greeting' entry with properly formatted extracted comment using '#.' prefix"
        );
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
    fn test_fluent_to_po_unclosed_select_expression() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("unclosed_select.ftl");
        let output_path = temp_dir.path().join("output.po");

        // Test specific error: unclosed select expression
        let content = r#"hello = Hello World
bad_plural = {$count ->
    [one] item
    # Missing closing brace
"#;
        fs::write(&input_path, content).unwrap();

        let result = fluent_to_po(&input_path, &output_path, "en", None);
        assert!(result.is_err(), "Should fail on unclosed select expression");
    }

    #[test]
    fn test_fluent_to_po_mismatched_braces() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("mismatched_braces.ftl");
        let output_path = temp_dir.path().join("output.po");

        // Test specific error: mismatched braces (opening { but closing with ])
        let content = r#"hello = Hello World
greeting = Hello, {$name]!
"#;
        fs::write(&input_path, content).unwrap();

        let result = fluent_to_po(&input_path, &output_path, "en", None);
        assert!(result.is_err(), "Should fail on mismatched braces");
    }

    #[test]
    fn test_fluent_to_po_invalid_key_syntax() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("invalid_key.ftl");
        let output_path = temp_dir.path().join("output.po");

        // Test specific error: invalid attribute syntax (missing dot before attribute)
        let content = r#"button = Click me
Button for clicking
"#;
        fs::write(&input_path, content).unwrap();

        let result = fluent_to_po(&input_path, &output_path, "en", None);
        assert!(result.is_err(), "Should fail on invalid attribute syntax");
    }

    #[test]
    fn test_po_to_fluent_completely_invalid_structure() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("invalid_structure.po");
        let output_path = temp_dir.path().join("output.ftl");

        // Test specific error: binary/non-text file content that should fail parsing
        let content = b"\x00\x01\x02\xFF\xFE\x80\x90Binary data that is not valid text\x00\x00";
        fs::write(&input_path, content).unwrap();

        let result = po_to_fluent(&input_path, &output_path);
        assert!(
            result.is_err(),
            "Should fail on binary/invalid file content"
        );
    }

    #[test]
    fn test_po_to_fluent_invalid_escape_sequence() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("invalid_escape.po");
        let output_path = temp_dir.path().join("output.ftl");

        // Test specific error: invalid escape sequence
        let content = r#"msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"

msgctxt "greeting"  
msgid "Hello, \z invalid escape!"
msgstr "Hello, world!"
"#;
        fs::write(&input_path, content).unwrap();

        let result = po_to_fluent(&input_path, &output_path);
        assert!(result.is_err(), "Should fail on invalid escape sequence");
    }

    #[test]
    fn test_po_to_fluent_malformed_msgid_syntax() {
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("malformed_msgid.po");
        let output_path = temp_dir.path().join("output.ftl");

        // Test specific malformed PO content: broken msgid structure (missing quotes)
        // This causes polib to fail during metadata parsing
        let content = "msgid\nmsgstr \"test\"";
        fs::write(&input_path, content).unwrap();

        let result = po_to_fluent(&input_path, &output_path);
        assert!(result.is_err(), "Should fail on broken msgid structure");
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
"Project-Id-Version: Mobile - wordpress-rs\n"

msgctxt "parse_api_root_failure_reason_wordfence_blocking_access"
msgid "Wordfence is blocking access to the site's API. Please check your Wordfence configuration."
msgstr "Wordfence está bloqueando o acesso à API do site. Por favor, verifique sua configuração do Wordfence."

msgctxt "application_passwords_not_supported"
msgid "The site does not support Application Passwords."
msgstr "O site não suporta Senhas de Aplicativo."
"#;

        fs::write(&input_path, problematic_po_content).unwrap();

        // This should succeed with our preprocessing fallback mechanism
        let result = po_to_fluent(&input_path, &output_path);

        match result {
            Ok(_) => {
                // Verify the output contains expected content
                let fluent_content = fs::read_to_string(&output_path).unwrap();
                assert!(
                    fluent_content
                        .contains("parse_api_root_failure_reason_wordfence_blocking_access")
                );
                assert!(fluent_content.contains("application_passwords_not_supported"));
            }
            Err(e) => {
                panic!(
                    "Expected successful conversion with preprocessing fallback, but got error: {e}"
                );
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
        let direct_parse_result = std::panic::catch_unwind(|| polib::po_file::parse(&po_path));

        // This should succeed without panic or preprocessing
        match direct_parse_result {
            Ok(Ok(catalog)) => {
                // Success! Our generated PO file has all required metadata
                println!("Generated PO file parsed successfully without preprocessing");
                assert!(catalog.messages().count() > 0);
            }
            Ok(Err(e)) => {
                panic!("Generated PO file failed to parse: {e}");
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
        assert!(roundtrip_content.contains("greeting = Hello, { $name }!"));
    }

    #[test]
    fn test_russian_cldr_plural_ordering() {
        // Test that Russian plurals are correctly ordered according to CLDR rules
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let po_path = temp_dir.path().join("output.po");
        let roundtrip_path = temp_dir.path().join("roundtrip.ftl");

        // Create a Fluent file with Russian plural forms (one, few, other)
        let fluent_content = r#"files = { $count ->
    [one] { $count } файл
    [few] { $count } файла  
   *[other] { $count } файлов
}
"#;
        fs::write(&input_path, fluent_content).unwrap();

        // Convert to PO with Russian locale
        let result = fluent_to_po(&input_path, &po_path, "ru", None);
        assert!(result.is_ok());

        // Verify Russian plural forms in metadata
        let po_content = fs::read_to_string(&po_path).unwrap();
        assert!(
            po_content.contains("nplurals=3"),
            "Should have 3 plural forms for Russian"
        );
        assert!(
            po_content.contains("Language: ru"),
            "Should specify Russian language"
        );

        // Verify msgstr forms are ordered according to Russian CLDR: [one, few, other]
        assert!(
            po_content.contains("msgstr[0] \"{$count} файл\""),
            "msgstr[0] should be 'one' form"
        );
        assert!(
            po_content.contains("msgstr[1] \"{$count} файла\""),
            "msgstr[1] should be 'few' form"
        );
        assert!(
            po_content.contains("msgstr[2] \"{$count} файлов\""),
            "msgstr[2] should be 'other' form"
        );

        // Test round-trip conversion preserves ordering
        let roundtrip_result = po_to_fluent(&po_path, &roundtrip_path);
        assert!(roundtrip_result.is_ok());

        let roundtrip_content = fs::read_to_string(&roundtrip_path).unwrap();
        assert!(roundtrip_content.contains("[one] { $count } файл"));
        assert!(roundtrip_content.contains("[few] { $count } файла"));
        assert!(roundtrip_content.contains("*[other] { $count } файлов"));
    }

    #[test]
    fn test_arabic_cldr_plural_ordering() {
        // Test that Arabic plurals are correctly ordered according to CLDR rules (6 forms)
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let po_path = temp_dir.path().join("output.po");
        let roundtrip_path = temp_dir.path().join("roundtrip.ftl");

        // Create a Fluent file with Arabic plural forms (zero, one, two, few, many, other)
        let fluent_content = r#"items = { $count ->
    [zero] لا توجد عناصر
    [one] عنصر واحد
    [two] عنصران
    [few] { $count } عناصر
    [many] { $count } عنصراً
   *[other] { $count } عنصر
}
"#;
        fs::write(&input_path, fluent_content).unwrap();

        // Convert to PO with Arabic locale
        let result = fluent_to_po(&input_path, &po_path, "ar", None);
        assert!(result.is_ok());

        // Verify Arabic plural forms in metadata
        let po_content = fs::read_to_string(&po_path).unwrap();
        assert!(
            po_content.contains("nplurals=6"),
            "Should have 6 plural forms for Arabic"
        );
        assert!(
            po_content.contains("Language: ar"),
            "Should specify Arabic language"
        );

        // Verify msgstr forms are ordered according to Arabic CLDR: [zero, one, two, few, many, other]
        assert!(
            po_content.contains("msgstr[0] \"لا توجد عناصر\""),
            "msgstr[0] should be 'zero' form"
        );
        assert!(
            po_content.contains("msgstr[1] \"عنصر واحد\""),
            "msgstr[1] should be 'one' form"
        );
        assert!(
            po_content.contains("msgstr[2] \"عنصران\""),
            "msgstr[2] should be 'two' form"
        );
        assert!(
            po_content.contains("msgstr[3] \"{$count} عناصر\""),
            "msgstr[3] should be 'few' form"
        );
        assert!(
            po_content.contains("msgstr[4] \"{$count} عنصراً\""),
            "msgstr[4] should be 'many' form"
        );
        assert!(
            po_content.contains("msgstr[5] \"{$count} عنصر\""),
            "msgstr[5] should be 'other' form"
        );

        // Test round-trip conversion preserves all forms
        let roundtrip_result = po_to_fluent(&po_path, &roundtrip_path);
        assert!(roundtrip_result.is_ok());

        let roundtrip_content = fs::read_to_string(&roundtrip_path).unwrap();
        assert!(roundtrip_content.contains("[zero] لا توجد عناصر"));
        assert!(roundtrip_content.contains("[one] عنصر واحد"));
        assert!(roundtrip_content.contains("[two] عنصران"));
        assert!(roundtrip_content.contains("[few] { $count } عناصر"));
        assert!(roundtrip_content.contains("[many] { $count } عنصراً"));
        assert!(roundtrip_content.contains("*[other] { $count } عنصر"));
    }

    #[test]
    fn test_chinese_cldr_single_form() {
        // Test that Chinese (single plural form) is handled correctly
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let po_path = temp_dir.path().join("output.po");
        let roundtrip_path = temp_dir.path().join("roundtrip.ftl");

        // Create a Fluent file with Chinese plural (only 'other' form needed)
        let fluent_content = r#"items = { $count ->
   *[other] { $count } 个项目
}
"#;
        fs::write(&input_path, fluent_content).unwrap();

        // Convert to PO with Chinese locale
        let result = fluent_to_po(&input_path, &po_path, "zh", None);
        assert!(result.is_ok());

        // Verify Chinese plural forms in metadata
        let po_content = fs::read_to_string(&po_path).unwrap();
        assert!(
            po_content.contains("nplurals=1"),
            "Should have 1 plural form for Chinese"
        );
        assert!(
            po_content.contains("Language: zh"),
            "Should specify Chinese language"
        );

        // Chinese should still generate msgstr[0] and msgstr[1] for PO compatibility
        assert!(po_content.contains("msgstr[0] \"{$count} 个项目\""));
        assert!(po_content.contains("msgstr[1] \"{$count} 个项目\""));

        // Test round-trip conversion
        let roundtrip_result = po_to_fluent(&po_path, &roundtrip_path);
        assert!(roundtrip_result.is_ok());

        let roundtrip_content = fs::read_to_string(&roundtrip_path).unwrap();
        assert!(roundtrip_content.contains("*[other] { $count } 个项目"));
    }

    #[test]
    fn test_mixed_numeric_and_cldr_categories() {
        // Test that mixed numeric and CLDR categories are handled correctly
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let po_path = temp_dir.path().join("output.po");
        let roundtrip_path = temp_dir.path().join("roundtrip.ftl");

        // Create a Fluent file mixing numeric and CLDR categories
        let fluent_content = r#"notification = { $count ->
    [0] No notifications
    [1] One notification  
    [one] { $count } notification
   *[other] { $count } notifications
}
"#;
        fs::write(&input_path, fluent_content).unwrap();

        // Convert to PO with English locale
        let result = fluent_to_po(&input_path, &po_path, "en", None);
        assert!(result.is_ok());

        let po_content = fs::read_to_string(&po_path).unwrap();

        // Should prioritize CLDR categories first, then numeric forms
        // For English: [one, other] categories come first, then [0, 1] numeric forms
        assert!(
            po_content.contains("msgstr[0] \"{$count} notification\""),
            "msgstr[0] should be CLDR 'one' form"
        );
        assert!(
            po_content.contains("msgstr[1] \"{$count} notifications\""),
            "msgstr[1] should be CLDR 'other' form"
        );
        assert!(
            po_content.contains("No notifications"),
            "Should include numeric '0' form"
        );
        assert!(
            po_content.contains("One notification"),
            "Should include numeric '1' form"
        );

        // Test round-trip conversion preserves all forms
        let roundtrip_result = po_to_fluent(&po_path, &roundtrip_path);
        assert!(roundtrip_result.is_ok());

        let roundtrip_content = fs::read_to_string(&roundtrip_path).unwrap();
        assert!(roundtrip_content.contains("notification ="));
        assert!(roundtrip_content.contains("{ $count ->"));
    }

    #[test]
    fn test_locale_with_region_code() {
        // Test that locale with region codes (like en-US, pt-BR) work correctly
        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.ftl");
        let po_path = temp_dir.path().join("output.po");

        // Create a Fluent file with plurals
        let fluent_content = r#"days = { $count ->
    [one] { $count } dia
   *[other] { $count } dias  
}
"#;
        fs::write(&input_path, fluent_content).unwrap();

        // Convert to PO with Brazilian Portuguese locale (should use Portuguese rules)
        let result = fluent_to_po(&input_path, &po_path, "pt-BR", None);
        assert!(result.is_ok());

        let po_content = fs::read_to_string(&po_path).unwrap();

        // Should use Portuguese plural rules (nplurals=2; plural=(n > 1))
        assert!(
            po_content.contains("nplurals=2"),
            "Should have 2 plural forms for Portuguese"
        );
        assert!(
            po_content.contains("plural=(n > 1)"),
            "Should use Portuguese plural rule"
        );
        assert!(
            po_content.contains("Language: pt-BR"),
            "Should preserve full locale code"
        );

        // Verify correct ordering (Portuguese uses same categories as English: one, other)
        assert!(
            po_content.contains("msgstr[0] \"{$count} dia\""),
            "msgstr[0] should be 'one' form"
        );
        assert!(
            po_content.contains("msgstr[1] \"{$count} dias\""),
            "msgstr[1] should be 'other' form"
        );
    }
}
