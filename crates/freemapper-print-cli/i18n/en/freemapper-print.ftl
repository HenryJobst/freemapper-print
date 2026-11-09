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
init-about = Create the project file of an event for a map.
init-after-content = The new file has one layout at the map scale, on A4 or else A3, whichever shows the map without notes, a title block, a scale check bar and a proposed contour interval to confirm. The map kind comes from --kind, from freemapper.toml next to the map, or from the map file.
init-after-exit-codes = Exit codes: 0 on success, 1 when the map cannot be read, the map kind is unknown or the project file exists already, 2 when the command line is wrong.
help-init-folder = Folder for the new freemapper-print.toml. Created if missing.
help-init-map = The OOM map file (.omap). The project file stores its path relative to itself.
help-init-title = Event title on the sheets. The default is the map's file name.
help-init-kind = Map kind. Needed when no freemapper.toml next to the map names it.
check-about = Draw every layout of a project without writing a file and list the notes.
check-after-exit-codes = Exit codes: 0 when no layout has notes (added north lines do not count), 1 when the project cannot be loaded or a layout has notes, 2 when the command line is wrong.
help-project = The project file freemapper-print.toml, or the folder that holds it.
export-about = Write one PDF per layout, named after the layout id.
export-after-exit-codes = Exit codes: 0 when every PDF is written, even with notes, 1 when the project cannot be loaded or a PDF cannot be drawn or written, 2 when the command line is wrong or names an unknown layout.
help-layout = Export only this layout. Repeat it for more. Without it, export writes every layout.
help-out-dir = Folder for the PDFs. It must exist. The default is the folder out next to the project file, created if missing. Export overwrites a PDF of the same name.

## Errors

error-line = Error: { $message }
error-unknown-lang = unknown language "{ $lang }"; available: { $available }
error-read-map = could not read the map { $path }
error-kind-unknown = cannot tell the map kind from the file; pass --kind sprint, --kind schoolyard or --kind forest
error-out-folder-missing = the folder { $path } for the PDF does not exist; create it first
error-out-is-map = --out points at the map file { $path }; choose a PDF path so the map stays safe
error-print = could not print the map
error-write = could not write the PDF
error-load-project = could not load the project { $path }
error-project-exists = { $path } exists already; init does not overwrite it
error-create-folder = could not create the folder { $path }
error-title-control = --title must not contain line breaks, tabs or other control or invisible formatting characters
error-unknown-layout = the project has no layout { $id }; its layouts are { $known }
error-out-dir-missing = the folder { $path } for the PDFs does not exist; create it first
error-sheet = could not draw layout { $layout }
error-disk-full = not enough disk space for { $path }: it needs { $needed } MB including a 50 MB reserve, and { $available } MB are free
error-write-file = could not write { $path }

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

## Project commands: init, check, export

init-written = Wrote { $path } with one layout, { $layout }: { $paper }, passe-partout { $margin } mm, print scale 1:{ $scale }, for { $kind } maps.
init-no-fit = No A4 or A3 sheet shows the map at 1:{ $scale } without notes or errors, so the project keeps A4. check reports this:
init-contour-hint = The project proposes a contour interval of { $value } m, common for { $kind } maps. Look it up on the map and confirm it, or change map.contour_interval_m.
init-no-contour-hint = Schoolyard maps get no proposed contour interval. If the map has contours, set map.contour_interval_m; until then check reports it as missing.
init-next = Run freemapper-print check { $path } next.
# Comments in the new project file.
init-comment-contour = Proposed for { $kind } maps. Confirm it against the map.
init-comment-no-contour = No contour interval is proposed for schoolyard maps. If the map has contours, add contour_interval_m with the interval in metres.
init-comment-footer = The copyright and credit lines go in the footer at the bottom left, below the map frame if the passe-partout is wide enough, else inside it. A copyright_line or credit_line element moves them.
check-summary = Layout { $layout }: { $paper } { $orientation }, print scale 1:{ $print_scale }, map scale 1:{ $map_scale }, geometry factor { $geometry }, symbol factor { $symbol }.
check-clean = Checked { $count ->
        [one] 1 layout
       *[other] { $count } layouts
    }, no notes.
check-with-notes = Layouts with notes: { $count } of { $total }.

## Sheet notes

list-and = { $first } and { $last }
note-element-outside-page = Note: the element { $id } reaches past the page edge and is cut off there.
note-footer-outside-page = Note: the footer with the copyright and credit lines reaches past the page edge and is cut off there.
note-element-overlaps-map = Note: the element { $id } hangs on the page and covers part of the map frame, so the map beneath it does not show.
note-footer-overlaps-map = Note: the passe-partout is too narrow for the footer with the copyright and credit lines. The footer sits inside the map frame on a white box and covers part of the map. A wider margin_mm or a copyright_line or credit_line element moves it.
note-larger-than-a3 = Note: the paper is larger than A3, which most office printers cannot take.
note-missing-scale-or-contour = Note: layout { $layout } lacks { $missing }.
missing-print-scale-text = a text with { $name } for the print scale
missing-contour-text = a text with { $name } for the contour interval
missing-contour-value = a value for { $name }, the contour interval in metres
note-north-lines-added = Note: the map has no north lines, so { $count ->
        [one] 1 line was
       *[other] { $count } lines were
    } added. They run unbroken across the map frame. Breaking them at small features such as boulders or path ends is the mapper's job in OOM.
note-north-lines-without-symbol = Note: the map has neither north lines nor a usable line symbol 601, so { $count ->
        [one] 1 plain black line was
       *[other] { $count } plain black lines were
    } drawn over the map.
note-copyright-truncated = Note: layout { $layout } shows { $shown } of { $total } copyright lines. One more line counts the rest.
note-logo-skipped = Note: the logo { $id } is left out: { $reason }
note-logo-simplified = Note: the logo { $id } is drawn simplified: { $reason }
note-unknown-placeholder = Note: the text { $id } contains { $name }, which is no placeholder, so it is printed as written.
note-unknown-part = Note: layout { $layout } hides the map part { $name }, but the map has no part of that name. Upper and lower case count.
note-credit-line-moved = Note: the credit line element { $id } would not lie wholly on the page, so the credit line stays in the footer at the bottom left.
note-render-footer-on-map = Note: the footer with the copyright and credit lines sits on a white box in the bottom-left corner of the printable area and covers part of the map. In a project (init, then export) it can go below the map frame.
note-render-copyright-truncated = Note: the footer shows { $shown } of { $total } copyright lines. One more line counts the rest.

## Why a logo is left out or drawn simpler (after "the logo … is left out:" or "drawn simplified:")

logo-reason-needs-file-and-width = it needs both file and width_mm.
logo-reason-not-found = { $file } does not exist.
logo-reason-unreadable = { $file } cannot be read ({ $error }).
logo-reason-not-a-file = { $file } is not a regular file.
logo-reason-file-too-large = { $file } is larger than 10 MB.
logo-reason-unknown-format = { $file } is not a PNG, JPEG or SVG file.
logo-reason-too-many-pixels = { $file } has { $width } × { $height } pixels, more than 25 megapixels. Save it with fewer pixels.
logo-reason-too-wide = { $file } is { $width } × { $height } pixels, wider or higher than 65 535 pixels.
logo-reason-undecodable = { $file } cannot be decoded ({ $error }).
logo-reason-decoder-limit = { $file } is too large to decode. Save it as an 8-bit image or with fewer pixels.
logo-reason-svg-not-utf8 = { $file } is not UTF-8 text.
logo-reason-svg-unreadable = { $file } cannot be read as SVG ({ $error }).
logo-reason-svg-unsupported = { $file } uses something a logo cannot have: { $detail }. Export the logo as a plain SVG with shapes, paths and gradients.
logo-reason-svg-too-complex = { $file } is too complex for a logo: { $detail }. Simplify the logo or export it as a plain SVG.
logo-reason-cannot-size = the width or height of { $file } cannot be worked out.
logo-reason-pixel-budget = the sheet holds 50 megapixels of logos already ({ $file }). Use smaller images or fewer logos.
logo-reason-svg-budget = the SVG logos of the sheet have used up their budget of 1 000 000 units already ({ $file }). Simplify them or use fewer.
logo-simplified-gradient = a gradient is drawn in its first colour.
logo-simplified-pattern = a pattern fill is drawn in black.
logo-simplified-opacity = transparency is ignored, the logo is drawn opaque.
logo-simplified-dashes = dashes are drawn as solid lines.
logo-simplified-image = an embedded image is left out.
logo-simplified-text = text is left out; convert it to paths.
logo-simplified-clip-path = a clip path is ignored.
note-missing-scale-check-bar = Note: layout { $layout } has no scale check bar. An element of kind scale_check_bar shows on the print whether the printer scaled it.
