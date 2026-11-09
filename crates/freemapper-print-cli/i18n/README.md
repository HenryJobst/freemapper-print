# Program texts

Every message the program shows comes from `i18n/<language>/freemapper-print.ftl`, a Project Fluent file
(ADR 0024). English is the reference: `en/freemapper-print.ftl` defines every message id, and a missing message in
another language falls back to it.

## Add a language

1. Copy `en/freemapper-print.ftl` to `i18n/<code>/freemapper-print.ftl`, with the two-letter code, for example `fr`.
2. Translate the text after each `=`. Keep the message ids and the variable names (`{ $path }`) as they are.
   Move a variable to wherever the grammar of your language wants it.
3. Add the language to `EMBEDDED` in `src/text.rs`.
4. Run `cargo test`. `same_ids_in_every_language` fails when an id is missing or extra, or when a message does
   not format.

Library errors, operating system errors and text from the map file reach the messages as variables and stay as
they are. Use the domain words from `docs/glossary.md` in your language.
