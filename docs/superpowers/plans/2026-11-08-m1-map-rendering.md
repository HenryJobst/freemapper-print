# M1 Map Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `freemapper-print render <map.omap>` writes an office print PDF of a finished OOM map at the exact
print scale, with every symbol type OOM maps use (including area patterns and embedded fonts).

**Architecture:** The OOM model and the renderer are copied from freemapper commit `743ce4b` (ADR 0014) into
`crates/freemapper-print`. A new `.omap` reader fills the copied `MapDocument`. The print scale is applied in
two steps: object coordinates are multiplied by the geometry factor, symbol dimensions by the symbol factor
(ADR 0012, via the copied `enlarge_symbols`). The PDF writer gets an exact page size, embedded fonts and
clipped pattern primitives.

**Tech Stack:** Rust 2024, pdf-writer 0.15.0, miniz_oxide 0.9.1, quick-xml 0.42.0, thiserror, clap 4.6.7,
toml (for `freemapper.toml`), ttf-parser and subsetter (new), fonts Arimo (Apache-2.0, metric-compatible with
Arial).

**Spec:** `docs/superpowers/specs/2026-11-08-freemapper-print-design.md` (§2, §5.1, §5.2, §5.4, §6, §7 M1).
Source of the copied code: `/Users/henryprivat/Documents/gitrepos/freemapper` at commit `743ce4b`.

## Global Constraints

- Every dependency version is at least two weeks old; reuse the versions already in `Cargo.lock`; pin a newer
  crate exactly with a comment when its latest release is younger (ADR 0006).
- Toolchain 1.99.1 from `rust-toolchain.toml`; `cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace` pass after every task.
- `unsafe_code = "forbid"`; pure Rust, no C libraries (ADR 0001).
- Source code and comments English; new domain terms get a row in `docs/glossary.md` first (ADR 0003).
- Program messages longer than a short phrase, README, manual, changelog and ADR text go through the `unslop`
  and `humanizer` skills (`CLAUDE.md`).
- Exact scale: one millimetre at print scale is one PDF millimetre; page size equals paper size; no implicit
  fitting (spec §5.2, ADR 0012).
- Symbol factor table (spec §5.2), as data:
  Sprint 1:4000 base, print 1:3000/1:4000/1:5000 → 1.0; Forest 1:15000 base, 1:15000 → 1.0, 1:10000/1:7500/
  1:5000 → 1.5; Schoolyard 1:1000 → 1.0. Print scales outside the table are refused.
- Objects tagged `freemapper:source=sheet` are hidden by default (spec §5.1).
- PDF metadata names freemapper-print and `https://github.com/HenryJobst/freemapper-print` (spec §5.4).
- Copied files keep a first doc line noting `Copied from freemapper 743ce4b (ADR 0014).`

## Review Focus

1. Real OOM maps contain features freemapper never writes (legacy `<coord>` elements, curves, rotated texts,
   hidden symbols, objects whose symbol id is missing): the render must finish, skip what it cannot draw and
   report how many objects it skipped. Pinned in Task 2 and Task 8.
2. A map that is larger than the paper at the chosen scale must be clipped to the page and the command must
   say by how much it overflows, never shrink it to fit. Pinned in Task 7 and Task 8.
3. A print scale outside the table, or a map kind that cannot be inferred, must stop with a message naming
   the allowed scales or the `--kind` option. Pinned in Task 3 and Task 8.
4. Non-ASCII text (ä ö ü ß, „Gänsefüßchen“, Czech ř) must appear as written, not as `?`. Pinned in Task 6.
5. The page must be exactly A4 (595.28 × 841.89 pt) or A3, and a 100 m line on a 1:4000 map printed at 1:5000
   must be 20 mm long in the PDF. Pinned in Task 7.

---

### Task 1: Copy the OOM model and the renderer from freemapper

**Files:**
- Create (copy from freemapper `743ce4b`, paths relative to `crates/freemapper/src/`):
  `crates/freemapper-print/src/omap/{mod.rs,coord.rs,model.rs,symbol_set.rs,enlarge.rs}`,
  `crates/freemapper-print/src/render/{mod.rs,draw.rs,geometry.rs,metrics.rs,pdf.rs,symbols.rs}`
- Create (copy): `assets/symbol-sets/{ISSprOM 2019_4000.omap,school-2019-additions.xml,sheet-additions.xml,README.md}`,
  `docs/render/oom-render-reference.md`
- Modify: `crates/freemapper-print/src/lib.rs`, `crates/freemapper-print/Cargo.toml`, `Cargo.toml`

**Interfaces:**
- Produces (unchanged from freemapper): `omap::{MapCoord, MapDocument, MapObject, MapObjectKind, MapPart,
  SymbolSet, SymbolCode, OmapError, closed_ring}`, `omap::enlarge::enlarge_symbols(&str, f64) ->
  Result<String, OmapError>`, `render::symbols::{RenderSymbols, Symbol, LineDef, PointDef, TextDef, ...}`,
  `render::draw::{primitives, Primitive, Shape}`, `render::geometry::*`, `render::pdf::{PaperBox, write_pdf}`.
- Produces (new): `omap::model::MapGeoref { scale_denominator: u32, declination_deg: f64, grivation_deg: f64,
  ref_projected: Option<(f64, f64)>, crs_spec: Option<String> }`, replacing freemapper's `Georeferencing` in
  `MapDocument::georef`.

- [ ] **Step 1:** Copy the files listed above. Leave out `omap/writer.rs` and everything outside `omap/` and
  `render/`. Replace `crate::georef::Georeferencing` with `MapGeoref` in `model.rs`. Fix the `include_str!`
  paths in `symbol_set.rs` (`../../../../assets/...` still resolves from `crates/freemapper-print/src/omap/`).
- [ ] **Step 2:** Add the first doc line `Copied from freemapper 743ce4b (ADR 0014).` to each copied `.rs` file
  and to `docs/render/oom-render-reference.md`. Add workspace dependencies `pdf-writer = "0.15.0"`,
  `miniz_oxide = "0.9.1"`, `quick-xml = "0.42.0"`, `thiserror = "2.0.21"` (versions as in `Cargo.lock`).
- [ ] **Step 3:** Run `cargo test -p freemapper-print`. Expected: every copied test passes. Tests that use
  freemapper-only code (writer, georef, build) are removed, not stubbed; list the removed tests in the commit
  message.
- [ ] **Step 4:** Run fmt, clippy and test as in Global Constraints. Commit:
  `feat(render): copy OOM model and renderer from freemapper 743ce4b`.

### Task 2: Read finished `.omap` maps

**Files:**
- Create: `crates/freemapper-print/src/omap/read.rs`
- Modify: `crates/freemapper-print/src/omap/{mod.rs,model.rs,symbol_set.rs}`
- Create fixtures: `tests/fixtures/maps/luebeck.omap` (copy of freemapper
  `examples/schleswig-holstein-luebeck/schleswig-holstein-luebeck.omap` at `743ce4b`),
  `tests/fixtures/maps/forest-sample.omap` (OOM repository, tag `v0.9.6`, `examples/forest sample.omap`),
  `tests/fixtures/maps/README.md` (origin, commit/tag and licence of each fixture)
- Test: `crates/freemapper-print/src/omap/read.rs` (unit tests), `crates/freemapper-print/tests/read_maps.rs`

**Interfaces:**
- Consumes: Task 1 model types.
- Produces: `omap::read::read_map(xml: &str) -> Result<MapDocument, OmapError>`,
  `omap::read::read_map_file(path: &Path) -> Result<MapDocument, OmapError>` (`.omap` and `.xmap`, same XML);
  model additions `MapObjectKind::Text { text, h_align, v_align, rotation_rad: f64 }`,
  `MapObject::pattern: Option<PatternOrigin>` with `PatternOrigin { rotation_rad: f64, origin: MapCoord }`;
  `SymbolSet::is_hidden(id: u32) -> bool`; `OmapError::Unsupported(String)`.

- [ ] **Step 1: Write the failing unit tests** in `read.rs`:
  - `reads_coords_text_form`: an object with `<coords count="3">0 0;1000 0 1;2000 500;</coords>` gives three
    `MapCoord`s with flags `0, 1, 0`.
  - `reads_legacy_coord_elements`: `<coords><coord x="0" y="0"/><coord x="1000" y="0" flags="18"/></coords>`
    gives two coords, the second with flags 18.
  - `reads_point_rotation_and_text`: `type="0" rotation="1.5"` → `Point { rotation_rad: 1.5 }`;
    `type="4"` with `<text>Hütte</text>`, `h_align="1" v_align="2" rotation="0.25"` → those values.
  - `reads_tags_and_pattern`: `<tags><t k="freemapper:source">sheet</t></tags>` and
    `<pattern rotation="0.5"><coord x="10" y="20"/></pattern>` are kept.
  - `reads_georeferencing`: `scale="4000"`, `declination="3.2"`, `grivation="1.1"`, projected ref point and
    `<spec language="PROJ.4">…</spec>` land in `MapGeoref`.
  - `rejects_binary_omap`: input starting with the bytes `OMAP` → `OmapError::Unsupported` whose message
    names OOM's legacy binary format and says to re-save the map in OOM.
- [ ] **Step 2:** Run `cargo test -p freemapper-print omap::read`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement `read_map` and `read_map_file`** with `quick_xml` (event reader, as in
  `symbol_set.rs`). Colours and symbols come from `SymbolSet::from_omap_xml`. Objects are read from every
  `<part>` in `<parts>` (also when wrapped in `<barrier>`); templates are ignored. `is_hidden` reads the
  `is_hidden="true"` attribute of `<symbol>`.
- [ ] **Step 4:** Run the unit tests. Expected: PASS.
- [ ] **Step 5: Write the integration tests** in `tests/read_maps.rs`:
  - `luebeck_object_count`: the number of objects read equals the number of `<object ` occurrences in the file;
    the part `peripheral information` exists and its objects carry `freemapper:source=sheet`; georef scale 4000.
  - `forest_sample_reads`: reads without error; object count equals the `<object ` count; scale as in the file.
- [ ] **Step 6:** Run `cargo test -p freemapper-print --test read_maps`. Expected: PASS. Commit:
  `feat(omap): read finished OOM maps`.

### Task 3: Map kinds and the symbol factor table

**Files:**
- Create: `crates/freemapper-print/src/scale.rs`
- Modify: `crates/freemapper-print/src/lib.rs`, `docs/glossary.md` (row "Kartenart" points to freemapper's
  term; add rows only for terms Task 3 introduces that the glossary lacks)
- Test: in `scale.rs`

**Interfaces:**
- Consumes: `MapDocument`, `SymbolSet::name`.
- Produces:
  - `enum MapKind { Sprint, Schoolyard, Forest }` with `serde` names `sprint`, `schoolyard`, `forest`;
  - `fn standard_enlargement(kind: MapKind, scale: u32) -> Option<f64>` (the table in Global Constraints;
    `None` outside it);
  - `fn symbol_factor(kind: MapKind, map_scale: u32, print_scale: u32) -> Result<f64, ScaleError>` =
    `standard_enlargement(print) / standard_enlargement(map)`, so a forest map drawn with the 1:10000 set
    and printed at 1:15000 gets 1/1.5;
  - `fn geometry_factor(map_scale: u32, print_scale: u32) -> f64` = `map_scale / print_scale`;
  - `fn infer_kind(map: &MapDocument, map_path: &Path) -> Option<MapKind>`: `map_kind` from
    `freemapper.toml` next to the map if present, else Sprint for ISSprOM sets at 1:3000–1:5000, Schoolyard for
    1:1000, Forest for ISOM sets at 1:5000–1:15000, else `None`;
  - `enum ScaleError { PrintScaleNotAllowed { kind, print_scale, allowed: Vec<u32> }, MapScaleNotAllowed {..} }`.

- [ ] **Step 1: Write the failing tests:** `sprint_keeps_symbol_size` (4000→5000 = 1.0, 4000→3000 = 1.0),
  `forest_enlarges_at_10000` (15000→10000 = 1.5, 10000→15000 ≈ 0.6667, 15000→7500 = 1.5),
  `schoolyard_only_1000` (1000→1000 = 1.0, 1000→2000 = Err listing `[1000]`),
  `sprint_refuses_10000` (Err listing `[3000, 4000, 5000]`), `geometry_factor_4000_to_5000` (= 0.8),
  `infer_from_sidecar` (temp dir with `freemapper.toml` containing `map_kind = "schoolyard"`),
  `infer_from_symbol_set` (the Lübeck fixture → Sprint; forest sample → Forest), `infer_unknown` (scale
  25000 → `None`).
- [ ] **Step 2:** Run `cargo test -p freemapper-print scale`. Expected: FAIL.
- [ ] **Step 3:** Implement as specified. Add `toml` and `serde` (versions from `Cargo.lock`).
- [ ] **Step 4:** Run the tests. Expected: PASS. Commit: `feat(scale): map kinds and symbol factor table`.

### Task 4: Area patterns

**Files:**
- Modify: `crates/freemapper-print/src/render/{symbols.rs,draw.rs,pdf.rs}`, `docs/render/oom-render-reference.md` §4
- Test: in `symbols.rs`, `draw.rs`, `pdf.rs`

**Interfaces:**
- Consumes: `MapObject::pattern` (Task 2).
- Produces:
  - `Symbol::Area { color: i32, patterns: Vec<Pattern> }`;
  - `enum Pattern { Lines { angle_rad: f64, rotatable: bool, spacing: f64, offset: f64, color: i32, width: f64,
    no_clipping: bool }, Points { angle_rad: f64, rotatable: bool, spacing: f64, offset: f64,
    offset_along: f64, distance: f64, symbol: Box<PointDef>, no_clipping: bool } }` (lengths in 0.001 mm as
    elsewhere in `symbols.rs`);
  - `Shape::Clipped { rings: Vec<Vec<Pt>>, inner: Box<Shape> }`: `inner` painted inside the even-odd clip of
    `rings`.

- [ ] **Step 1: Write the failing tests:**
  - `symbols.rs` `parses_line_and_point_patterns`: a hand-written area symbol with one `<pattern type="1"
    angle="0.785" rotatable="true" line_spacing="1000" line_offset="0" color="3" line_width="100"/>` and one
    `type="2"` with an inner point symbol parses into the two `Pattern` variants with those values.
  - `draw.rs` `line_pattern_fills_square`: a 10 × 10 mm square area with a horizontal line pattern, spacing
    1 mm, gives 10 or 11 `Clipped` strokes of width 0.1 mm whose clip is the square, all at the pattern's
    colour priority.
  - `draw.rs` `pattern_follows_object_rotation`: same square, `rotatable` pattern, object `pattern.rotation_rad
    = π/2` → stroke direction vertical.
  - `draw.rs` `point_pattern_positions`: point pattern with `spacing = distance = 2 mm` on a 10 × 10 mm square
    gives 25 or 36 point placements (exact count by OOM's rule in reference §4), each `Clipped`.
  - `pdf.rs` `clipped_shape_uses_clip_path`: the content stream of a `Clipped` stroke contains `q`, the ring
    path, `W* n`, the stroke and `Q` in that order.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement parsing in `symbols.rs`, generation in `draw.rs` (pattern lines span the area's
  bounding box rotated to the pattern angle; reference §4 gives angle, offset and origin rules; consult OOM
  `src/core/symbols/area_symbol.cpp` `FillPattern::createRenderables` where §4 is silent), painting in `pdf.rs`.
  Extend reference §4 with every rule taken from the OOM source, with its source tag.
- [ ] **Step 4:** Run the tests. Expected: PASS. Commit: `feat(render): area line and point patterns`.

### Task 5: Remaining line and text symbol features

**Files:**
- Modify: `crates/freemapper-print/src/render/{symbols.rs,draw.rs,geometry.rs}`,
  `docs/render/oom-render-reference.md` §2.5, §2.7, §6
- Test: in `draw.rs`, `geometry.rs`

**Interfaces:**
- Produces: `LineDef` gains `pointed_caps: Option<f64>` (cap length), `start_offset: f64`, `end_offset: f64`,
  `dash_symbol: Option<Box<PointDef>>`, `mid_on_dashes: bool`; `TextDef` gains `framing: Option<Framing>`
  with `enum Framing { Line { color: i32, half_width: f64 }, Shadow { color: i32, dx: f64, dy: f64 } }`.

- [ ] **Step 1: Write the failing tests** (values from the OOM sources, recorded in the reference doc):
  `pointed_cap_tapers_to_zero` (a 10 mm line with cap length 2 mm: the first and last fill polygon vertices
  have zero width), `start_end_offsets_shorten_line` (offsets 1 mm each → stroked length 8 mm),
  `dash_symbol_at_dash_points` (coords with `FLAG_DASH` get the dash symbol's primitives),
  `mid_symbols_on_dashed_line` (OOM `createDashGroups` placement), `text_line_framing` (a framed text yields
  the framing stroke below the glyphs, same text, width 2 × half width).
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement from OOM `line_symbol.cpp` (`createPointedLine`, `processContinuousLine`,
  `createDashGroups`, dash symbol handling) and `text_symbol.cpp` framing; add each rule with its source tag to
  the reference doc.
- [ ] **Step 4:** Run. Expected: PASS. Commit: `feat(render): pointed caps, offsets, dash symbols, text framing`.

### Task 6: Embedded fonts

**Files:**
- Create: `assets/fonts/Arimo-{Regular,Bold,Italic,BoldItalic}.ttf`, `assets/fonts/LICENSE` (Apache-2.0,
  from github.com/googlefonts/Arimo), `crates/freemapper-print/src/render/fonts.rs`,
  `docs/adr/0016-arimo-as-the-embedded-font.md`
- Delete: `crates/freemapper-print/src/render/metrics.rs`
- Modify: `crates/freemapper-print/src/render/{draw.rs,pdf.rs,mod.rs}`, `docs/adr/README.md`
- Test: in `fonts.rs`, `pdf.rs`

**Interfaces:**
- Produces: `enum FontFace { Regular, Bold, Italic, BoldItalic }`; `fn face(bold: bool, italic: bool) ->
  FontFace`; `fn text_width(text: &str, face: FontFace) -> f64` (em units); `const ASCENT: f64`,
  `const LINE_SPACING: f64` (from the font's hhea/OS2 tables, replacing the Helvetica constants);
  `Shape::Text` gains `italic: bool`. Every OOM font family maps to Arimo; `fonts::substituted(family: &str) ->
  bool` is true for families other than Arial/Arimo/Liberation Sans so the CLI can report them.

- [ ] **Step 1: Write the failing tests:** `arimo_width_matches_arial` (`text_width("Hütte", Regular)` within
  1 % of Arial's advance widths 0.722+0.556+0.278+0.278+0.556 em), `pdf_embeds_subset_fonts` (a PDF with one
  text has a `/FontFile2` stream and a `/ToUnicode` CMap, and is under 60 kB),
  `non_ascii_text_round_trips` (the ToUnicode CMap of a PDF with „Gänsefüßchen“ and „Řeka“ maps every glyph
  back to those characters; no `?` in the extracted text), `unknown_family_is_substituted` (`"Frutiger"` →
  true, `"Arial"` → false).
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement with `ttf-parser` for metrics and glyph ids and `subsetter` for subsetting; write
  Type0/CIDFontType2 fonts with Identity-H encoding and a ToUnicode CMap through pdf-writer. Pick the newest
  `ttf-parser` and `subsetter` releases that are at least two weeks old (ADR 0006). Write ADR 0016 (Arimo
  replaces Arial and every other OOM font family, Apache-2.0, metric-compatible; cost if wrong: maps that use
  condensed or decorative fonts look different) and add it to the ADR index.
- [ ] **Step 4:** Run. Expected: PASS. Commit: `feat(render): embedded subset fonts (Arimo)`.

### Task 7: Exact-scale page output

**Files:**
- Create: `crates/freemapper-print/src/print.rs`
- Modify: `crates/freemapper-print/src/render/pdf.rs`, `crates/freemapper-print/src/lib.rs`
- Test: in `print.rs`, `crates/freemapper-print/tests/exact_scale.rs`

**Interfaces:**
- Consumes: `read_map_file` (Task 2), `symbol_factor`, `geometry_factor` (Task 3), `enlarge_symbols`,
  `RenderSymbols::from_set`, `primitives` (Tasks 1, 4–6).
- Produces:
  - `enum Paper { A4, A3 }` with `fn size_mm(self) -> (f64, f64)` = (210, 297) / (297, 420);
  - `enum Orientation { Auto, Portrait, Landscape }`;
  - `struct PrintOptions { print_scale: Option<u32>, kind: MapKind, paper: Paper, orientation: Orientation,
    centre: Option<MapCoord>, show_sheet_objects: bool }` (`None` scale = map scale; `None` centre = centre of
    the bounding box of all drawn objects; `Auto` picks the orientation in which more of the map fits);
  - `struct PrintReport { page_mm: (f64, f64), print_scale: u32, geometry_factor: f64, symbol_factor: f64,
    overflow_mm: Option<(f64, f64)>, skipped_objects: usize, substituted_fonts: Vec<String> }`;
  - `struct RenderedPage { primitives: Vec<Primitive>, clip: PaperBox, page_mm: (f64, f64), colours:
    Vec<RenderColour> }`: everything a page backend needs, in page millimetres. It is the only input of the
    PDF writer, so a later SVG or tile backend for the browser editors (this project's M6 and the
    course-setting application) consumes the same value. No PDF-specific data goes into `Primitive`;
  - `fn render_page(map: &MapDocument, options: &PrintOptions) -> Result<(RenderedPage, PrintReport),
    PrintError>`;
  - `fn print_map(map: &MapDocument, options: &PrintOptions, title: &str) -> Result<(Vec<u8>, PrintReport),
    PrintError>` = `render_page` followed by `write_pdf`;
  - `render::pdf::write_pdf(page: &RenderedPage, info: &PdfInfo) -> Vec<u8>` with `PdfInfo { title: String, creator: String }`; the creator is
  `freemapper-print – https://github.com/HenryJobst/freemapper-print`. The freemapper `sheet` argument and
  `page_size` auto-selection are removed.

- [ ] **Step 1: Write the failing tests** in `tests/exact_scale.rs` (build a `MapDocument` in code with the
  copied ISSprOM set):
  - `a4_page_is_exact`: MediaBox is `[0 0 595.2756 841.8898]` (± 0.01 pt); A3 landscape `[0 0 1190.55 841.89]`.
  - `line_length_at_print_scale`: a horizontal 25 mm map line (100 m at 1:4000) printed at 1:5000 strokes
    from x to x + 20 mm, i.e. 56.693 pt (± 0.01).
  - `symbol_width_kept_at_5000`: the same line with symbol 501.1 (width 0.1 mm) has stroke width 0.2835 pt at
    1:5000 (symbol factor 1.0).
  - `forest_symbols_grow_at_10000`: a forest map line of width 0.18 mm at 1:15000 printed at 1:10000 has
    stroke width 0.27 mm.
  - `overflow_is_reported_not_shrunk`: a 300 mm wide map at A4 portrait gives `overflow_mm = Some((100.0, _))`
    (300 − (210 − 2 × 5)) and the line is still drawn at full scale, clipped.
  - `sheet_objects_hidden_by_default`: an object tagged `freemapper:source=sheet` produces no paint unless
    `show_sheet_objects`.
  - `skipped_objects_counted`: an object with an unknown symbol id raises `skipped_objects` to 1.
  - `render_page_is_backend_neutral`: `render_page` on the 25 mm line returns one `Stroke` primitive whose
    path runs 20 mm in page millimetres; no PDF bytes are involved.
- [ ] **Step 2:** Run `cargo test -p freemapper-print --test exact_scale`. Expected: FAIL.
- [ ] **Step 3: Implement `print_map`:** clone the symbol set and apply `enlarge_symbols` with the symbol
  factor (skip it at 1.0); multiply object coordinates by the geometry factor (rounded to integer µm); draw; translate so
  `centre` lands at the centre of the page; clip to the page minus a 5 mm margin.
- [ ] **Step 4:** Run. Expected: PASS. Commit: `feat(print): exact-scale office PDF of a map`.

### Task 8: `render` command

**Files:**
- Modify: `crates/freemapper-print-cli/src/main.rs`, `crates/freemapper-print-cli/Cargo.toml`
- Test: `crates/freemapper-print-cli/tests/render_cli.rs`

**Interfaces:**
- Consumes: `read_map_file`, `infer_kind`, `print_map`, `PrintReport`.
- Produces: `freemapper-print render <MAP> --out <PDF> [--scale <N>] [--paper a4|a3]
  [--orientation auto|portrait|landscape] [--kind sprint|schoolyard|forest] [--show-sheet-objects]`.
  Relative paths are resolved against the working directory. Exit code 0 on success, 1 on errors.

- [ ] **Step 1: Write the failing tests** (run the binary with `env!("CARGO_BIN_EXE_freemapper-print")` on the
  fixtures into a temp dir):
  - `renders_luebeck_a4`: exit 0, the PDF exists, stderr names `1:4000`, `A4`, the symbol factor and contains
    the "actual size" reminder.
  - `refuses_sprint_at_10000`: `--scale 10000` → exit 1, stderr lists `1:3000, 1:4000, 1:5000`.
  - `asks_for_kind`: a fixture copy whose georeferencing scale is 25000 → exit 1, stderr names `--kind`.
  - `reports_overflow`: Lübeck at `--paper a4 --orientation portrait` reports how many millimetres of the map
    do not fit, if any; the test asserts the message appears exactly when `overflow_mm` is `Some`.
  - `renders_forest_sample`: exit 0; stderr reports skipped objects and substituted fonts if there are any.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement with clap derive. Message texts (English, run through `unslop` and `humanizer`):
  the summary line (paper, orientation, print scale, map scale, geometry factor, symbol factor), the overflow
  warning, the skipped-objects and font-substitution notes, and the reminder to print at actual size (100 %,
  no "fit to page").
- [ ] **Step 4:** Run. Expected: PASS. Commit: `feat(cli): render a map to an exact-scale PDF`.

### Task 9: Comparison with OOM and the first real print

**Files:**
- Create: `crates/freemapper-print/tests/oom_comparison.rs`, `tests/fixtures/oom/README.md`
- Create (by the user, see Step 1): `tests/fixtures/oom/luebeck-4000-a4.pdf`,
  `tests/fixtures/oom/forest-sample-a4.pdf`

**Interfaces:**
- Consumes: `print_map`; external tool `pdftoppm` (poppler) only in this ignored test.

- [ ] **Step 1: User step.** Ask the user to export the two fixture maps from OOM 0.9.6 as vector PDF (A4,
  map scale, print area centred on the map, no templates, no grid) into `tests/fixtures/oom/`.
  `tests/fixtures/oom/README.md` records the exact OOM settings. Until the files exist, the test prints that
  they are missing and passes.
- [ ] **Step 2: Write the test** `#[ignore]` `matches_oom_within_tolerance`: rasterise both PDFs with
  `pdftoppm -r 150 -gray`, read the PGM files, crop each to its non-white bounding box, require equal size
  ± 2 px, and require that fewer than 3 % of pixels differ by more than 96 grey levels. On failure write a
  difference image next to the outputs and print its path.
- [ ] **Step 3:** Run `cargo test -p freemapper-print --test oom_comparison -- --ignored`. Expected: PASS, or
  a difference image that shows only accepted deviations (record them in `tests/fixtures/oom/README.md`).
- [ ] **Step 4: User step.** The user prints the Lübeck PDF at actual size on a home printer and measures a
  known distance; record the result in the README of the fixtures. Commit:
  `test(render): compare with OOM's PDF export`.

### Task 10: Documentation

**Files:**
- Modify: `README.md`, `README.en.md`, `CHANGELOG.md`, `CHANGELOG.en.md`, `docs/manual/de.md`,
  `docs/manual/en.md`, `docs/glossary.md`

- [ ] **Step 1:** README (both languages): status line names the `render` command; a short "Benutzung"/"Usage"
  section with one example and the hint to print at actual size. Changelog: one "Hinzugefügt"/"Added" entry.
  Manual: chapter 9 (printing at the right scale) gets its first real content; the other chapters stay
  planned. Glossary: rows for any term introduced in Tasks 2–8 that is still missing (for example
  "Druckrand / page margin").
- [ ] **Step 2:** Run all user text through `unslop` and `humanizer`.
- [ ] **Step 3:** Run fmt, clippy and test. Commit: `docs: render command in README, manual and changelog`.
