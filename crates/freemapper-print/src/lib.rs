//! freemapper-print turns a finished orienteering map and the courses from a course-setting program into
//! print PDFs, one sheet per course.
//!
//! All domain logic lives in this library. The binary crate `freemapper-print-cli` holds the CLI, the MCP
//! server and the layout editor (ADR 0001). Terms follow `docs/glossary.md`.

pub mod course;
pub mod courses;
pub mod describe;
pub mod export;
pub mod numbers;
pub mod omap;
pub mod output;
pub mod overprint;
pub mod placement;
pub mod print;
pub mod project;
pub mod render;
pub mod rules;
pub mod scale;
pub mod sheet;

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
    //! The module boundaries of ADR 0039: `course`, `render` and `omap` move into freemapper-core later, so they
    //! must not reach into the parts of freemapper-print that stay here.

    use std::path::{Path, PathBuf};

    /// Every `.rs` file under `src/<dir>`, tests included.
    fn sources(dir: &str) -> Vec<PathBuf> {
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
        walk(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(dir),
            &mut out,
        );
        assert!(!out.is_empty(), "no sources under src/{dir}");
        out
    }

    /// The `crate::` paths of `forbidden` that the files under `src/<dir>` use, as "file: path".
    fn uses(dir: &str, forbidden: &[&str]) -> Vec<String> {
        let mut found = Vec::new();
        for file in sources(dir) {
            let text = std::fs::read_to_string(&file).unwrap();
            for module in forbidden {
                let path = format!("crate::{module}");
                // `crate::course` must not match `crate::courses`.
                let hit = text.match_indices(&path).any(|(i, _)| {
                    !text[i + path.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
                });
                if hit {
                    found.push(format!("{}: {path}", file.display()));
                }
            }
        }
        found
    }

    #[test]
    fn course_uses_only_omap_and_rules() {
        let found = uses(
            "course",
            &[
                "project",
                "courses",
                "sheet",
                "numbers",
                "export",
                "render",
                "overprint",
                "placement",
                "describe",
                "print",
            ],
        );
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn render_does_not_know_courses_or_sheets() {
        let found = uses(
            "render",
            &[
                "overprint",
                "sheet",
                "project",
                "course",
                "courses",
                "numbers",
                "export",
                "describe",
                "print",
            ],
        );
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn only_the_xml_loop_of_course_is_shared() {
        // The description symbol reader shares the streaming loop and attribute parsing with the course readers;
        // the note list, text budget and length check stay inside `course` (`pub(super)`).
        let shared = ["Handler", "attr", "drive", "local", "number"];
        let mut seen = Vec::new();
        let outside_course = sources("")
            .into_iter()
            .filter(|f| !f.components().any(|c| c.as_os_str() == "course"));
        for file in outside_course {
            let text = std::fs::read_to_string(&file).unwrap();
            for (i, _) in text.match_indices("course::xml::") {
                let rest = &text[i + "course::xml::".len()..];
                let list = rest
                    .strip_prefix('{')
                    .and_then(|r| r.split_once('}'))
                    .map_or_else(
                        || {
                            rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                                .next()
                                .unwrap_or("")
                        },
                        |(inner, _)| inner,
                    );
                seen.extend(
                    list.split(',')
                        .map(|n| n.trim().to_string())
                        .filter(|n| !n.is_empty()),
                );
            }
        }
        assert!(!seen.is_empty());
        assert!(
            seen.iter().all(|n| shared.contains(&n.as_str())),
            "{seen:?}"
        );
    }

    #[test]
    fn the_check_sees_a_forbidden_path() {
        // The test above would pass on an empty list: one known use proves the walk reads the files.
        assert!(!uses("sheet", &["project"]).is_empty());
        assert!(uses("sheet", &["projec"]).is_empty());
    }
}
