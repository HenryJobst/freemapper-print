//! Tests of the editor page: its words in both catalogs and its assets against the suite rules.

use std::collections::BTreeSet;

use freemapper_print::web::{check_page_rules, suite_assets};

use crate::text::TEXTS;

const INDEX: &str = include_str!("web/index.html");
const APP_JS: &str = include_str!("web/app.js");
const SHEET_JS: &str = include_str!("web/sheet.js");
const FORMS_JS: &str = include_str!("web/forms.js");
const MASKS_JS: &str = include_str!("web/masks.js");
const NUMBERS_JS: &str = include_str!("web/numbers.js");
const APP_CSS: &str = include_str!("web/app.css");

/// Size limit of the page's own assets plus the suite assets (core ADR 0030).
const BUDGET: usize = 300 * 1024;

fn family(code: &str) -> BTreeSet<String> {
    TEXTS
        .message_ids(code)
        .into_iter()
        .filter(|id| id.starts_with("editor-") || id.starts_with("suite-"))
        .collect()
}

#[test]
fn both_catalogs_have_the_same_page_messages() {
    let (en, de) = (family("en"), family("de"));
    assert!(en.contains("suite-banner-dismiss"));
    assert!(en.iter().filter(|id| id.starts_with("editor-")).count() > 10);
    assert_eq!(en, de);
}

#[test]
fn every_text_the_page_names_exists() {
    let known = family("en");
    let mut named: BTreeSet<String> = BTreeSet::new();
    for attribute in ["data-text", "data-label", "data-title", "data-placeholder"] {
        let needle = format!("{attribute}=\"");
        for (at, _) in INDEX.match_indices(&needle) {
            let rest = &INDEX[at + needle.len()..];
            let id = &rest[..rest.find('"').unwrap()];
            named.insert(id.to_owned());
        }
    }
    for source in [INDEX, APP_JS, SHEET_JS, FORMS_JS, MASKS_JS, NUMBERS_JS] {
        for (at, _) in source.match_indices("\"editor-") {
            let rest = &source[at + 1..];
            named.insert(rest[..rest.find('"').unwrap()].to_owned());
        }
    }
    assert!(named.contains("editor-page-title"));
    // A count text (`tn`) is named by its stem; its forms end in -one and -other.
    let missing: Vec<_> = named
        .difference(&known)
        .filter(|id| {
            !(known.contains(&format!("{id}-one")) && known.contains(&format!("{id}-other")))
        })
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}

#[test]
fn the_page_and_the_suite_assets_keep_the_suite_rules() {
    let suite = suite_assets();
    let mut files: Vec<(&str, &[u8])> = suite
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    files.extend([
        ("index.html", INDEX.as_bytes()),
        ("app.js", APP_JS.as_bytes()),
        ("sheet.js", SHEET_JS.as_bytes()),
        ("forms.js", FORMS_JS.as_bytes()),
        ("masks.js", MASKS_JS.as_bytes()),
        ("numbers.js", NUMBERS_JS.as_bytes()),
        ("app.css", APP_CSS.as_bytes()),
    ]);
    if let Err(problems) = check_page_rules(&files, BUDGET) {
        panic!("{problems:#?}");
    }
}

#[test]
fn the_page_names_the_class_names_and_ids_of_the_suite_frame() {
    for needle in [
        "class=\"frame\"",
        "class=\"menubar\"",
        "class=\"menu\"",
        "class=\"toolbar\"",
        "class=\"status\"",
        "id=\"status-message\"",
        "id=\"banner\"",
        "id=\"notices\" aria-live=",
        "<title data-text=\"editor-page-title\">",
    ] {
        assert!(INDEX.contains(needle), "{needle}");
    }
}

#[test]
fn the_page_knows_a_text_for_every_element_kind() {
    use freemapper_print::project::ElementKind::*;
    for kind in [
        TitleBlock,
        Text,
        Logo,
        ScaleCheckBar,
        CreditLine,
        CopyrightLine,
        ControlDescriptions,
    ] {
        let name = super::preview::kind_name(kind);
        assert!(
            APP_JS.contains(&format!("  {name}: \"editor-kind-")),
            "{name}"
        );
    }
    assert!(APP_JS.contains("footer: \"editor-kind-footer\""));
}
