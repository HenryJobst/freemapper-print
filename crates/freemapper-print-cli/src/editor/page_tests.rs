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
const NOTES_JS: &str = include_str!("web/notes.js");
const EXPORTER_JS: &str = include_str!("web/exporter.js");
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
    for source in [
        INDEX,
        APP_JS,
        SHEET_JS,
        FORMS_JS,
        MASKS_JS,
        NUMBERS_JS,
        EXPORTER_JS,
    ] {
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
        ("exporter.js", EXPORTER_JS.as_bytes()),
        ("notes.js", NOTES_JS.as_bytes()),
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

#[test]
fn the_page_turns_the_renderers_blend_style_into_a_class() {
    assert!(
        SHEET_JS.contains(
            r#"replaceAll(' style="mix-blend-mode:multiply"', ' class="blend-multiply"')"#
        )
    );
    assert!(!SHEET_JS.contains("moveStyles"));
    assert!(APP_CSS.contains(".blend-multiply { mix-blend-mode: multiply; }"));
}

/// The labels `notes.js` cuts from the front of a message.
fn cut_labels() -> Vec<String> {
    let line = NOTES_JS
        .lines()
        .find(|l| l.starts_with("const LABELS = ["))
        .expect("notes.js lists the labels it cuts");
    line["const LABELS = [".len()..line.rfind(']').unwrap()]
        .split(',')
        .map(|l| l.trim().trim_matches('"').to_owned())
        .collect()
}

#[test]
fn every_note_shows_its_severity_as_a_word_and_never_twice() {
    // Every note item is built in one place, with the severity word in its own span.
    assert!(NOTES_JS.contains(r#"word.className = "sev""#));
    assert!(
        NOTES_JS.contains("editor-severity-info") && NOTES_JS.contains("editor-severity-error")
    );
    for (name, source) in [("app.js", APP_JS), ("exporter.js", EXPORTER_JS)] {
        assert!(
            source.contains("noteItem("),
            "{name} builds notes with noteItem"
        );
        assert!(
            !source.contains("note.message"),
            "{name} must not show a bare message"
        );
    }
    // A note's message begins with a label of its own ("Info:", "Note:", "Hinweis:", ...); the page cuts it, so
    // the word shown is the severity and never a second one. Every label the catalogs use must be on the list.
    let labels = cut_labels();
    for code in ["en", "de"] {
        let ftl = std::fs::read_to_string(format!(
            "{}/i18n/{code}/freemapper-print.ftl",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        for line in ftl.lines().filter(|l| l.starts_with("note-")) {
            let text = line.split_once(" = ").map_or("", |(_, text)| text);
            // A label is a short head before the first colon without placeholders or quotes.
            let Some((head, _)) = text.split_once(':') else {
                continue;
            };
            if head.chars().count() <= 20 && !head.contains(['{', '"', ',']) {
                assert!(labels.iter().any(|l| l == head), "{code}: {line}");
            }
        }
    }
}

/// The source of the function `name` in `source`: from its `function` line to the next top-level one.
fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = ["async function ", "function "]
        .iter()
        .find_map(|kind| source.find(&format!("{kind}{name}(")))
        .unwrap_or_else(|| panic!("no function {name}"));
    let rest = &source[start..];
    let end = rest[1..]
        .find("\nasync function ")
        .into_iter()
        .chain(rest[1..].find("\nfunction "))
        .min()
        .map_or(rest.len(), |at| at + 1);
    &rest[..end]
}

#[test]
fn every_call_to_the_server_routes_a_dead_connection_through_the_shared_handler() {
    // The poll, the preview, a write and undo/redo each end in `lost()`, which says so, shows a banner with a
    // reload button and stops polling; the export dialog gets the same handler.
    for name in ["poll", "showSheet", "write", "step"] {
        let body = function_body(APP_JS, name);
        assert!(body.contains("lost()"), "{name} does not call lost()");
        assert!(body.contains("catch"), "{name} does not catch");
    }
    assert!(function_body(APP_JS, "poll").contains("state.misses >= 2"));
    let lost = function_body(APP_JS, "lost");
    assert!(lost.contains("clearInterval(state.timer)"));
    assert!(lost.contains(r#"showBanner(t("editor-status-lost"))"#));
    assert!(lost.contains("location.reload()"));
    assert!(APP_JS.contains("exporter.setup(lost)"));
    for name in ["go", "stop"] {
        assert!(
            function_body(EXPORTER_JS, name).contains("onLost()"),
            "{name} of the export dialog does not use the shared handler"
        );
    }
}

#[test]
fn work_chosen_for_a_window_mask_counts_as_unsent() {
    // The rule that `Fertig` and a change of sheet ask by: a corridor line half drawn, or controls typed or
    // clicked into the window form.
    let dirty = MASKS_JS
        .split("export const dirty")
        .nth(1)
        .expect("masks.js exports dirty");
    let dirty = &dirty[..dirty.find(";\n").unwrap()];
    assert!(dirty.contains(r#"tool.mode === "corridor" && tool.points.length > 0"#));
    assert!(dirty.contains(r#"tool.mode === "window""#));
    assert!(dirty.contains("tool.form.controls.input"));
    // The page asks through it before `Fertig`, a new sheet and undo.
    assert!(function_body(APP_JS, "leaveOk").contains("masks.dirty()"));
    assert!(function_body(APP_JS, "close").contains("leaveOk()"));
}

#[test]
fn a_page_names_itself_in_every_preview_request() {
    assert!(APP_JS.contains("page: state.pageId"));
    assert!(!APP_JS.contains("Date.now()"));
}
