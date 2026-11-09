# 0026. Logos: PNG and JPEG as images, SVG as paths from an allow-list

Date: 2026-11-08
Status: Accepted

## Context

Clubs put their own logo and their sponsors' logos on the sheet. Most have them as PNG or JPEG, some as SVG
from the designer. The sheet layer draws fills, strokes, text and raster images in CMYK (`SheetItem`). The
core stays pure Rust (ADR 0001). The design listed PDF logos too.

`usvg` copies subtrees while it resolves an SVG: every `use`, every marker, patterns per path, masks and clip
paths in bounding-box units per user, `feImage` targets. The first version of this decision counted `use`
and markers before calling `usvg`. The review of Task 8 broke that count twice. A `use` whose `href` had a
space before the `#`, or that carried both `href` and `xlink:href`, was resolved differently from `usvg` (2.4
GB in 20 s). A 4 KB file of masks nested seven deep took 3 GB, because masks were not counted at all.
Counting every copy `usvg` makes means following its internals release by release. The second review found
three more ways through the count, all by attribute namespaces: `usvg` finds `use` targets by `id` in any
namespace, while the check read only its own; a short `x:d` before the real `d` hid 400 KB of path data
from the check; and a foreign `clipPathUnits`, `clip-path: inherit` or a `style` that overrides the
attribute hid bounding-box clip paths (10 to 23 s, up to 700 MB). The third review found files that stay
small in memory but take long: 199 000 nested groups with a clip path made the check walk every ancestor of
every group (94 s), 100 000 attributes on one element made the XML parser and the check compare each pair
(60 s), and a gradient of 199 000 stops that ten others take through `href` made `usvg` sort the stops ten
times in quadratic time (47 s). The fourth review went through the document type declaration. The byte
scan ended the internal subset at the first `]`, but the XML parser reads processing instructions in the
subset to `?>` and skips `<!ELEMENT`, `<!ATTLIST` and `<!NOTATION` to the first `>`, so `<?x ]>?>` or
`<!ELEMENT x ]>` made the scan read the entities after it as plain text. Their markup came back in through
`&e;`: 199 000 nested clipped groups, 100 000 attributes or nested entity bombs, each over 10 to 20 s. The
entity count found the subset by searching the text for `<!DOCTYPE` and `]>`, which a comment fooled
(1.3 to 1.6 GB). A root with 990 namespace declarations and children that each add one made the XML
parser copy the namespaces for every child (over 20 s for 20 000 children), and a normal Inkscape file was
refused because `version` and `inkscape:version` share a local name. The fifth review found two more paths
that grow with the square of the input. `roxmltree` looks every entity reference up by walking the list of
declarations, so 80 000 empty entities and 180 000 references to the last one passed the size count and ran
past 20 s. `simplecss`, which both `usvg` and the check use to split `style` attributes, works out the line
and column from the start of the value for every declaration: a plain 180 KB style took 7 s, and a 4.8 KB
file that builds its style from entities ran past 20 s. After that fix a review found the same costs
multiplied by `use`: `usvg` parses the attributes of a used element again for every copy, while the work
count charged a copy one unit however long its attributes were. A 16 KB file with a 2 KB style used 50 000
times through nested `use`, or a 1 MB dash array or transform used 1000 times, ran past 20 s.

## Decision

- PNG and JPEG are decoded with `image` (features `png` and `jpeg` only) into a `RasterImage`. An image with
  an alpha channel keeps it.
- Before the XML parser runs, one linear pass over the bytes (`svg_check::pre_scan`) reads the markup the
  way `roxmltree` reads it. It skips comments, CDATA sections and processing instructions where the parser
  skips them, so a declaration inside a comment counts for neither. Markup the parser would refuse, such as
  `<g / >` or an attribute value with `<`, is refused here too. The pass refuses elements nested more than
  256 deep, an element with more than 1000 attributes, more than 50 million attribute pairs in all (the sum
  of the squares of the attribute counts) and more than 64 namespace declarations (`xmlns`, `xmlns:*`) in
  the file. Every later check is then linear or bounded by these numbers, and the check of clip paths
  inside clip paths marks them in one pass instead of walking ancestors.
- The document type declaration must follow a strict grammar, and the pass reads its internal subset by that
  grammar alone. A DTD that does more than declare text entities is refused with the advice to export a
  plain SVG.
  - A DTD may be `<!DOCTYPE name>`, optionally with `PUBLIC "id" "uri"` or `SYSTEM "uri"`, and an
    internal subset of white space, comments and `<!ENTITY name "text">` (or single quotes). The entity
    text holds no `<`, `&` or `%` and is at most 1024 bytes long. The subset ends only at `]`, optional
    white space and `>`.
  - Everything else is refused: processing instructions, `<!ELEMENT`, `<!ATTLIST`, `<!NOTATION`, parameter
    entities and references, entities with `SYSTEM` or `PUBLIC`, anything else in the subset, a subset
    that does not end, a second DOCTYPE and a DOCTYPE after the root element starts.
  - The parser finds the end of the refused declarations differently from a simple scan, or they bring in
    markup, defaults or text the scan cannot see. Illustrator's "Save As" writes the SVG 1.1 public
    identifier and an entity per namespace (`ns_ai` and so on). Inkscape and most other editors write no
    DTD or the identifier alone. Both kinds pass. The external identifier on the DOCTYPE stays allowed although
    the controller's ruling listed it as refused: `roxmltree` reads its two quoted strings and never loads
    the DTD, and refusing it would refuse every Illustrator file.
  - At most 64 entity declarations, so every lookup the parser makes is short.
  - The pass returns the entities with the number of references to each in text and attribute values. Each
    reference brings in exactly its entity's text, so the expanded size is their sum, at most about 50 MB
    (ADR 0019).
- The pass also measures every attribute value with its entities expanded. A `style` attribute may hold at
  most 2048 bytes and 64 declarations (counted as colons, a character reference counted as one), and the
  `style` attributes of a file at most 256 KiB together. With these numbers `simplecss` reads at most 64
  times 2048 bytes per attribute and about 17 million bytes per file. Any other value may hold at most
  1 MiB, except `d` and `points`, which the count of work bounds after parsing. We measured `transform`,
  `class`, `font`, `font-family`, `stroke-dasharray` and a dozen other presentation attributes at 49 MB
  built from entities. Only the dash array passed 1 s (1.1 s), and no logo needs a megabyte in one
  attribute. A `<style>` element never reaches `simplecss`, because the allow-list refuses it.
- An SVG logo must pass an allow-list on its XML before `usvg` sees it (`sheet::logo::svg_check`). Anything
  else leaves the logo out with `SheetNote::LogoSkipped` and the advice to export it as a plain SVG.
  - Elements: `svg`, `g`, `path`, `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `defs`,
    `linearGradient`, `radialGradient`, `stop`, `title`, `desc`, `metadata`, `use`, `symbol` (only through
    `use`) and `clipPath`. A `style` element in any namespace is refused, because `usvg` reads it as a
    style sheet. Elements of other namespaces (Inkscape's `sodipodi:namedview`, RDF in `metadata`) are
    passed over, as `usvg` skips them with their content.
  - `filter`, `mask` and the marker properties must be `none`. `fill` and `stroke` may point to a gradient,
    `clip-path` to a clip path; `url()` in any other property is refused. A gradient may take its stops
    from another gradient, at most 16 links deep, and may have at most 256 stops, its own or those it
    takes.
  - A clip path must be in user space (`clipPathUnits="userSpaceOnUse"`, the default). Its children are
    shapes or `use` of a shape, and nothing inside it, or used by it, has a clip path of its own.
  - Attributes are read as `usvg`'s `parse_svg_element` reads them: by local name, in no namespace or the
    SVG, XLink or XML namespace. The `use` map is built as `usvg` builds it, from the first attribute with
    the local name `id` in any namespace. Anything that two readers could take differently is refused: two
    attributes of one local name on an element if `usvg` or the check reads that name (geometry, links,
    `style`, `class`, gradient and clip attributes, `viewBox`, `preserveAspectRatio` and every presentation
    attribute) or if both are in namespaces `usvg` reads (`href` with `xlink:href` only if both point to
    the same id, as exporters write them), an attribute of any other
    namespace except those of Inkscape, Sodipodi and Adobe Illustrator, which `usvg` passes over, and `id`
    or `style` even in those, because `usvg` looks both up in every namespace.
  - `clip-path` may be set once per element, as attribute or in `style`, and never to `inherit`, which
    copies the parent's clip path. A `use` that refers to itself, directly or through other elements, is
    refused, also a `use` of its own ancestor; `usvg` drops some of these loops and expands the others
    until its own node limit stops it.
  - Every reference resolves as in `usvg`: for `use` and gradient links the unprefixed `href` first, then
    `xlink:href`, parsed by `svgtypes::IRI` (spaces around `#id` allowed); `url()` by `svgtypes::FuncIRI` or
    `svgtypes::Paint`; `style` attributes split by `simplecss`. Both crates are the versions `usvg` uses. An
    id that occurs twice is refused, because `usvg` resolves it differently for `use` and for other links.
  - What `usvg` still copies is counted: one unit per element plus one per two bytes of path data, again for
    every `use` and for every element a clip path clips, the stops of every gradient that takes them
    through `href`, and the largest number of stops once per shape (ADR 0019 has the limits).
  - The bytes of attribute values are counted the same way, once per copy: at most 256 KiB of `style`
    and 4 MiB of all values once every `use` and clip path is resolved. The first number is the file
    limit on `style` from the byte scan, now also after copies. The second started at 32 MiB and was
    lowered by measurement: 31 MiB of dash arrays through 30 copies took 1.8 s, and next to 245 KB of
    style 3.3 s. At 4 MiB the slowest measured file is 0.54 s (two copies of a 1 MiB dash array next to
    120 styles of 2048 bytes); 256 KiB of number-heavy style alone takes 0.52 s.
- `usvg` runs without its default features: no text, no fonts, no svgz, no writer. Embedded images are never
  loaded and external files are never read. Each path becomes a fill and a stroke in the SVG's paint order,
  at its absolute transform, with curves flattened by the map renderer's `flatten_cubic`. Strokes keep their
  width, scaled into mm.
- `Shape::Fill` has a fill rule (`FillRule`). Map areas fill even-odd as in OOM, and so do the other sheet
  elements; an SVG fill takes `nonzero` or `evenodd` from the SVG. The PDF writes `f` or `f*`. Clip outlines
  stay even-odd.
- Colours are converted from RGB to CMYK the naive way: k = 1 − max(r, g, b), c = (1 − r − k) / (1 − k),
  m and y alike, and no colour when k = 1. There is no colour management.
- The sheet cannot draw everything the allow-list lets through. Each of the following gives one
  `SheetNote::LogoSimplified` per logo: a gradient prints in its first stop colour, opacity below 1 prints
  opaque, dashes print solid, and clip paths have no effect.
- Shapes that reach outside the SVG's picture are clipped to the logo's box, as an SVG viewer clips them.
- The logo is `width_mm` wide; the height follows the aspect ratio of the pixels or of the SVG size.
- A logo file that is missing, broken, of another format or over a limit (ADR 0019) is left out with
  `SheetNote::LogoSkipped` naming the file and the reason. The sheet still renders.
- PDF logos are not read.

## Addendum: cost per export (M2 final review, 2026-11-09)

The limits above bound one file. The final review used one 25 MP PNG in 30 logo elements on two layouts: every
element read and decoded the file again, and the PDF embedded it 30 times (14.3 s, 2.86 GB).

- A `LogoCache` lives for one `export` or `check` run and is passed into `render_sheet`. It reads and decodes
  each distinct resolved path once and keeps the image as `Arc<RasterImage>` and the SVG as a parsed tree, so all
  placements share one image and the PDF embeds it once. It remembers failures too. When keeping another logo
  would hold more than two sheets' worth, it drops the decoded logos first; a page keeps alive what it uses.
- A sheet has a budget: 50 megapixels of raster logos and 1 000 000 units of SVG work, each logo counted as often
  as it is placed. SVG work is the size `check_svg` counts plus the points the curves flatten into. The size of a
  PNG or JPEG is read from its header, so a logo past the budget is left out (`LogoSkipped`, budget reason)
  before it is decoded. The probe now takes 0.29 s and 128 MB.
- Time margin: the slowest hostile or legal SVG measured takes about 0.55 to 0.65 s on an idle machine and up to
  1.47 s under a load of 14. The limits stay, with that margin.
- The lists of names `usvg` reads (`READ_IN_ANY_NAMESPACE`, `read_namespace`, `ALLOWED`) are written by hand from
  usvg 0.49. Every `usvg` upgrade re-checks them against its parser.

## Consequences

- `image` and `usvg` add 33 crates to `Cargo.lock`, all pure Rust. The lock file holds `crc32fast`, `flate2`,
  `log` and `svgtypes` at their previous releases until the newest ones are two weeks old (ADR 0006). `png`
  and `flate2` bring `miniz_oxide` 0.8, so the lock has it twice: 0.8.9 for them, 0.9.1 for the PDF writer.
- A logo with masks, patterns, filters, markers, embedded images or text is refused even where `usvg` could
  have drawn it safely. The note tells the club to export a plain SVG; most editors have such an export, and
  text has to be converted to paths anyway.
- The checks follow `usvg`'s reference and attribute rules but none of its copying rules, so a new `usvg`
  release needs a look at how it parses references and attributes, including every place it reads an
  attribute by local name alone.
- Files that carry `href` and `xlink:href` with different targets on one element are refused.
- A logo whose DTD declares elements, attribute defaults, notations, parameter entities, external entities
  or entities with markup is refused, although the XML is valid. No drawing program we know writes such a
  DTD for a plain logo. Names that only an editor reads may repeat, so `version` next to `inkscape:version`
  passes.
- Logos print in the colours a naive conversion gives. Bright RGB blues and greens come out duller than on
  screen; a club that needs exact colours supplies a logo in the colours it wants.
- `Shape::Stroke` has no miter limit, so an SVG's `stroke-miterlimit` (default 4) is not carried over and the
  PDF default of 10 applies: sharp corners of mitred strokes can stick out further than in a browser.
- Cost if wrong: a club with only a PDF logo has to export it as SVG or PNG first.

Source: M2 plan Task 8, controller rulings 2026-11-08, Task 8 reviews and fix rounds 1 to 5 (2026-11-09).
