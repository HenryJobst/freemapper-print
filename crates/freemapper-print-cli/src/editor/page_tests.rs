//! Tests of the editor page: its words in both catalogs and its assets against the suite rules.

use std::collections::BTreeSet;

use freemapper_print::web::{check_page_rules, suite_assets};

use crate::text::TEXTS;

const INDEX: &str = include_str!("web/index.html");
const APP_JS: &str = include_str!("web/app.js");
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
    for source in [INDEX, APP_JS] {
        for (at, _) in source.match_indices("\"editor-") {
            let rest = &source[at + 1..];
            named.insert(rest[..rest.find('"').unwrap()].to_owned());
        }
    }
    assert!(named.contains("editor-page-title"));
    let missing: Vec<_> = named.difference(&known).collect();
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
