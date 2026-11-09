//! Text blocks of the sheet layer: the title block, texts, and the footer with the copyright
//! and credit lines (design §5.2). Sizes come in pt, the page is in mm.

use crate::project::{Anchor, AnchorTarget, Corner, ElementKind, ElementSpec, SheetLang};
use crate::render::fonts::{ASCENT, LINE_SPACING, char_width, face, text_width};
use crate::render::page::{DirectColour, FillRule, PaperBox, Pt, Shape, SheetItem};

use super::placeholders::{PlaceholderValues, fill};
use super::{MissingItem, PlacedElement, SheetContext, SheetNote, layout, layout_elements};

/// 1 pt in mm.
const MM_PER_PT: f64 = 25.4 / 72.0;
/// Title block: size of the first line, bold, when the element gives none (pt).
const TITLE_PT: f64 = 14.0;
/// Title block lines after the first, and the lines of a text (pt).
const BODY_PT: f64 = 9.0;
/// Footer: size of the copyright and credit lines when no element gives one (pt).
const FOOTER_PT: f64 = 5.0;
/// White box behind a title block or text, around the text (mm).
const BOX_PADDING_MM: f64 = 1.0;
/// Footer on a white box inside the map frame: padding (mm).
const FOOTER_BOX_PADDING_MM: f64 = 0.8;
/// The id the default footer goes by in notes. Elements cannot take it (the project loader
/// refuses it), so a note with this id is always about the footer.
pub const FOOTER_ID: &str = "footer";
/// Most copyright lines shown, the line about the rest included.
const MAX_COPYRIGHT_LINES: usize = 6;
/// Footer below the map frame: baseline of the last line above the paper edge (mm).
const FOOTER_BASELINE_MM: f64 = 4.0;
/// The footer goes below the frame only if the passe-partout there is at least this wide (mm):
/// the baseline distance plus the descent of a 5 pt line and some air.
const FOOTER_MIN_MARGIN_MM: f64 = 6.5;

/// One line of a text block.
#[derive(Debug, Clone, PartialEq)]
struct Line {
    text: String,
    size_mm: f64,
    bold: bool,
}

/// Lines of text stacked top down, with an optional white box behind.
#[derive(Debug, Clone, PartialEq)]
struct Block {
    lines: Vec<Line>,
    /// Padding between the text and the edge of the block (mm); the white box has this margin.
    padding_mm: f64,
    white_box: bool,
}

impl Block {
    /// Width and height in mm, padding included.
    fn size(&self) -> (f64, f64) {
        let width = self
            .lines
            .iter()
            .map(|l| text_width(&l.text, face(l.bold, false)) * l.size_mm)
            .fold(0.0, f64::max);
        let height: f64 = self.lines.iter().map(|l| l.size_mm * LINE_SPACING).sum();
        (
            width + 2.0 * self.padding_mm,
            height + 2.0 * self.padding_mm,
        )
    }

    /// The items for the block with its top-left corner at `top_left`: the white box, then
    /// the lines in black; and the box they cover.
    fn place(&self, top_left: Pt) -> PlacedElement {
        let (w, h) = self.size();
        let rect = PaperBox {
            min: top_left,
            max: (top_left.0 + w, top_left.1 + h),
        };
        let mut items = Vec::new();
        if self.white_box {
            items.push(SheetItem::Shape {
                colour: DirectColour::WHITE,
                shape: Shape::Fill {
                    rings: vec![vec![
                        rect.min,
                        (rect.max.0, rect.min.1),
                        rect.max,
                        (rect.min.0, rect.max.1),
                    ]],
                    rule: FillRule::EvenOdd,
                },
            });
        }
        let mut top = top_left.1 + self.padding_mm;
        for l in &self.lines {
            items.push(SheetItem::Shape {
                colour: DirectColour::BLACK,
                shape: Shape::Text {
                    origin: (top_left.0 + self.padding_mm, top + ASCENT * l.size_mm),
                    size: l.size_mm,
                    bold: l.bold,
                    italic: false,
                    text: l.text.clone(),
                    rotation: 0.0,
                    stroke: None,
                },
            });
            top += l.size_mm * LINE_SPACING;
        }
        PlacedElement { items, rect }
    }

    /// The block hung on `anchor` of the page or frame.
    fn place_on(&self, anchor: &Anchor, ctx: &SheetContext<'_>) -> PlacedElement {
        let top_left = layout::anchor_point(anchor, ctx.page, ctx.frame, self.size());
        self.place(top_left)
    }
}

fn plain_lines(lines: &[String], size_pt: f64) -> Vec<Line> {
    lines
        .iter()
        .map(|t| Line {
            text: t.clone(),
            size_mm: size_pt * MM_PER_PT,
            bold: false,
        })
        .collect()
}

/// Draws a `title_block`, `text`, `credit_line` or `copyright_line` element. A placeholder that
/// does not exist adds [`SheetNote::UnknownPlaceholder`]. `None` for a copyright line without
/// any source.
pub(super) fn place_text_element(
    ctx: &SheetContext<'_>,
    spec: &ElementSpec,
    notes: &mut Vec<SheetNote>,
) -> Option<PlacedElement> {
    let block = match spec.kind {
        ElementKind::TitleBlock | ElementKind::Text => {
            let values = values(ctx);
            let lines = spec
                .lines
                .iter()
                .map(|template| {
                    let (text, unknown) = fill(template, &values);
                    for name in unknown {
                        let note = SheetNote::UnknownPlaceholder {
                            id: spec.id.clone(),
                            name,
                        };
                        if !notes.contains(&note) {
                            notes.push(note);
                        }
                    }
                    text
                })
                .collect::<Vec<_>>();
            let lines = if spec.kind == ElementKind::TitleBlock {
                let first = spec.size_pt.unwrap_or(TITLE_PT) * MM_PER_PT;
                lines
                    .into_iter()
                    .enumerate()
                    .map(|(i, text)| Line {
                        text,
                        size_mm: if i == 0 { first } else { BODY_PT * MM_PER_PT },
                        bold: i == 0,
                    })
                    .collect()
            } else {
                plain_lines(&lines, spec.size_pt.unwrap_or(BODY_PT))
            };
            Block {
                lines,
                padding_mm: if spec.box_white() {
                    BOX_PADDING_MM
                } else {
                    0.0
                },
                white_box: spec.box_white(),
            }
        }
        ElementKind::CreditLine => Block {
            // Never smaller than the footer: the project loader refuses less, and a spec built
            // by hand gets the same floor.
            lines: plain_lines(
                &[credit_text(ctx)],
                spec.size_pt.unwrap_or(FOOTER_PT).max(FOOTER_PT),
            ),
            padding_mm: 0.0,
            white_box: false,
        },
        ElementKind::CopyrightLine => Block {
            lines: plain_lines(
                &copyright_lines(ctx, spec.size_pt.unwrap_or(FOOTER_PT) * MM_PER_PT, notes),
                spec.size_pt.unwrap_or(FOOTER_PT),
            ),
            padding_mm: 0.0,
            white_box: false,
        },
        _ => return None,
    };
    if block.lines.is_empty() {
        return None;
    }
    Some(block.place_on(&spec.anchor, ctx))
}

/// The footer parts no element moved, in their default place: the copyright lines above the
/// credit line, one block (design §5.2). Every sheet has the credit line; it cannot be switched
/// off, and a `credit_line` element only moves it. `with_credit` is false when such an element
/// placed it wholly on the page.
/// Placed by [`footer_block`]. The caller checks the box it covers like an element's
/// ([`FOOTER_ID`]).
pub(super) fn default_footer(
    ctx: &SheetContext<'_>,
    with_credit: bool,
    notes: &mut Vec<SheetNote>,
) -> Option<PlacedElement> {
    let has = |kind| layout_elements(ctx.project, ctx.layout).any(|e| e.kind == kind);
    let mut text = Vec::new();
    if !has(ElementKind::CopyrightLine) {
        text.extend(copyright_lines(ctx, FOOTER_PT * MM_PER_PT, notes));
    }
    if with_credit {
        text.push(credit_text(ctx));
    }
    footer_block(&text, ctx.page, ctx.frame, ctx.layout.map_frame.border_mm)
}

/// The footer of `lines` (copyright lines, then the credit line) at 5 pt: below `frame`,
/// left-aligned with it, the last baseline 4 mm above the paper edge; or, when the passe-partout
/// below the frame is under 6.5 mm or too narrow for all the lines, inside the frame at its bottom
/// left on a white box, inset by half the frame line `border_mm` (ADR 0029). `None` for no lines.
/// Sheets and the `render` command share it.
pub(super) fn footer_block(
    lines: &[String],
    page: PaperBox,
    frame: PaperBox,
    border_mm: f64,
) -> Option<PlacedElement> {
    if lines.is_empty() {
        return None;
    }
    let mut block = Block {
        lines: plain_lines(lines, FOOTER_PT),
        padding_mm: 0.0,
        white_box: false,
    };
    let margin = page.max.1 - frame.max.1;
    let (_, h) = block.size();
    // The last baseline: the block's bottom less the descent part of the last line.
    let size = block.lines.last().map_or(0.0, |l| l.size_mm);
    let bottom = page.max.1 - FOOTER_BASELINE_MM + (size * LINE_SPACING - ASCENT * size);
    // Below the frame only if the whole block fits there, not just the last line.
    if margin >= FOOTER_MIN_MARGIN_MM && bottom - h >= frame.max.1 {
        Some(block.place((frame.min.0, bottom - h)))
    } else {
        block.padding_mm = FOOTER_BOX_PADDING_MM;
        block.white_box = true;
        // The frame line is centred on the frame edge: keep the box off its inner half.
        let half_border = border_mm / 2.0;
        let anchor = Anchor {
            to: AnchorTarget::MapFrame,
            corner: Corner::BottomLeft,
            offset_mm: [half_border, half_border],
        };
        let top_left = layout::anchor_point(&anchor, page, frame, block.size());
        Some(block.place(top_left))
    }
}

/// The footer of a page drawn without a project (the `render` command): the copyright lines from
/// the map notes and the credit line in `lang`, against the page's frame, with no frame line.
/// Notes about the lines name the layout `""`.
pub(crate) fn map_footer(
    map_notes: &str,
    lang: SheetLang,
    page: PaperBox,
    frame: PaperBox,
    notes: &mut Vec<SheetNote>,
) -> Option<PlacedElement> {
    let mut lines = copyright_lines_of(map_notes, lang, "", frame, FOOTER_PT * MM_PER_PT, notes);
    lines.push(crate::credit_line(lang).to_string());
    footer_block(&lines, page, frame, 0.0)
}

fn values(ctx: &SheetContext<'_>) -> PlaceholderValues {
    PlaceholderValues::new(
        ctx.project,
        &ctx.layout.id,
        ctx.print.print_scale,
        ctx.map.georef.scale_denominator,
    )
}

fn credit_text(ctx: &SheetContext<'_>) -> String {
    crate::credit_line(ctx.project.project.event.lang).to_string()
}

/// One source per line: `print.copyright` if set, otherwise the notes of the map; split at line
/// breaks, trimmed, empty lines dropped. More than six lines are cut to the first five and a
/// sixth that counts the rest ([`SheetNote::CopyrightTruncated`]); a line wider than the frame
/// is cut with "…".
fn copyright_lines(
    ctx: &SheetContext<'_>,
    size_mm: f64,
    notes: &mut Vec<SheetNote>,
) -> Vec<String> {
    let source = ctx
        .project
        .project
        .print
        .copyright
        .as_deref()
        .unwrap_or(&ctx.map.notes);
    copyright_lines_of(
        source,
        ctx.project.project.event.lang,
        &ctx.layout.id,
        ctx.frame,
        size_mm,
        notes,
    )
}

/// [`copyright_lines`] of the text `source`, for the sheet of `layout` in `lang` with `frame`.
fn copyright_lines_of(
    source: &str,
    lang: SheetLang,
    layout: &str,
    frame: PaperBox,
    size_mm: f64,
    notes: &mut Vec<SheetNote>,
) -> Vec<String> {
    let mut lines: Vec<String> = source
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    let total = lines.len();
    if total > MAX_COPYRIGHT_LINES {
        let shown = MAX_COPYRIGHT_LINES - 1;
        lines.truncate(shown);
        let rest = total - shown;
        lines.push(match lang {
            SheetLang::De => format!("… und {rest} weitere Quellen"),
            SheetLang::En => format!("… and {rest} more sources"),
        });
        notes.push(SheetNote::CopyrightTruncated {
            layout: layout.to_string(),
            total,
            shown,
        });
    }
    let limit = frame.max.0 - frame.min.0 - 2.0 * FOOTER_BOX_PADDING_MM;
    lines
        .into_iter()
        .map(|l| cut_to_width(l, size_mm, limit))
        .collect()
}

/// `text` cut at a character boundary to fit `limit` mm at `size_mm`, ending in "…"; as is when
/// it fits.
fn cut_to_width(text: String, size_mm: f64, limit: f64) -> String {
    let regular = face(false, false);
    let width = |t: &str| text_width(t, regular) * size_mm;
    if width(&text) <= limit {
        return text;
    }
    let ellipsis = width("…");
    let mut used = ellipsis;
    let mut end = 0;
    for (i, c) in text.char_indices() {
        used += char_width(c, regular) * size_mm;
        if used > limit {
            break;
        }
        end = i + c.len_utf8();
    }
    format!("{}…", text[..end].trim_end())
}

/// What the text elements of the layout leave out: a text with `{print_scale}`, one with
/// `{contour_interval}`, the contour interval of the project. Empty when nothing is missing.
pub(super) fn missing_scale_or_contour(ctx: &SheetContext<'_>) -> Vec<MissingItem> {
    use super::placeholders::mentions;
    let lines: Vec<&String> = layout_elements(ctx.project, ctx.layout)
        .filter(|e| matches!(e.kind, ElementKind::TitleBlock | ElementKind::Text))
        .flat_map(|e| &e.lines)
        .collect();
    let shows = |name| lines.iter().any(|l| mentions(l, name));
    let mut missing = Vec::new();
    if !shows("print_scale") {
        missing.push(MissingItem::PrintScaleText);
    }
    if !shows("contour_interval") {
        missing.push(MissingItem::ContourText);
    }
    if ctx.project.project.map.contour_interval_m.is_none() {
        missing.push(MissingItem::ContourValue);
    }
    missing
}
