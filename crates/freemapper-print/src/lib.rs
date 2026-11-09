//! freemapper-print turns a finished orienteering map and the courses from a course-setting program into
//! print PDFs, one sheet per course.
//!
//! All domain logic lives in this library. The binary crate `freemapper-print-cli` holds the CLI, the MCP
//! server and the layout editor (ADR 0001). Terms follow `docs/glossary.md`.

pub mod courses;
pub mod describe;
pub mod export;
pub mod mask;
pub mod numbers;
pub mod output;
pub mod overprint;
pub mod print;
pub mod project;
pub mod render;
pub mod rules;
pub mod scale;
pub mod sheet;

// The map model, the course model and the number placement come from freemapper-core (ADR 0043), under the
// module names they had here.
pub use freemapper_course as course;
pub use freemapper_course::placement;
pub use freemapper_omap as omap;

/// The program and where to find it, as named in the PDF metadata.
pub const CREDIT: &str = concat!("freemapper-print – ", env!("CARGO_PKG_REPOSITORY"));

/// The credit line every German sheet carries at the bottom (it cannot be switched off). The dash
/// is U+2013.
pub const CREDIT_LINE_DE: &str = concat!(
    "Erstellt mit freemapper-print – ",
    env!("CARGO_PKG_REPOSITORY")
);

/// The credit line every English sheet carries at the bottom.
pub const CREDIT_LINE_EN: &str = concat!(
    "Created with freemapper-print – ",
    env!("CARGO_PKG_REPOSITORY")
);

/// The credit line for a sheet in `lang`.
pub const fn credit_line(lang: project::SheetLang) -> &'static str {
    match lang {
        project::SheetLang::De => CREDIT_LINE_DE,
        project::SheetLang::En => CREDIT_LINE_EN,
    }
}

#[cfg(test)]
mod boundaries {
    //! The boundaries of ADR 0039 and 0043. `omap`, `course` and `placement` are freemapper-core crates now, so
    //! the compiler keeps them apart; the code here reaches them through the re-exports in this file only.
    //! `render` moves into freemapper-core later and must not reach into the parts of freemapper-print that stay
    //! here, nor into the course crate.

    use std::path::{Path, PathBuf};

    /// Every `.rs` file under `src/<dir>`, tests included.
    fn sources(dir: &str) -> Vec<PathBuf> {
        files_under(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(dir))
    }

    /// Every `.rs` file under `root`.
    fn files_under(root: &Path) -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        walk(root, &mut out);
        assert!(!out.is_empty(), "no sources under {}", root.display());
        out
    }

    /// The first path segments that `text` reaches through `crate::` or through `super::super::` (or more
    /// `super`s), whether written `crate::a::b` or in a group such as `crate::{a::b, c::{d}}`.
    ///
    /// The rule is textual, not a Rust parse: after each `crate::` and each run of two or more `super::`, take
    /// the identifier that follows, or, after `{`, the first identifier of each item of the group (a nested
    /// bare `{` group counts as items of the same level). A single `super::` stays inside the own tree and is
    /// not looked at. A module of the own tree that shares a name with a forbidden one would be a false hit.
    fn reached(text: &str) -> Vec<String> {
        /// The identifier at the start of `s`, and the rest after it.
        fn ident(s: &str) -> (&str, &str) {
            let s = s.trim_start();
            let end = s
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(s.len());
            s.split_at(end)
        }
        /// The items of a group whose `{` is already consumed; returns what follows its `}`.
        fn group<'a>(mut s: &'a str, out: &mut Vec<String>) -> &'a str {
            loop {
                s = s.trim_start();
                if let Some(inner) = s.strip_prefix('{') {
                    s = group(inner, out);
                } else {
                    let (name, rest) = ident(s);
                    if !name.is_empty() {
                        out.push(name.to_string());
                    }
                    s = rest;
                }
                // Skip the rest of the item (its own sub-paths and groups) up to the next top-level comma.
                let mut depth = 0usize;
                let mut end = s.len();
                for (i, c) in s.char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' if depth == 0 => {
                            end = i;
                            break;
                        }
                        '}' => depth -= 1,
                        ',' if depth == 0 => {
                            end = i;
                            break;
                        }
                        _ => {}
                    }
                }
                s = &s[end..];
                match s.chars().next() {
                    Some(',') => s = &s[1..],
                    Some('}') => return &s[1..],
                    _ => return s,
                }
            }
        }
        let mut out = Vec::new();
        for (i, _) in text.match_indices("crate::") {
            let rest = &text[i + "crate::".len()..];
            if let Some(inner) = rest.trim_start().strip_prefix('{') {
                group(inner, &mut out);
            } else {
                out.push(ident(rest).0.to_string());
            }
        }
        for (i, _) in text.match_indices("super::super::") {
            // Only the start of a run: skip the `super::` that has another `super::` in front of it.
            if text[..i].ends_with("super::") {
                continue;
            }
            let mut rest = &text[i..];
            while let Some(r) = rest.strip_prefix("super::") {
                rest = r;
            }
            if let Some(inner) = rest.trim_start().strip_prefix('{') {
                group(inner, &mut out);
            } else {
                out.push(ident(rest).0.to_string());
            }
        }
        out.retain(|n| !n.is_empty());
        out
    }

    /// The external core paths that `render` must not use: in core it will depend on neither the rule profiles,
    /// nor the file writing, nor the courses.
    const FORBIDDEN_EXTERNAL: [&str; 3] = [
        "freemapper_rules::",
        "freemapper_fs::",
        "freemapper_course::",
    ];

    /// The paths of `forbidden` modules that `text` reaches, see [`reached`], and the external paths of
    /// [`FORBIDDEN_EXTERNAL`] that it names.
    fn forbidden_in(text: &str, forbidden: &[&str]) -> Vec<String> {
        let mut seen = reached(text);
        seen.retain(|n| forbidden.contains(&n.as_str()));
        for external in FORBIDDEN_EXTERNAL {
            if text.contains(external) {
                seen.push(external.trim_end_matches("::").to_string());
            }
        }
        seen.sort();
        seen.dedup();
        seen
    }

    /// The forbidden paths that the files under `src/<dir>` use, as "file: module".
    fn uses(dir: &str, forbidden: &[&str]) -> Vec<String> {
        let mut found = Vec::new();
        for file in sources(dir) {
            let text = std::fs::read_to_string(&file).unwrap();
            for module in forbidden_in(&text, forbidden) {
                found.push(format!("{}: {module}", file.display()));
            }
        }
        found
    }

    #[test]
    fn the_checker_rejects_grouped_and_super_super_paths() {
        let f = ["project", "overprint", "numbers", "courses"];
        for bad in [
            "use crate::project::Project;",
            "use crate::{project::Project};",
            "use crate::{omap::Omap, overprint::on_map};",
            "use crate::{omap::{A, B}, project::{self, Project}};",
            "use crate::{\n    omap::Omap,\n    numbers::x,\n};",
            "use crate::{{overprint::on_map}};",
            "use super::super::numbers::x;",
            "use super::super::super::project::P;",
            "let x = crate::numbers::f();",
            "use freemapper_rules::Rules;",
            "let r = freemapper_fs::write_inside(a, b, c);",
            "use freemapper_course::Course;",
        ] {
            assert!(!forbidden_in(bad, &f).is_empty(), "accepted: {bad}");
        }
    }

    #[test]
    fn the_checker_accepts_legitimate_paths() {
        let f = ["project", "overprint", "numbers", "courses"];
        for good in [
            "use crate::omap::Omap;",
            "use crate::{omap::Omap, rules::Rules};",
            "use crate::{omap::{A, B}, rules};",
            "use crate::courses_helper::x;",
            "use super::super::tm::GRS80;",
            "use super::super::draw::{Primitive, Other};",
            "use super::project::X; use super::numbers;",
            "use std::{project::X};",
            "use freemapper_text::Texts;",
            "use freemapper_georef::Crs;",
        ] {
            let hits = forbidden_in(good, &f);
            assert!(hits.is_empty(), "rejected: {good}: {hits:?}");
        }
    }

    #[test]
    fn render_does_not_know_courses_or_sheets() {
        let found = uses(
            "render",
            &[
                "rules",
                "output",
                "scale",
                "overprint",
                "sheet",
                "project",
                "course",
                "courses",
                "placement",
                "numbers",
                "export",
                "describe",
                "print",
            ],
        );
        assert!(found.is_empty(), "{found:#?}");
    }

    /// The files under `src/` and `tests/` other than this one that name `freemapper_omap` or
    /// `freemapper_course`, as "file: crate".
    fn direct_crate_uses() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let lib = root.join("src").join("lib.rs");
        let mut found = Vec::new();
        for file in files_under(&root.join("src"))
            .into_iter()
            .chain(files_under(&root.join("tests")))
            .filter(|f| *f != lib)
        {
            let text = std::fs::read_to_string(&file).unwrap();
            for name in ["freemapper_omap", "freemapper_course"] {
                if text.contains(name) {
                    found.push(format!("{}: {name}", file.display()));
                }
            }
        }
        found
    }

    #[test]
    fn the_core_crates_come_in_through_the_re_exports() {
        // One path per item: `crate::omap`, `crate::course` and `crate::placement` (`freemapper_print::…` in the
        // integration tests), so the code reads as before the move and a crate rename touches this file only.
        let found = direct_crate_uses();
        assert!(found.is_empty(), "{found:#?}");
        let lib = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .unwrap();
        for line in [
            "pub use freemapper_course as course;",
            "pub use freemapper_course::placement;",
            "pub use freemapper_omap as omap;",
        ] {
            assert!(lib.contains(line), "missing: {line}");
        }
    }

    #[test]
    fn mask_knows_no_other_module() {
        // `mask` is to move into a core crate (ADR 0045): it reaches nothing of this crate and no core crate.
        let text =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mask.rs"))
                .unwrap();
        assert!(reached(&text).is_empty(), "{:?}", reached(&text));
        assert!(forbidden_in(&text, &[]).is_empty());
        assert!(!text.contains("freemapper_"));
    }

    #[test]
    fn the_check_sees_a_forbidden_path() {
        // The test above would pass on an empty list: one known use proves the walk reads the files.
        assert!(!uses("sheet", &["project"]).is_empty());
        assert!(uses("sheet", &["projec"]).is_empty());
    }
}
