/// CLDR Plural Rules module
///
/// This module contains the Unicode CLDR plural rules for various locales,
/// providing proper plural form mapping for PO file generation.
use std::collections::HashMap;

/// Represents CLDR plural rule information for a specific locale
#[derive(Debug, Clone)]
pub struct CldrPluralRule {
    /// The locale code (e.g., "en", "ru", "ar")
    pub locale: &'static str,
    /// The PO-style plural forms expression
    pub plural_expression: &'static str,
    /// The CLDR categories in the correct order for this locale
    pub categories: &'static [&'static str],
}

impl CldrPluralRule {
    const fn new(
        locale: &'static str,
        plural_expression: &'static str,
        categories: &'static [&'static str],
    ) -> Self {
        Self {
            locale,
            plural_expression,
            categories,
        }
    }
}

// CLDR Plural Rules for common locales
// These follow the Unicode CLDR plural rules specification
static CLDR_PLURAL_RULES: &[CldrPluralRule] = &[
    // Languages with 1 form (everything is 'other')
    CldrPluralRule::new("zh", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("ja", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("ko", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("vi", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("th", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("id", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("ms", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("tr", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("fa", "nplurals=1; plural=0;", &["other"]),
    CldrPluralRule::new("az", "nplurals=1; plural=0;", &["other"]),
    // Languages with 2 forms (one, other)
    CldrPluralRule::new("en", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("de", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("es", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("it", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("pt", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("nl", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("sv", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("da", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("no", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("fi", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("el", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("he", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("hu", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("ca", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("eu", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("bg", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("et", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    CldrPluralRule::new("lv", "nplurals=2; plural=(n != 1);", &["one", "other"]),
    // French (2 forms but different rule)
    CldrPluralRule::new("fr", "nplurals=2; plural=(n > 1);", &["one", "other"]),
    CldrPluralRule::new("pt-br", "nplurals=2; plural=(n > 1);", &["one", "other"]),
    // Languages with 3 forms (one, few, other)
    CldrPluralRule::new(
        "hr",
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    CldrPluralRule::new(
        "sr",
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    CldrPluralRule::new(
        "bs",
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    // Russian/Ukrainian (3 forms: one, few, other)
    CldrPluralRule::new(
        "ru",
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    CldrPluralRule::new(
        "uk",
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    CldrPluralRule::new(
        "be",
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    // Polish (3 forms with complex rule)
    CldrPluralRule::new(
        "pl",
        "nplurals=3; plural=(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    // Czech/Slovak (3 forms)
    CldrPluralRule::new(
        "cs",
        "nplurals=3; plural=(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2;",
        &["one", "few", "other"],
    ),
    CldrPluralRule::new(
        "sk",
        "nplurals=3; plural=(n==1) ? 0 : (n>=2 && n<=4) ? 1 : 2;",
        &["one", "few", "other"],
    ),
    // Lithuanian (3 forms)
    CldrPluralRule::new(
        "lt",
        "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && (n%100<10 || n%100>=20) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    // Romanian (3 forms)
    CldrPluralRule::new(
        "ro",
        "nplurals=3; plural=(n==1 ? 0 : (n==0 || (n%100 > 0 && n%100 < 20)) ? 1 : 2);",
        &["one", "few", "other"],
    ),
    // Slovenian (4 forms)
    CldrPluralRule::new(
        "sl",
        "nplurals=4; plural=(n%100==1 ? 0 : n%100==2 ? 1 : n%100==3 || n%100==4 ? 2 : 3);",
        &["one", "two", "few", "other"],
    ),
    // Arabic (6 forms: zero, one, two, few, many, other)
    CldrPluralRule::new(
        "ar",
        "nplurals=6; plural=(n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : n%100>=11 ? 4 : 5);",
        &["zero", "one", "two", "few", "many", "other"],
    ),
    // Welsh (4 forms: zero, one, two, other)
    CldrPluralRule::new(
        "cy",
        "nplurals=4; plural=(n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : 3);",
        &["zero", "one", "two", "other"],
    ),
    // Gaelic (5 forms)
    CldrPluralRule::new(
        "gd",
        "nplurals=5; plural=(n==1 || n==11) ? 0 : (n==2 || n==12) ? 1 : (n > 2 && n < 20) ? 2 : (n%10==1 && n%100!=11) ? 3 : 4;",
        &["one", "two", "few", "many", "other"],
    ),
    // Irish (5 forms)
    CldrPluralRule::new(
        "ga",
        "nplurals=5; plural=(n==1 ? 0 : n==2 ? 1 : n<7 ? 2 : n<11 ? 3 : 4);",
        &["one", "two", "few", "many", "other"],
    ),
];

/// Default plural forms for unknown locales (English rules)
pub const DEFAULT_PLURAL_FORMS: &str = "nplurals=2; plural=(n != 1);";

pub const CLDR_SINGULAR_CATEGORY: &str = "one";
pub const CLDR_OTHER_CATEGORY: &str = "other";

pub fn is_singular_category(key: &str) -> bool {
    // Both CLDR "one" category and Fluent numeric "1" are considered singular
    key == CLDR_SINGULAR_CATEGORY || key == "1"
}

pub fn is_other_category(key: &str) -> bool {
    key == CLDR_OTHER_CATEGORY
}

/// Get the CLDR plural rules for a given locale
///
/// Returns the PO-style plural forms expression.
///
/// Note: CLDR categories for reference:
/// - English: ["one", "other"]  
/// - Russian: ["one", "few", "other"]
/// - Arabic: ["zero", "one", "two", "few", "many", "other"]
/// - Chinese: ["other"]
/// Get the plural forms string for a locale (for PO metadata)
pub fn get_plural_forms_for_locale(locale: &str) -> &'static str {
    // Try exact match first
    for rule in CLDR_PLURAL_RULES {
        if rule.locale.eq_ignore_ascii_case(locale) {
            return rule.plural_expression;
        }
    }

    // Try language part only (e.g. "en" from "en-US")
    let lang_part = locale.split('-').next().unwrap_or(locale);
    for rule in CLDR_PLURAL_RULES {
        if rule.locale.eq_ignore_ascii_case(lang_part) {
            return rule.plural_expression;
        }
    }

    // Default to English rules
    DEFAULT_PLURAL_FORMS
}

/// Get the CLDR categories for a specific locale
pub fn get_cldr_categories_for_locale(locale: &str) -> &'static [&'static str] {
    // Try exact match first
    for rule in CLDR_PLURAL_RULES {
        if rule.locale.eq_ignore_ascii_case(locale) {
            return rule.categories;
        }
    }

    // Try language part only (e.g. "en" from "en-US")
    let lang_part = locale.split('-').next().unwrap_or(locale);
    for rule in CLDR_PLURAL_RULES {
        if rule.locale.eq_ignore_ascii_case(lang_part) {
            return rule.categories;
        }
    }

    // Default to English categories
    &["one", "other"]
}

/// Map Fluent CLDR categories to PO msgstr indices (locale-aware)
///
/// This function takes the CLDR categories from Fluent plural forms
/// and maps them to the correct PO msgstr[n] indices based on the
/// locale-specific CLDR ordering.
pub fn map_cldr_categories_to_po_indices_for_locale(
    forms: &[(String, String)],
    locale: &str,
) -> Vec<String> {
    let mut msgstr_forms = Vec::new();

    // Create a map from CLDR category to text for quick lookup
    let mut form_map = HashMap::new();
    for (key, text) in forms {
        form_map.insert(key.as_str(), text.clone());
    }

    // Use locale-specific CLDR categories in the correct order
    let locale_categories = get_cldr_categories_for_locale(locale);
    for &category in locale_categories {
        if let Some(text) = form_map.get(category) {
            msgstr_forms.push(text.clone());
        }
    }

    // Handle any numeric forms that weren't covered
    for (key, text) in forms {
        if key.chars().all(|c| c.is_ascii_digit()) && !msgstr_forms.contains(text) {
            msgstr_forms.push(text.clone());
        }
    }

    // Ensure we have at least 2 forms for PO plural messages
    if msgstr_forms.len() < 2 {
        if let Some(first_form) = msgstr_forms.first() {
            msgstr_forms.push(first_form.clone());
        } else {
            // Fallback to empty forms
            msgstr_forms.push(String::new());
            msgstr_forms.push(String::new());
        }
    }

    msgstr_forms
}

/// Map PO msgstr indices back to Fluent CLDR categories (locale-aware)
///
/// This function reverses the mapping process, converting PO msgstr[n]
/// entries back to the appropriate CLDR category names for the specific locale.
pub fn map_po_indices_to_cldr_categories_for_locale(
    msgstr_count: usize,
    locale: &str,
) -> Vec<&'static str> {
    let locale_categories = get_cldr_categories_for_locale(locale);
    locale_categories
        .iter()
        .take(msgstr_count)
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cldr_plural_rules() {
        // Test exact locale match
        let rule = get_plural_forms_for_locale("ru");
        assert!(rule.contains("nplurals=3"));

        // Test language part extraction
        let rule = get_plural_forms_for_locale("en-US");
        assert!(rule.contains("nplurals=2"));

        // Test default fallback
        let rule = get_plural_forms_for_locale("unknown");
        assert!(rule.contains("nplurals=2"));
    }

    #[test]
    fn test_get_plural_forms_for_locale() {
        // Test that the simplified function returns just the plural forms string
        assert_eq!(
            get_plural_forms_for_locale("en"),
            "nplurals=2; plural=(n != 1);"
        );
        assert_eq!(
            get_plural_forms_for_locale("ru"),
            "nplurals=3; plural=(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2);"
        );
        assert_eq!(get_plural_forms_for_locale("zh"), "nplurals=1; plural=0;");

        // Test unknown locale falls back to English
        assert_eq!(
            get_plural_forms_for_locale("unknown"),
            "nplurals=2; plural=(n != 1);"
        );
    }

    #[test]
    fn test_singular_helper() {
        assert!(is_singular_category("one"));
        assert!(is_singular_category("1"));
        assert!(!is_singular_category("0")); // "0" is grammatically plural
        assert!(!is_singular_category("other"));
        assert!(!is_singular_category("few"));
    }

    #[test]
    fn test_get_cldr_categories_for_locale() {
        // Test English
        assert_eq!(get_cldr_categories_for_locale("en"), &["one", "other"]);

        // Test Russian (3 forms)
        assert_eq!(
            get_cldr_categories_for_locale("ru"),
            &["one", "few", "other"]
        );

        // Test Arabic (6 forms)
        assert_eq!(
            get_cldr_categories_for_locale("ar"),
            &["zero", "one", "two", "few", "many", "other"]
        );

        // Test Chinese (1 form)
        assert_eq!(get_cldr_categories_for_locale("zh"), &["other"]);

        // Test locale with region code
        assert_eq!(get_cldr_categories_for_locale("en-US"), &["one", "other"]);

        // Test unknown locale (falls back to English)
        assert_eq!(get_cldr_categories_for_locale("unknown"), &["one", "other"]);
    }

    #[test]
    fn test_locale_aware_mapping() {
        // Test Russian ordering (one, few, other)
        let forms = vec![
            ("one".to_string(), "один элемент".to_string()),
            ("few".to_string(), "несколько элементов".to_string()),
            ("other".to_string(), "много элементов".to_string()),
        ];

        let msgstr_forms = map_cldr_categories_to_po_indices_for_locale(&forms, "ru");

        // Should be ordered according to Russian CLDR: one, few, other
        assert_eq!(msgstr_forms[0], "один элемент");
        assert_eq!(msgstr_forms[1], "несколько элементов");
        assert_eq!(msgstr_forms[2], "много элементов");

        // Test reverse mapping
        let categories = map_po_indices_to_cldr_categories_for_locale(3, "ru");
        assert_eq!(categories, vec!["one", "few", "other"]);
    }
}
