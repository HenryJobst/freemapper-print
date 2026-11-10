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
check-about = Draw every layout of a project, or every course when it has a course file, without writing a file, and list the notes.
check-after-exit-codes = Exit codes: 0 when no layout or course has notes (notes marked Info do not count), 1 when the project or its course file cannot be loaded or a layout, a course or the course file has notes, 2 when the command line is wrong.
help-project = The project file freemapper-print.toml, or the folder that holds it.
export-about = Write one PDF per layout, named after the layout id. With a course file, write one PDF per course, named after the course, on the layout its assignment names.
export-after-exit-codes = Exit codes: 0 when every PDF is written, even with notes, 1 when the project or its course file cannot be loaded or a PDF cannot be drawn or written, 2 when the command line is wrong or names an unknown layout or course.
help-layout = Export only this layout, without courses. Repeat it for more. Without it, export writes every layout, or every course when the project has a course file.
help-course = Export only this course. Repeat it for more. Without it, export writes every course of the course file.
help-single-file = Write all courses into one PDF, one page each, named after the event title.
help-out-dir = Folder for the PDFs. It must exist. The default is the folder out next to the project file, created if missing. Export overwrites a PDF of the same name.

## Errors

error-line = Error: { $message }
error-unknown-lang = unknown language "{ $lang }"; available: { $available }
error-read-map = could not read the map { $path }
error-kind-unknown = cannot tell the map kind from the file; pass --kind sprint, --kind schoolyard or --kind forest
error-out-folder-missing = the folder { $path } for the PDF does not exist; create it first
error-out-is-map = --out points at the map file { $path }; choose a PDF path so the map stays safe
error-print = could not print the map
error-load-project = could not load the project { $path }
error-base-part-hidden = { $path }: field `{ $field }`: the map part "base" is the base area of a sprint map (walkable area) and is always printed; remove "base" from hide_parts
error-project-exists = { $path } exists already; init does not overwrite it
error-create-folder = could not create the folder { $path }
error-title-control = --title must not contain line breaks, tabs or other control or invisible formatting characters
error-unknown-layout = the project has no layout { $id }; its layouts are { $known }
error-out-dir-missing = the folder { $path } for the PDFs does not exist; create it first
error-sheet = could not draw layout { $layout }
error-course-sheet = could not draw course "{ $course }"
error-courses = could not use the course file
error-unknown-course = the course file has no course "{ $name }"; its courses are { $known }
error-no-course-file = --course and --single-file need a course file, and the project names none under [sources] courses
error-disk-full = not enough disk space for { $path }: it needs { $needed } MB including a 50 MB reserve, and { $available } MB are free
error-write-file = could not write { $path }
error-outside-project = { $path }: a link leads out of the project folder, so nothing is written there; replace the link with a real folder, or give the folder it leads to with --out-dir

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
init-no-fit = Warning: no A4 or A3 sheet shows the map at 1:{ $scale } without notes or errors, so the project keeps A4. check reports this:
init-contour-hint = The project proposes a contour interval of { $value } m, common for { $kind } maps. Look it up on the map and confirm it, or change map.contour_interval_m.
init-no-contour-hint = Schoolyard maps get no proposed contour interval. If the map has contours, set map.contour_interval_m; until then check reports it as missing.
init-next = Run freemapper-print check { $path } next.
init-kind-flag = Map kind: { $kind } maps, as --kind says.
init-kind-sidecar = Map kind: { $kind } maps, as freemapper.toml next to the map says.
init-kind-inferred = Map kind: { $kind } maps, inferred from the map's symbol set and scale. If that is wrong, change map.kind in the project file.
init-fitting = Looking for an A4 or A3 sheet that shows the map at 1:{ $scale } …
init-next-with-notes = Run freemapper-print check { $path } next. It lists the notes above until the layout changes.
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
report-course-summary = Wrote { $out } for course "{ $course }" on layout { $layout }: { $paper } { $orientation }, print scale 1:{ $print_scale }, map scale 1:{ $map_scale }, geometry factor { $geometry }, symbol factor { $symbol }.
report-course-page = Page { $page }, course "{ $course }" on layout { $layout }: { $paper } { $orientation }, print scale 1:{ $print_scale }, map scale 1:{ $map_scale }, geometry factor { $geometry }, symbol factor { $symbol }.
report-single-file = Wrote { $out } with { $count ->
        [one] 1 page
       *[other] { $count } pages
    }.
check-course-summary = Course "{ $course }" on layout { $layout }: { $paper } { $orientation }, print scale 1:{ $print_scale }, map scale 1:{ $map_scale }, geometry factor { $geometry }, symbol factor { $symbol }.
check-courses-clean = Checked { $count ->
        [one] 1 course
       *[other] { $count } courses
    } and { $layouts ->
        [one] 1 layout sheet
       *[other] { $layouts } layout sheets
    }, no notes.
check-courses-with-notes = With notes: { $count } of { $total } courses, { $layouts_with_notes } of { $layouts } layout sheets.
check-course-file-notes = The course file itself has notes: see the lines above the first course.

## Sheet notes

list-and = { $first } and { $last }
note-element-outside-page = Note: the element { $id } reaches past the page edge and is cut off there.
note-footer-outside-page = Note: the footer with { $content ->
        [copyright] the copyright lines
        [credit] the credit line
       *[both] the copyright and credit lines
    } reaches past the page edge and is cut off there.
note-element-overlaps-map = Note: the element { $id } hangs on the page and covers part of the map frame, so the map beneath it does not show.
note-footer-overlaps-map = Note: the passe-partout is too narrow for the footer with { $content ->
        [copyright] the copyright lines
        [credit] the credit line
       *[both] the copyright and credit lines
    }. The footer sits inside the map frame on a white box and covers part of the map. A wider margin_mm or a copyright_line or credit_line element moves it.
note-larger-than-a3 = Note: the paper is larger than A3, which most office printers cannot take.
note-missing-scale-or-contour = Note: layout { $layout } { $count ->
        [one] lacks
       *[other] lacks
    } { $missing }.
missing-print-scale-text = a text with { $name } for the print scale
missing-contour-text = a text with { $name } for the contour interval
missing-contour-value = a value for { $name }, the contour interval in metres
note-north-lines-added = Info: the map has no north lines, so { $count ->
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
note-render-footer-on-map = Note: the footer with { $content ->
        [copyright] the copyright lines
        [credit] the credit line
       *[both] the copyright and credit lines
    } sits on a white box in the bottom-left corner of the printable area and covers part of the map. In a project (init, then export) it can go below the map frame.
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
note-footer-covers-element = Note: the footer with { $content ->
        [copyright] the copyright lines
        [credit] the credit line
       *[both] the copyright and credit lines
    } at the bottom left covers part of the element { $id }. Move the element, or move the footer lines with a copyright_line or credit_line element.
error-disk-full-unknown = not enough disk space for { $path }: it needs { $needed } MB including a 50 MB reserve, and the disk does not say how much is free
note-copyright-line-cut = Note: the copyright line "{ $line }" is wider than the map frame and is cut short, ending in "…". Shorten it in the map notes or in print.copyright.
hint-crop = For a sheet that shows only part of the map, map_frame.centre_map_mm picks the part and map_frame.crop = true marks the cut as intended.
note-map-cropped = Info: the map is { $cut } for the map frame and is cut there on purpose (map_frame.crop = true).

## Course sheets

note-course-outside-frame = Note: on the sheet of course "{ $course }", { $count ->
        [one] control { $controls } lies
       *[other] controls { $controls } lie
    } outside the map frame or across its edge. Without map_frame.centre_map_mm the frame centres on the course; larger paper or a print scale with a larger number shows more.
note-default-layout = Info: course "{ $course }" has no assignment and prints on the first layout, { $layout }.
note-no-description-box-iof-xml = Info: course "{ $course }" comes from IOF XML, which has no control descriptions, so its sheet has no description box.
note-no-description-box-no-element = Info: the layout of course "{ $course }" has no control_descriptions element, so its sheet has no description box.
note-no-description-box-outside-page = Note: the description box of course "{ $course }" would reach past the page edge, so the sheet leaves it out. Move it with its anchor or make it smaller with cell_mm.
note-no-description-box-no-descriptions = Note: no control of course "{ $course }" has descriptions in the course file, so its sheet has no description box. Enter them in Purple Pen.
note-element-covers-course = Note: the element { $id } covers { $count ->
        [one] control { $controls }
       *[other] controls { $controls }
    } of the course. Move the element with its anchor so that runners can see the course.
note-unknown-description-symbol = Note: the descriptions of course "{ $course }" use symbol { $id }, which the course file's description standard does not have. Its cell in the description box stays empty.
note-overprint-cut = Note: the course overprint of course "{ $course }" asks for more hatch lines, dashes or crosses than one sheet draws, so some are left out.
note-masks-capped = Note: course "{ $course }" has { $total } masks, and a sheet applies at most { $applied }. The others are left out, so the map shows where only they would hide it.
note-overprint-in-masked-area = Note: on the sheet of course "{ $course }", { $objects } { $count ->
        [one] lies
       *[other] lie
    } wholly or in part where the mask hides the map. The overprint is printed there all the same, with no map under it. Add or widen a window, or check that the course has the right mask.
masked-marked-route = { $count ->
        [one] a marked route
       *[other] { $count } marked routes
    }
masked-crossing-point = { $count ->
        [one] a crossing point
       *[other] { $count } crossing points
    }
masked-forbidden-route = { $count ->
        [one] a forbidden route
       *[other] { $count } forbidden routes
    }
masked-boundary = { $count ->
        [one] an out-of-bounds boundary
       *[other] { $count } out-of-bounds boundaries
    }
masked-out-of-bounds-area = { $count ->
        [one] an out-of-bounds area
       *[other] { $count } out-of-bounds areas
    }
masked-dangerous-area = { $count ->
        [one] a dangerous area
       *[other] { $count } dangerous areas
    }
masked-text = { $count ->
        [one] a text from the course file
       *[other] { $count } texts from the course file
    }
note-control-in-masked-area = Note: on the sheet of course "{ $course }", { $count ->
        [one] control { $controls } lies
       *[other] controls { $controls } lie
    } where the mask hides the map, so runners find no map around { $count ->
        [one] it
       *[other] them
    }. Add a window there, or check that the course has the right mask.
note-mask-covers-nothing = Info: the mask "{ $mask }" hides nothing in the map frame of course "{ $course }". If it should, check its points. They are map mm with y pointing up, as OOM shows them.
note-mask-leaves-no-window = Note: the mask "{ $mask }" hides the whole map frame of course "{ $course }", so the sheet shows no map at all. Check its windows and its points. They are map mm with y pointing up, as OOM shows them.
note-mask-check-limited = Note: on the sheet of course "{ $course }" the check against the mask reached its limit, so part of the overprint was not checked. Look at the sheet for course symbols with no map under them.
note-course-value-unknown = Info: the text { $id } uses { $name }, but the course file has no value for course "{ $course }", so it prints empty.
note-number-conflict = Note: on the sheet of course "{ $course }" the number of control { $control } covers { $conflicts }. It is printed there all the same. { $proposal } To use that position, run numbers accept with --course "{ $course }", --control { $control } and --sheet or --layout-wide.
note-number-gap-out-of-range = Note: on the sheet of course "{ $course }" the number of control { $control } is { $gap } mm from its circle. The rule profile allows { $min } to { $max } mm. It is printed with this gap all the same; a correction under [[numbers]] in the project file can change it.
number-conflict-symbol = the symbol of control { $control }
number-conflict-leg = the leg from { $from } to { $to }
number-conflict-number = the number of control { $control }
number-conflict-element = the element { $id }
number-conflict-outside = the edge of the map frame
number-conflict-point-symbol = { $count ->
        [one] the point symbol { $code }
       *[other] { $count } objects of the point symbol { $code }
    } of the map
number-conflict-line-symbol = { $count ->
        [one] the line symbol { $code }
       *[other] { $count } objects of the line symbol { $code }
    } of the map
note-number-conflict-unsearched = Note: on the sheet of course "{ $course }" the number of control { $control } covers { $conflicts }. It is printed there all the same. The sheet reached its search limit, so there is no proposal. Set a position with numbers accept, --direction and --gap.
note-number-search-limited = Note: on the sheet of course "{ $course }" the search for number positions reached its limit. The numbers it did not get to stay where their level puts them, automatic ones to the upper right of their circle, and get no proposal. Check the sheet.
number-proposal-free = A free position is direction { $direction }° with a gap of { $gap } mm.
number-proposal-none = There is no free place; the least covered one is direction { $direction }° with a gap of { $gap } mm.

## Notes on the course file

note-course-unknown-element = Note: the course file contains the element { $name }, which this version does not read. It is left out.
note-course-forked = Info: courses { $variants } are the variants of fork "{ $family }". Each prints as a sheet of its own.
note-course-alternative-controls = Note: course "{ $course }" offers alternative controls at one stop. This version cannot print that, so the course is left out.
note-course-notes-truncated = Info: the course file gave more notes than are listed here.
note-course-white-out = Info: the course file has white-out areas. Each one hides the map on the courses it belongs to, unless a course's assignment names a mask of its own.
note-course-no-descriptions = Info: the course file has no control descriptions.
note-course-map-issue-as-start = Info: the map issue point { $code } is used as the start.
note-course-variations = Note: course "{ $course }" has forks or loops (Purple Pen variations). This version cannot print them, so the course is left out. Purple Pen's IOF XML export writes each variation as a course of its own.
note-fit-y-up = Info: the course file's y axis points up; the program turned it to match the map.
note-fit-orientation-assumed = Note: the course fits the map with its y axis pointing up and pointing down, and nothing in the file decides, so the program assumes it points up. Check a sheet, then set course_y_axis = "up" or "down" under [sources] in the project file.
note-fit-axis-against-format = Note: the project file sets course_y_axis = "{ $set }" under [sources], but Purple Pen always writes the y axis pointing up, so the course prints mirrored. If the file comes from Purple Pen, delete the line.
note-fit-scale-unchecked = Info: the course file states no map scale, so it was not compared with the map's scale.
note-fit-georef-checked = Info: { $count ->
        [one] 1 control has
       *[other] { $count } controls have
    } a map position and latitude and longitude. They differ by { $metres } m at most.
note-fit-georef-skipped = Info: latitude and longitude were not checked, because { $reason }.
fit-skip-not-georeferenced = the map is not georeferenced
fit-skip-unsupported-crs = the map's coordinate system { $crs } is not UTM, the only one the program supports
fit-skip-no-lat-lng = no control has both a map position and latitude and longitude
fit-skip-check-off = the project file switches the check off
note-fit-georef-check-off = Info: the project file switches the check against latitude and longitude off (course_georef_check = false), so the program ignored them.
note-fit-placed-by-georef = Info: { $count ->
        [one] control { $controls } has
       *[other] controls { $controls } have
    } only latitude and longitude. The map's georeferencing placed them.
note-stale-assignment = Note: the project file assigns course "{ $course }" to a layout, but the course file has no such course. The assignment stays in the project file.
note-stale-correction = Note: the project file corrects the number of control { $control } for { $scope }, but the course file has no such control there. The correction stays in the project file.
stale-scope-layout = layout { $name }
stale-scope-course = course "{ $name }"
note-stale-changed-base = Note: the course file has moved the number of control { $control } since the project file corrected it. Check whether the correction still fits.
note-stale-mask = Note: the project file gives course "{ $course }" the mask "{ $mask }", but the course file has no such white-out area. The course prints without a mask; the assignment stays in the project file.
note-mask-unused = Info: no course uses the mask "{ $id }". Name it in an assignment with mask = "{ $id }", or delete its [[masks]] entry.
note-white-out-by-number = Info: course "{ $course }" prints with the white-out area { $id } of the course file, which lies around { $x }, { $y } (map mm, y up). The number counts the white-out areas in course file order. If you delete one in Purple Pen or their order changes, the same number means another area, so check the assignment then.
note-white-out-stand-in = Info: on the sheet of course "{ $course }", the [[masks]] table with the id { $id } takes the place of the white-out area of the course file that lies around { $x }, { $y } (map mm, y up). The number counts the white-out areas in course file order. If you delete one in Purple Pen or their order changes, the table replaces another area, so check its id then.
note-map-cropped-to-course = Info: the map is { $cut } for the map frame. A course sheet shows the part around the course, so the rest is cut off on purpose.
note-course-placeholder-not-filled = Info: the text { $id } uses { $name }, which is only filled on course sheets. On this sheet it stays empty.
note-no-courses-to-export = Note: the course file has no course to export, so no PDF was written.
quoted = "{ $text }"

# ---- init --courses, numbers accept (M3 Task 11) ----
help-init-courses = The course file, .ppen (Purple Pen) or .xml (IOF XML 3.0). The project stores its path relative to the project file. A .ppen gets one layout per print scale in the file, an assignment per course and a description box; IOF XML gets one layout at the map scale.
numbers-about = Change the positions of control numbers in the project file.
numbers-accept-about = Add a correction for one control number to the project file. Without --direction and --gap it takes the proposal that check shows for that control.
numbers-accept-after-content = The new [[numbers]] table goes to the end of the file as text. Everything before it stays as it is, comments included. The program then loads the file again; if that fails, it puts the old content back.
numbers-accept-after-exit-codes = Exit codes: 0 when the correction is written, 1 when the project or its course file cannot be loaded or written, or the changed file does not load (the old content is put back), 2 when the command line is wrong, the course or control is unknown, check shows no proposal for the control, or the file has a correction for the same control and scope already.
help-numbers-course = Name of the course, as in the course file.
help-numbers-control = Code of the control.
help-numbers-layout-wide = The correction counts for every course that prints on the course's layout.
help-numbers-sheet = The correction counts for this course's sheet only. It beats a layout-wide one.
help-numbers-direction = Direction in degrees, 0 is east, counter-clockwise. Needs --gap. Written with one decimal.
help-numbers-gap = Gap between number and circle in print mm, above 0 and at most 10. Needs --direction. Written with two decimals.
init-courses-read = Read the course file { $path }: { $count ->
        [one] 1 course
       *[other] { $count } courses
    }, fitted to the map.
init-courses-scales-ppen = Print scales come from the course file, one layout per scale. A course without a print scale gets the map scale, 1:{ $scale }.
init-courses-scales-iof = The course file is IOF XML. It has no print scales, so all courses print on one layout at the map scale, 1:{ $scale }. It has no control descriptions either, so the layout gets no description box.
init-courses-description-box = Each layout has a description box in the top-right corner of the map frame, with the cell size of the rule profile.
init-courses-crop = A course sheet cuts the map to the course on purpose, so the layouts have map_frame.crop = true.
init-written-courses = Wrote { $path } with { $layouts ->
        [one] one layout
       *[other] { $layouts } layouts
    } and { $assignments ->
        [one] one course assignment
       *[other] { $assignments } course assignments
    }, for { $kind } maps.
init-courses-layout = Layout { $layout } at 1:{ $scale }: { $paper }, passe-partout { $margin } mm, for { $courses }.
init-courses-no-fit = Warning: no A4 or A3 sheet shows the map at 1:{ $scale } without notes or errors, so layout { $layout } keeps A4. check reports this.
error-read-courses = could not read the course file { $path }
error-numbers-no-course-file = numbers accept needs a course file, and the project names none under [sources] courses
error-unknown-control = the course "{ $course }" has no control { $control }; its controls are { $known }
error-numbers-no-proposal = there is no proposal for control { $control } on the sheet of course "{ $course }": its number has no conflict. Give --direction and --gap to set a position yourself.
error-numbers-layout-conflict = the proposal for control { $control } (direction { $direction }° with a gap of { $gap } mm) comes from the sheet of course "{ $course }" and is not free for every course on layout { $layout }: it has a conflict on the sheets of { $courses }. Use --sheet for the sheet of course "{ $course }" alone, or give a position for all with --direction and --gap. The project file was left unchanged.
error-numbers-exists = the project file has a correction for control { $control } for { $scope } already. Edit the file to change it; numbers accept only adds new ones.
error-numbers-value = --direction needs a finite number, and --gap must be above 0 and at most { $max } mm
numbers-accepted = Added a correction for the number of control { $control } for { $scope } to { $path }: direction { $direction }° with a gap of { $gap } mm. Run check to see the result.
error-load-changed-project = the project { $path } does not load with the new correction, so it was left as it was

# ---- masks window, masks corridor (M4 Task 4) ----
masks-about = Add masks for window and corridor orienteering to the project file.
masks-window-about = Add a mask for window orienteering: the map shows only in a circle around each control of a course.
masks-window-after-content = The radius is print mm on the sheet, at the print scale of the layout the course prints on, so a window looks the same at every scale. Without --controls every control of the course gets a window. --controls takes control codes separated by commas (31,32,35), or two control numbers joined by a dash (3-6) for the controls numbered 3 to 6 on the sheet; start and finish are not counted. The start and the finish get a window only with --include-start and --include-finish. Windows that overlap merge into one. The new [[masks]] table goes to the end of the file as text, with a comment that says how it was made; with --assign the course's assignment gets the mask, or a new assignment follows the table. Everything before stays as it is, comments included. The program then loads the file again; if that fails, it puts the old content back.
masks-after-exit-codes = Exit codes: 0 when the mask is written, 1 when the project, its map or its course file cannot be loaded or written, or the changed file does not load (the old content is put back), 2 when the command line is wrong: an unknown course or control, a value out of range, a mask id the file has already, a course that has another mask, or a corridor that turns too sharply for its width.
masks-corridor-about = Add a mask for corridor orienteering: the map shows only in a corridor along a polyline you give. No rule says where a corridor goes, so the program does not draw one by itself.
masks-corridor-after-content = The points are map mm with y pointing up, as OOM's status bar shows them, the same as centre_map_mm and the points under [[masks]]. The width is print mm on the sheet: at the print scale of the course's layout with --course, otherwise at the print scale all layouts share. The corridor runs half the width to each side of the polyline. Its ends are cut square, and turns up to 120° keep a pointed corner while sharper ones are cut off. The program refuses a corridor that turns back on itself, has a segment too short for the turn at its end, or comes back across itself; simplify the polyline there or reduce the width. The new [[masks]] table goes to the end of the file as text, as for masks window.
help-masks-window-course = Name of the course, as in the course file. Its controls get the windows, and its layout gives the print scale.
help-masks-controls = Which controls get a window: codes separated by commas (31,32,35) or control numbers as a range (3-6). Default: every control.
help-masks-radius = Radius of every window in print mm, from 1 to 100. Default: 15.
help-masks-radius-for = Radius for one control, CODE=MM, such as 35=20. Can be given more than once.
help-masks-include-start = The start gets a window too.
help-masks-include-finish = The finish gets a window too.
help-masks-window-id = Id of the new mask. Default: window-<course>, with a number added when that is taken.
help-masks-window-assign = The course prints with the new mask: its assignment gets it, or a new assignment is added.
help-masks-points = The polyline as x,y;x,y;… in map mm with y up, decimals with a point, at least 2 points.
help-masks-width = Width of the corridor in print mm, from 1 to 100. Default: 10.
help-masks-corridor-id = Id of the new mask. Default: corridor-<n>, the first number not taken.
help-masks-corridor-assign = The course of --course prints with the new mask. Needs --course.
help-masks-corridor-course = The course whose layout gives the print scale for the width.
error-masks-no-course-file = masks window and masks corridor --course need a course file, and the project names none under [sources] courses
error-masks-map-scale = the map states no scale, so print mm cannot be turned into map mm
error-masks-id = a mask id needs 1 to { $max } characters and no control characters
error-masks-id-taken = the project file has a mask "{ $id }" already. Choose another id with --id, or change the mask in the file.
error-masks-course-has-mask = course "{ $course }" prints with the mask "{ $mask }" already. Change its assignment in the project file to use another one; the program does not replace it.
error-masks-assignment-inline = the assignment of course "{ $course }" is not written as a table [[assignments]] of its own, so the program cannot add the mask to it. Add mask = "{ $id }" to it by hand. The project file was left unchanged.
error-masks-assignments-inline = the assignments are not written as tables [[assignments]] in the file, so the program cannot add one for course "{ $course }". Add an assignment with mask = "{ $id }" by hand. The project file was left unchanged.
error-masks-radius-for = --radius-for takes CODE=MM, such as 35=20, and "{ $text }" is not written that way
error-masks-size = { $value } mm is outside { $min } to { $max } mm; radius and width are print mm on the sheet
error-masks-selection = --controls takes control codes separated by commas (31,32,35) or a range of control numbers (3-6), and "{ $text }" is neither
error-masks-range = course "{ $course }" has the control numbers 1 to { $count }, so the range { $from }-{ $to } does not fit
error-masks-radius-unselected = control { $control } gets no window, so --radius-for cannot set its radius
error-masks-no-controls = course "{ $course }" has no control to put a window around
error-masks-no-position = control { $control } has no position on the map
error-masks-too-many-points = the mask would have more than { $max } points. Use fewer windows or a smaller radius.
error-masks-points = --points takes points as x,y;x,y;… in map mm with y up, decimals with a point, and "{ $text }" is not such a list
error-masks-too-few-points = a corridor needs at least 2 different points
error-masks-too-many-corridor-points = a corridor takes at most { $max } points
error-masks-point-out-of-reach = the point { $x }, { $y } lies beyond ±2 000 000 mm, the reach of an OOM map
error-masks-sharp-turn = the corridor turns too sharply at { $x }, { $y } for its width, so its edges would cross. Simplify the polyline there or reduce the width.
error-masks-self-intersecting = the corridor crosses itself near { $x }, { $y }. Simplify the polyline or reduce the width.
error-masks-invalid = the generated mask does not pass the check of the project file ({ $problem }). This is a bug; please report it.
error-masks-scales = the layouts print at different scales ({ $scales }), and the width is print mm. Give --course, so that the width follows the layout of that course.
error-load-changed-mask = the project { $path } does not load with the new mask, so it was left as it was
masks-window-comment = Windows of { $radius } mm radius on paper at 1:{ $scale } around { $controls } of course "{ $course }", made by masks window.
masks-window-radius-for = Control { $control } has a window of { $radius } mm radius.
masks-window-written = Added the mask "{ $id }" to { $path }: { $count ->
        [one] one window
       *[other] { $count } windows
    } of { $radius } mm radius on paper at 1:{ $scale } around { $controls } of course "{ $course }".
masks-no-start-finish = { $which ->
        [start] The start has no window, so check names it. --include-start gives it one.
        [finish] The finish has no window, so check names it. --include-finish gives it one.
       *[both] The start and the finish have no window, so check names them. --include-start and --include-finish give them one.
    }
masks-corridor-comment = Corridor { $width } mm wide on paper at 1:{ $scale }, made by masks corridor from these points:
masks-corridor-written = Added the mask "{ $id }" to { $path }: a corridor { $width } mm wide on paper at 1:{ $scale } along { $count } points.
masks-assigned = Course "{ $course }" prints with it now, on layout { $layout }.
masks-not-assigned = No course uses it yet. Name it in an assignment with mask = "{ $id }", or run the command again with --assign.
masks-run-check = Run check to see the result.
