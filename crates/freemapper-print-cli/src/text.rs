//! Program texts from Fluent files (ADR 0024, ADR 0025).
//!
//! Every user-facing text lives in `i18n/<language>/freemapper-print.ftl`. The files are embedded
//! with `include_str!` and handed to the shared [`freemapper_text::Catalog`], which loads them,
//! picks the language (`--lang`, `FREEMAPPER_LANG`, `LC_ALL`, `LC_MESSAGES`, `LANG`) and falls
//! back to English, and to the id when no language has the message. This module keeps only the
//! thin wrappers the program calls: [`Texts`], [`detect_lang`] and [`available`]. An unknown
//! `--lang` value is an [`UnknownLang`] error, a usage error for the caller; an unknown
//! environment value passes to the next source.

use fluent_bundle::FluentArgs;
pub use freemapper_text::Lang;
use freemapper_text::{Catalog, FluentValue};

/// The embedded catalogs, English first.
pub static TEXTS: Catalog = Catalog::new(&[
    ("en", include_str!("../i18n/en/freemapper-print.ftl")),
    ("de", include_str!("../i18n/de/freemapper-print.ftl")),
]);

/// A `--lang` value that names no available language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownLang {
    /// The value as given.
    pub given: String,
    /// The languages the program has texts for.
    pub available: Vec<Lang>,
}

/// The languages the program has texts for, English first.
pub fn available() -> Vec<Lang> {
    TEXTS.available()
}

/// Picks the language: `--lang`, else `FREEMAPPER_LANG`, else `LC_ALL`, `LC_MESSAGES`, `LANG`,
/// else English. Environment values naming no available language pass to the next source; a
/// `--lang` value naming none is an error. `env` reads one environment variable
/// (`|name| std::env::var(name).ok()` in the programs).
pub fn detect_lang(
    arg: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Lang, UnknownLang> {
    if let Some(given) = arg {
        return TEXTS.parse_lang_arg(given).map_err(|_| UnknownLang {
            given: given.to_owned(),
            available: available(),
        });
    }
    Ok(TEXTS.detect_lang(None, env))
}

/// The program texts in one language, with English as the fallback.
pub struct Texts {
    lang: Lang,
}

impl Texts {
    pub fn new(lang: &Lang) -> Texts {
        Texts { lang: lang.clone() }
    }

    /// The text `id`, falling back to English, and to the id itself as a last resort.
    pub fn get(&self, id: &str, args: Option<&FluentArgs>) -> String {
        let args: Vec<(&str, FluentValue)> = args
            .map(|a| {
                a.iter()
                    .map(|(name, value)| (name, value.clone()))
                    .collect()
            })
            .unwrap_or_default();
        TEXTS.text(&self.lang, id, &args)
    }

    /// The language of the texts.
    pub fn lang(&self) -> &Lang {
        &self.lang
    }

    /// A text without variables.
    pub fn plain(&self, id: &str) -> String {
        self.get(id, None)
    }

    /// A number with the decimal separator of the language, from a `format!` result with a dot.
    pub fn decimal(&self, formatted: &str) -> String {
        formatted.replace('.', &self.plain("decimal-separator"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prefixes of the message ids the CLI sources name as string literals.
    const ID_PREFIXES: &[&str] = &[
        "check-",
        "cli-",
        "decimal-",
        "error-",
        "export-",
        "fit-",
        "help-",
        "hint-",
        "init-",
        "list-",
        "logo-",
        "masks-",
        "missing-",
        "note-",
        "number-",
        "numbers-",
        "overflow-",
        "quoted-",
        "reminder-",
        "render-",
        "report-",
        "stale-",
        "warning-",
        "word-",
    ];

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    fn de() -> Lang {
        TEXTS.parse_lang_arg("de").unwrap()
    }

    #[test]
    fn language_detection_matches_before() {
        // (--lang, environment, expected language code or None for a usage error)
        type Case<'a> = (Option<&'a str>, &'a [(&'a str, &'a str)], Option<&'a str>);
        let cases: &[Case] = &[
            (
                Some("de"),
                &[("FREEMAPPER_LANG", "en"), ("LANG", "de_DE.UTF-8")],
                Some("de"),
            ),
            (Some("DE"), &[], Some("de")),
            (Some("en"), &[("LANG", "de_DE")], Some("en")),
            (Some("xx"), &[("LANG", "de_DE")], None),
            (Some("fr"), &[("LANG", "de_DE")], None),
            (Some(""), &[], None),
            (Some("C"), &[], None),
            (
                None,
                &[("FREEMAPPER_LANG", "en"), ("LANG", "de_DE.UTF-8")],
                Some("en"),
            ),
            (None, &[("LANG", "de_DE.UTF-8")], Some("de")),
            (None, &[("LANG", "en_US")], Some("en")),
            (None, &[("LANG", "C")], Some("en")),
            (None, &[("LANG", "POSIX")], Some("en")),
            (None, &[("LANG", "xx")], Some("en")),
            (
                None,
                &[("FREEMAPPER_LANG", "fr"), ("LANG", "de_DE.UTF-8")],
                Some("de"),
            ),
            (None, &[("LC_ALL", "C"), ("LANG", "de_DE")], Some("de")),
            (
                None,
                &[("LC_ALL", ""), ("LC_MESSAGES", "POSIX"), ("LANG", "de_AT")],
                Some("de"),
            ),
            (
                None,
                &[("LC_ALL", "en_US.UTF-8"), ("LANG", "de_DE")],
                Some("en"),
            ),
            (
                None,
                &[("LC_MESSAGES", "de-DE"), ("LANG", "en")],
                Some("de"),
            ),
            (None, &[], Some("en")),
        ];
        for (arg, pairs, expected) in cases {
            let got = detect_lang(*arg, &env_of(pairs)).ok().map(|l| l.code());
            assert_eq!(got.as_deref(), *expected, "arg {arg:?}, env {pairs:?}");
        }
    }

    #[test]
    fn catalog_has_no_problems() {
        let args = sample_args();
        let owned: Vec<(&str, FluentValue)> = args.iter().map(|(n, v)| (n, v.clone())).collect();
        let problems = TEXTS.problems(&owned);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn every_id_used_in_code_exists() {
        let ids = TEXTS.message_ids("en");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut scanned = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            scanned += 1;
            for id in freemapper_text::ids_in_source(&source, ID_PREFIXES) {
                assert!(
                    ids.contains(&id),
                    "{} uses unknown message id {id}",
                    path.display()
                );
            }
        }
        assert!(
            scanned >= 2,
            "expected the CLI sources under {}",
            dir.display()
        );
    }

    #[test]
    fn missing_id_returns_the_id() {
        assert_eq!(
            Texts::new(&Lang::english()).plain("no-such-message"),
            "no-such-message"
        );
    }

    #[test]
    fn unknown_lang_argument_is_an_error() {
        let env = env_of(&[("LANG", "de_DE")]);
        let err = detect_lang(Some("fr"), &env).unwrap_err();
        assert_eq!(err.given, "fr");
        assert_eq!(err.available, vec![Lang::english(), de()]);
        assert!(detect_lang(Some(""), &env).is_err());
        assert!(detect_lang(Some("C"), &env).is_err());
    }

    /// Sample values for every variable the catalogs use (a number where a selector needs one).
    fn sample_args() -> FluentArgs<'static> {
        let mut args = FluentArgs::new();
        for name in [
            "message",
            "lang",
            "available",
            "path",
            "out",
            "paper",
            "orientation",
            "print_scale",
            "map_scale",
            "geometry",
            "symbol",
            "error",
            "kind",
            "base_scale",
            "mm",
            "first",
            "second",
            "next",
            "cut",
            "hints",
            "last",
            "names",
            "chars",
            "id",
            "layout",
            "missing",
            "name",
            "shown",
            "total",
            "applied",
            "reason",
            "value",
            "scale",
            "needed",
            "available",
            "known",
            "margin",
            "file",
            "field",
            "detail",
            "line",
            "content",
            "width",
            "height",
            "course",
            "controls",
            "variants",
            "family",
            "code",
            "metres",
            "reason",
            "crs",
            "control",
            "scope",
            "page",
            "text",
            "conflicts",
            "direction",
            "gap",
            "from",
            "to",
            "min",
            "max",
            "proposal",
            "courses",
            "set",
            "mask",
            "objects",
            "x",
            "y",
            "which",
            "problem",
            "radius",
            "points",
            "scales",
            "ids",
            "codes",
        ] {
            args.set(name, FluentValue::from("sample"));
        }
        args.set("count", FluentValue::from(3_i64));
        args.set("layouts", FluentValue::from(2_i64));
        args.set("layouts_with_notes", FluentValue::from(1_i64));
        args.set("assignments", FluentValue::from(3_i64));
        args
    }

    #[test]
    fn no_isolation_marks() {
        for lang in available() {
            let texts = Texts::new(&lang);
            let mut args = FluentArgs::new();
            args.set("message", "boom");
            let out = texts.get("error-line", Some(&args));
            assert!(out.contains("boom"), "{out}");
            assert!(!out.contains(['\u{2068}', '\u{2069}']), "{out:?}");
        }
    }

    #[test]
    fn plural_and_decimal_follow_the_language() {
        let one = |texts: &Texts, n: i64| {
            let mut args = FluentArgs::new();
            args.set("count", n);
            texts.get("note-skipped", Some(&args))
        };
        let en = Texts::new(&Lang::english());
        assert!(one(&en, 1).starts_with("Note: 1 object is not drawn"));
        assert!(one(&en, 3).starts_with("Note: 3 objects are not drawn"));
        let german = Texts::new(&de());
        assert!(one(&german, 1).starts_with("Hinweis: 1 Objekt wird nicht"));
        assert!(one(&german, 3).starts_with("Hinweis: 3 Objekte werden nicht"));
        assert_eq!(en.decimal("1.333"), "1.333");
        assert_eq!(german.decimal("1.333"), "1,333");
    }
}
