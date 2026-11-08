# Program texts of freemapper-print, English (reference language, ADR 0024).
# Variable names are the same in every language. Texts from the library crate, from the
# operating system and from the map file arrive in $error, $names and $chars and stay as they are.

## Format

# The character between whole and decimal digits.
decimal-separator = .

## Command line help (clap)

cli-about = Prints finished orienteering maps as exact-scale PDFs
help-lang = Language of the program texts: de or en. Without it, FREEMAPPER_LANG, then LC_ALL, LC_MESSAGES and LANG decide.
render-about = Render a map to a PDF at an exact print scale.
render-after-size = The PDF has 1 mm on paper for the exact map distance at the print scale. Print it at actual size.
render-after-exit-codes = Exit codes: 0 on success, 1 when the map cannot be read or printed, 2 when the command line is wrong.
help-map = The OOM map file (.omap).
help-out = PDF file to write. An existing file is overwritten; its folder must exist.
help-scale = Print scale denominator, e.g. 4000 for 1:4000. The default is the scale of the map.
help-paper = Paper size.
help-orientation = Page orientation. Auto picks the one that cuts off less of the map.
help-kind = Map kind. Needed only when the program cannot tell it from freemapper.toml or the map file.
help-show-sheet-objects = Also draw freemapper's information block: title, scale, scale bar and credits. It scales with the map at another print scale, so its scale text can be wrong then.

## Errors

error-line = Error: { $message }
error-unknown-lang = unknown language "{ $lang }"; available: { $available }
error-read-map = could not read the map { $path }
error-kind-unknown = cannot tell the map kind from the file; pass --kind sprint, --kind schoolyard or --kind forest
error-out-folder-missing = the folder { $path } for the PDF does not exist; create it first
error-out-is-map = --out points at the map file { $path }; choose a PDF path so the map stays safe
error-print = could not print the map
error-write = could not write the PDF

## Notes and warnings after a print

word-landscape = landscape
word-portrait = portrait
report-summary = Wrote { $out }: { $paper } { $orientation }, print scale 1:{ $print_scale }, map scale 1:{ $map_scale }, geometry factor { $geometry }, symbol factor { $symbol }.
note-sidecar = Note: { $error }. The map kind comes from the map file instead.
warning-non-standard-scale = Warning: 1:{ $print_scale } is not a standard scale for { $kind } maps. The map is reduced and the symbols keep their 1:{ $base_scale } size. Use this print for training only.
word-kind-sprint = sprint
word-kind-schoolyard = schoolyard
word-kind-forest = forest
overflow-too-wide = { $mm } mm too wide
overflow-too-tall = { $mm } mm too tall
overflow-and = { $first } and { $second }
hint-paper-a3 = A3 paper
hint-other-orientation = the other orientation
hint-scale-larger = a print scale with a larger number
hint-scale-example = a print scale with a larger number, for example 1:{ $next }
hint-scale-example-non-standard = a print scale with a larger number, for example 1:{ $next } (not standard, for training only)
warning-overflow = Warning: the map does not fit on the page. It is { $cut } for the printable area. Try { $hints }, or { $last }.
warning-overflow-single = Warning: the map does not fit on the page. It is { $cut } for the printable area. Try { $last }.
note-skipped = Note: { $count ->
        [one] 1 object is
       *[other] { $count } objects are
    } not drawn because the symbol is unknown or hidden.
note-unknown-type = Note: { $count ->
        [one] 1 object is
       *[other] { $count } objects are
    } not drawn because the program does not know their object type.
note-truncated = Note: { $count ->
        [one] 1 object was
       *[other] { $count } objects were
    } simplified or cut short because the file asks for more detail than the renderer draws.
note-page-limit = Note: { $count ->
        [one] 1 object is
       *[other] { $count } objects are
    } not drawn because the page reached its drawing limit.
note-dropped-pattern = Note: { $count ->
        [one] 1 object is
       *[other] { $count } objects are
    } drawn without an area pattern whose spacing is below 0.05 mm or not a number.
note-fonts = Note: the font Arimo replaces { $names }.
note-missing-glyphs = Note: the embedded font has no glyph for these characters, so they are missing in the PDF: { $chars }
reminder-actual-size = Print at actual size: 100 %, with "fit to page" switched off. Any other size breaks the scale.
