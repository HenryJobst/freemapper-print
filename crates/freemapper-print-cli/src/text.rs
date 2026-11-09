//! Program texts from Fluent files (ADR 0024).
//!
//! Copied from freemapper-courseplan cf1dae0 (branch `m2a-editing`, `src/text.rs`; shared crate
//! freemapper-i18n later). Adapted: file name, `Texts` wrapper, `detect_lang` reports an unknown
//! `--lang` value as an error, courseplan's error-type messages are left out.
//!
//! Every user-facing text lives in `i18n/<language>/freemapper-print.ftl`. The files are embedded
//! with `include_str!`, so adding a language means adding a folder, listing it in `EMBEDDED` and
//! running the tests. English is the reference and the fallback: a message missing in another
//! language is taken from English, and a message missing everywhere comes back as its id.
//!
//! # Language detection
//!
//! [`detect_lang`] looks at these sources in order and takes the first one that names an
//! available language:
//!
//! 1. the `--lang` argument,
//! 2. `FREEMAPPER_LANG`,
//! 3. `LC_ALL`, 4. `LC_MESSAGES`, 5. `LANG`.
//!
//! Only the language part counts, case-insensitively: `de`, `de-DE`, `de_DE.UTF-8` and `de_AT`
//! all mean German. An unset or empty variable, `C`, `POSIX` and a language the program does not
//! have (`fr`) pass to the next source, so `FREEMAPPER_LANG=fr` with `LANG=de_DE.UTF-8` gives
//! German. If nothing matches, the result is English. A `--lang` value that names no available
//! language is an [`UnknownLang`] error, a usage error for the caller.

use std::sync::OnceLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

/// A language of the program texts, identified by its language part (`en`, `de`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lang(pub LanguageIdentifier);

impl Lang {
    /// English, the reference language and the fallback.
    pub fn english() -> Lang {
        lang_of("en")
    }

    /// The language part as text, for example `de`.
    pub fn code(&self) -> String {
        self.0.language.to_string()
    }
}

/// A `--lang` value that names no available language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownLang {
    /// The value as given.
    pub given: String,
    /// The languages the program has texts for.
    pub available: Vec<Lang>,
}

/// The embedded catalogs: language code and file content. English comes first.
const EMBEDDED: &[(&str, &str)] = &[
    ("en", include_str!("../i18n/en/freemapper-print.ftl")),
    ("de", include_str!("../i18n/de/freemapper-print.ftl")),
];

type Bundle = FluentBundle<FluentResource>;

fn lang_of(code: &str) -> Lang {
    Lang(LanguageIdentifier::from_parts(
        code.parse().expect("valid language code"),
        None,
        None,
        &[],
    ))
}

/// The languages the program has texts for, English first.
pub fn available() -> Vec<Lang> {
    EMBEDDED.iter().map(|(code, _)| lang_of(code)).collect()
}

fn build_bundle(code: &str, source: &str) -> Bundle {
    // A file with syntax errors still yields its valid messages; the tests refuse such files.
    let resource = match FluentResource::try_new(source.to_owned()) {
        Ok(resource) => resource,
        Err((resource, _)) => resource,
    };
    let mut bundle = FluentBundle::new_concurrent(vec![lang_of(code).0]);
    // Isolation marks would show up as stray characters in terminals.
    bundle.set_use_isolating(false);
    // Overriding is impossible with a single resource; ignore the (empty) error list.
    let _ = bundle.add_resource(resource);
    bundle
}

fn bundles() -> &'static [(String, Bundle)] {
    static BUNDLES: OnceLock<Vec<(String, Bundle)>> = OnceLock::new();
    BUNDLES.get_or_init(|| {
        EMBEDDED
            .iter()
            .map(|(code, source)| ((*code).to_owned(), build_bundle(code, source)))
            .collect()
    })
}

fn bundle_of(lang: &Lang) -> Option<&'static Bundle> {
    let code = lang.code();
    bundles().iter().find(|(c, _)| *c == code).map(|(_, b)| b)
}

fn english_bundle() -> &'static Bundle {
    bundle_of(&Lang::english()).expect("English is embedded")
}

/// The language part of a locale value such as `de_DE.UTF-8`, `de-AT` or `en`; `None` for an
/// empty value, `C` and `POSIX`.
fn language_part(value: &str) -> Option<String> {
    let head = value.split(['.', '@']).next().unwrap_or("");
    let language = head.split(['_', '-']).next().unwrap_or("").trim();
    if language.is_empty()
        || language.eq_ignore_ascii_case("c")
        || language.eq_ignore_ascii_case("posix")
    {
        None
    } else {
        Some(language.to_ascii_lowercase())
    }
}

fn match_available(value: &str) -> Option<Lang> {
    let language = language_part(value)?;
    available().into_iter().find(|l| l.code() == language)
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
        return match_available(given.trim()).ok_or_else(|| UnknownLang {
            given: given.to_owned(),
            available: available(),
        });
    }
    Ok(["FREEMAPPER_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|name| env(name).and_then(|v| match_available(&v)))
        .unwrap_or_else(Lang::english))
}

fn format_in(bundle: &Bundle, id: &str, args: Option<&FluentArgs>) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    Some(
        bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned(),
    )
}

/// Looks `id` up in the bundles in order; the id itself when none has it.
fn lookup(chain: &[&Bundle], id: &str, args: Option<&FluentArgs>) -> String {
    chain
        .iter()
        .find_map(|bundle| format_in(bundle, id, args))
        .unwrap_or_else(|| id.to_owned())
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
        match bundle_of(&self.lang) {
            Some(bundle) => lookup(&[bundle, english_bundle()], id, args),
            None => lookup(&[english_bundle()], id, args),
        }
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
    use fluent_bundle::FluentValue;
    use std::collections::BTreeSet;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    fn de() -> Lang {
        lang_of("de")
    }

    #[test]
    fn detect_order() {
        let env = env_of(&[("FREEMAPPER_LANG", "en"), ("LANG", "de_DE.UTF-8")]);
        assert_eq!(detect_lang(Some("de"), &env), Ok(de()));
        assert_eq!(detect_lang(Some("DE"), &env), Ok(de()));
        assert_eq!(detect_lang(None, &env), Ok(Lang::english()));
        let env = env_of(&[("LANG", "de_DE.UTF-8")]);
        assert_eq!(detect_lang(None, &env), Ok(de()));
        assert_eq!(detect_lang(Some("en"), &env), Ok(Lang::english()));
        // FREEMAPPER_LANG before LC_ALL before LC_MESSAGES before LANG.
        let env = env_of(&[
            ("FREEMAPPER_LANG", "de"),
            ("LC_ALL", "en"),
            ("LC_MESSAGES", "en"),
            ("LANG", "en"),
        ]);
        assert_eq!(detect_lang(None, &env), Ok(de()));
        let env = env_of(&[("LC_ALL", "de"), ("LC_MESSAGES", "en"), ("LANG", "en")]);
        assert_eq!(detect_lang(None, &env), Ok(de()));
        let env = env_of(&[("LC_ALL", "en_US.UTF-8"), ("LANG", "de_DE")]);
        assert_eq!(detect_lang(None, &env), Ok(Lang::english()));
        let env = env_of(&[("LC_MESSAGES", "de-DE"), ("LANG", "en")]);
        assert_eq!(detect_lang(None, &env), Ok(de()));
        // Unknown, empty, C and POSIX values fall through to the next source.
        let env = env_of(&[("FREEMAPPER_LANG", "fr"), ("LANG", "de_DE.UTF-8")]);
        assert_eq!(detect_lang(None, &env), Ok(de()));
        let env = env_of(&[("LC_ALL", ""), ("LC_MESSAGES", "POSIX"), ("LANG", "de_AT")]);
        assert_eq!(detect_lang(None, &env), Ok(de()));
        let env = env_of(&[("LC_ALL", "C"), ("LANG", "de_DE")]);
        assert_eq!(detect_lang(None, &env), Ok(de()));
        // Nothing matches: English.
        assert_eq!(
            detect_lang(None, &env_of(&[("LANG", "C")])),
            Ok(Lang::english())
        );
        assert_eq!(detect_lang(None, &env_of(&[])), Ok(Lang::english()));
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

    fn message_ids(source: &str) -> BTreeSet<String> {
        use fluent_syntax::ast::Entry;
        let resource = fluent_syntax::parser::parse(source).expect("file parses without errors");
        resource
            .body
            .iter()
            .filter_map(|entry| match entry {
                Entry::Message(m) => Some(m.id.name.to_owned()),
                _ => None,
            })
            .collect()
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
            "reason",
            "value",
            "scale",
            "needed",
            "available",
            "known",
            "margin",
            "file",
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
        ] {
            args.set(name, FluentValue::from("sample"));
        }
        args.set("count", FluentValue::from(3_i64));
        args
    }

    #[test]
    fn same_ids_in_every_language() {
        let english = message_ids(EMBEDDED[0].1);
        assert!(!english.is_empty());
        for (code, source) in EMBEDDED {
            let ids = message_ids(source);
            assert_eq!(ids, english, "message ids of {code} differ from English");
            let bundle = bundle_of(&lang_of(code)).unwrap();
            let args = sample_args();
            for id in &ids {
                let pattern = bundle.get_message(id).unwrap().value().unwrap();
                let mut errors = Vec::new();
                let out = bundle.format_pattern(pattern, Some(&args), &mut errors);
                assert!(errors.is_empty(), "{code}/{id}: {errors:?}");
                assert!(!out.trim().is_empty(), "{code}/{id} is empty");
            }
        }
    }

    #[test]
    fn fallback_to_english() {
        let partial = build_bundle("de", "only-german = Nur Deutsch");
        let chain = [&partial, english_bundle()];
        assert_eq!(lookup(&chain, "only-german", None), "Nur Deutsch");
        assert_eq!(lookup(&chain, "help-paper", None), "Paper size.");
        assert_eq!(lookup(&chain, "no-such-message", None), "no-such-message");
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
