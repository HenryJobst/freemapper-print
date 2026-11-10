//! Control descriptions: the IOF description symbols and their texts, and the description box of a course
//! (ADR 0037). Terms follow `docs/glossary.md`.
//!
//! The box follows Purple Pen's `DescriptionFormatter.cs` (which rows) and `DescriptionRenderer.cs` (how they
//! are drawn), GitHub `petergolde/PurplePen`, commit `201417f97bb69302e3e0db123f428870e0914515` (2026-11-07),
//! the commit ADR 0036 takes the symbols from:
//!
//! - Rows: the event title and the course's secondary title, each split at `|` into several rows (an empty
//!   event title still gives one empty row), the course row (name, length, climb; for a score course name
//!   and number of controls), then per stop a control row (A to H) or an instruction row, eight cells wide. A start and a map
//!   exchange control show the start triangle in A; a control shows its number in A (not on score courses)
//!   and its code in B. A finish and a crossing point are instruction rows with their own symbol (14.3 and
//!   13.3 when the file gives none); the finish row shows the length of the leg into it, and a marked leg
//!   into the finish turns its symbol into 14.1. A marked leg elsewhere adds a 13.2 row with its length after
//!   its first control, a leg into a map exchange control a 13.5 row, and a map exchange at a control a
//!   13.5control row with "0 m", or a 15.6 row for a map flip (not in 2004). Score courses show only the start
//!   and the controls, sorted as Purple Pen sorts them ([`compare_codes`]). A crossing point right after
//!   another and a map exchange control at the end are left out.
//! - Distances are the leg through its bends, map mm × map scale ÷ 1000, rounded to 10 m (ties to even, as
//!   .NET's `Math.Round`).
//! - Lines (`DescriptionAppearance` in `Appearance.cs`): thick 5 % and thin 2.5 % of a cell, flat ends. Every
//!   row but a control row has a thick top line; control rows have one after the start and after every third
//!   control row. The outer sides and the line after columns B and E are thick, the other column lines thin;
//!   the last row has a thick bottom line.
//! - Text: Arimo, metric twin of Arial, at the sizes Purple Pen falls back to with Arial (em per cell): title,
//!   course row and instruction distance 0.56 bold, column A 0.63 bold, column B 0.56, column F 0.50, split
//!   column F 0.42. Text that does not fit its cells shrinks to 95 % of the fitting size. Column F text keeps
//!   Purple Pen's forms: `a|b` two lines, `a/b` a diagonal with one value above and one below, `x` printed
//!   as `×`.
//! - The box is black on white: a white background covers it and half a thick line around it.

pub mod symbols;

use crate::course::{ControlKind, Course, CourseKind, FittedEvent};
use crate::omap::MapCoord;
use crate::project::SheetLang;
use crate::render::fonts::{ASCENT, DESCENT, face, text_width};
use crate::render::page::{Cap, DirectColour, FillRule, Join, PaperBox, Pt, Shape, SheetItem};
use crate::sheet::PlacedElement;
use symbols::{Standard, SymbolGeometry, symbol, symbol_text};

/// Line widths in cells (Purple Pen's `thickDescriptionLine` 5 and `thinDescriptionLine` 2.5 per 100).
const THICK: f64 = 0.05;
const THIN: f64 = 0.025;
/// Text sizes in em per cell (Purple Pen's sizes with Arial, whose metrics Arimo shares).
const TITLE_EM: f64 = 0.56;
const COLUMN_A_EM: f64 = 0.63;
const COLUMN_B_EM: f64 = 0.56;
const COLUMN_F_EM: f64 = 0.50;
const COLUMN_F_SMALL_EM: f64 = 0.42;
const INSTRUCTION_EM: f64 = 0.56;
/// Width of a row in cells.
const WIDTH: f64 = 8.0;
/// Segments of a circle of a symbol drawn as a polygon.
const CIRCLE_SEGMENTS: usize = 48;

/// The texts of the two header rows: the event title, and the course with its length and climb.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DescriptionHeader {
    pub event: String,
    pub course: String,
    /// The length in km as printed, without the unit (`4,3`); empty when unknown.
    pub length_km: String,
    /// The climb in metres as printed, without the unit (`25`); empty when unknown.
    pub climb_m: String,
}

/// A cell of a control row (columns A to H).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Cell {
    #[default]
    Empty,
    /// A description symbol, by the id the course file stores.
    Symbol(String),
    Text(String),
}

/// One row of the description box.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// A line of the event title; `|` in the title starts a new one.
    Title(String),
    /// A line of the course's secondary title.
    SecondaryTitle(String),
    /// Course name, length and climb (normal course), or course name and number of controls (score course).
    Header(Vec<String>),
    /// Columns A to H of a start, control or map exchange control.
    Control(Box<[Cell; 8]>),
    /// An instruction row eight cells wide: the symbol (`None` when the id is unknown) and the distance text.
    Instruction {
        symbol: Option<String>,
        text: String,
    },
}

/// A drawn description box with its top-left corner at (0, 0).
#[derive(Debug, Clone, PartialEq)]
pub struct DescriptionBox {
    pub element: PlacedElement,
    pub rows: Vec<Row>,
    /// Symbol ids the course uses that the description standard does not know, each once, in row order.
    /// Their cells stay empty.
    pub unknown_symbols: Vec<String>,
}

/// Whether any stop of `course` has a control with descriptions.
pub fn has_descriptions(course: &Course<MapCoord>, event: &FittedEvent) -> bool {
    course.stops.iter().any(|s| {
        event
            .controls
            .iter()
            .any(|c| c.code == s.control && c.description.is_some())
    })
}

/// The description box of `course`, `cell_mm` per cell, with its top-left corner at (0, 0). `map_scale` turns
/// leg lengths on the map into metres. The size does not depend on the print scale.
pub fn description_box(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    cell_mm: f64,
    lang: SheetLang,
    header: &DescriptionHeader,
    map_scale: u32,
) -> DescriptionBox {
    let standard = course_standard(course, event);
    let mut unknown = Vec::new();
    let rows = rows(
        course,
        event,
        lang,
        header,
        map_scale,
        standard,
        &mut unknown,
    );
    let element = draw(&rows, cell_mm, standard);
    DescriptionBox {
        element,
        rows,
        unknown_symbols: unknown,
    }
}

/// `element` moved by `by` (mm).
pub fn moved(element: PlacedElement, by: Pt) -> PlacedElement {
    let p = |q: Pt| (q.0 + by.0, q.1 + by.1);
    let shape = |s: Shape| match s {
        Shape::Fill { rings, rule } => Shape::Fill {
            rings: rings
                .into_iter()
                .map(|r| r.into_iter().map(p).collect())
                .collect(),
            rule,
        },
        Shape::Stroke {
            path,
            closed,
            width,
            cap,
            join,
        } => Shape::Stroke {
            path: path.into_iter().map(p).collect(),
            closed,
            width,
            cap,
            join,
        },
        Shape::Text {
            origin,
            size,
            bold,
            italic,
            text,
            rotation,
            stroke,
        } => Shape::Text {
            origin: p(origin),
            size,
            bold,
            italic,
            text,
            rotation,
            stroke,
        },
        // The box draws no clipped shapes.
        other @ Shape::Clipped { .. } => other,
    };
    PlacedElement {
        items: element
            .items
            .into_iter()
            .map(|i| match i {
                SheetItem::Shape { colour, shape: s } => SheetItem::Shape {
                    colour,
                    shape: shape(s),
                },
                SheetItem::Image { rect, image } => SheetItem::Image {
                    rect: PaperBox {
                        min: p(rect.min),
                        max: p(rect.max),
                    },
                    image,
                },
            })
            .collect(),
        rect: PaperBox {
            min: p(element.rect.min),
            max: p(element.rect.max),
        },
    }
}

/// The description standard of the course: that of its first control with descriptions. A value no standard
/// matches takes the 2018 symbols, as 2024 does; Purple Pen itself writes only 2004 and 2018.
fn course_standard(course: &Course<MapCoord>, event: &FittedEvent) -> Standard {
    course
        .stops
        .iter()
        .find_map(|s| {
            event
                .controls
                .iter()
                .find(|c| c.code == s.control)?
                .description
                .as_ref()
        })
        .and_then(|d| Standard::from_description(&d.standard))
        .unwrap_or(Standard::Iof2018)
}

/// `id` when the standard knows it, else `None` with the id added to `unknown` once.
fn known(id: &str, standard: Standard, unknown: &mut Vec<String>) -> Option<String> {
    if symbol(id, standard).is_some() {
        Some(id.to_string())
    } else {
        if !unknown.iter().any(|u| u == id) {
            unknown.push(id.to_string());
        }
        None
    }
}

/// `metres` rounded to 10 m (ties to even, as .NET's `Math.Round`), as "400 m".
fn distance_text(metres: f64) -> String {
    format!("{} m", ((metres / 10.0).round_ties_even() * 10.0) as i64)
}

/// The stops of `course` in the order of their rows: file order, and for a score course the order of Purple
/// Pen's `CourseView.CreateScoreCourseView` (commit 201417f): by control kind (start, map exchange, control; the
/// enum order of `ControlPointKind`, map issue points read as starts), then by points (not read, so equal), then
/// by code ([`compare_codes`]), then by control id. Purple Pen gives starts and map exchanges no code, so they
/// compare equal there and go by control id; the position of the control in the file stands in for its id.
fn order(course: &Course<MapCoord>, event: &FittedEvent, score: bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..course.stops.len()).collect();
    if !score {
        return order;
    }
    let control = |i: usize| {
        let code = &course.stops[i].control;
        event.controls.iter().position(|c| &c.code == code)
    };
    let rank = |i: usize| match control(i).map(|k| event.controls[k].kind) {
        Some(ControlKind::Start) => 0,
        Some(ControlKind::MapExchange) => 1,
        Some(ControlKind::Control) => 2,
        _ => 3,
    };
    order.sort_by(|&a, &b| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| {
                if rank(a) == 2 && rank(b) == 2 {
                    compare_codes(&course.stops[a].control, &course.stops[b].control)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| control(a).cmp(&control(b)))
    });
    order
}

/// Purple Pen's `Util.CompareCodes` (commit 201417f): codes that are whole numbers come first in numeric order,
/// then the others in text order. A whole number is what `int.TryParse` with `NumberStyles.None` takes: digits
/// only, no sign or spaces, within `i32`. Purple Pen compares the others by the current culture; this compares
/// them by Unicode code points.
pub fn compare_codes(a: &str, b: &str) -> std::cmp::Ordering {
    let int = |s: &str| {
        if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) {
            s.parse::<i32>().ok()
        } else {
            None
        }
    };
    match (int(a), int(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn rows(
    course: &Course<MapCoord>,
    event: &FittedEvent,
    lang: SheetLang,
    header: &DescriptionHeader,
    map_scale: u32,
    standard: Standard,
    unknown: &mut Vec<String>,
) -> Vec<Row> {
    // Purple Pen splits both titles at `|` and draws an empty event title as an empty row.
    let mut rows: Vec<Row> = header
        .event
        .split('|')
        .map(|t| Row::Title(t.to_string()))
        .collect();
    if let Some(secondary) = &course.secondary_title {
        rows.extend(
            secondary
                .split('|')
                .map(|t| Row::SecondaryTitle(t.to_string())),
        );
    }
    let score = course.kind == CourseKind::Score;
    rows.push(Row::Header(if score {
        let count = crate::overprint::control_count(course, event);
        let text = symbol_text("number_controls", lang).unwrap_or("{0}");
        vec![
            header.course.clone(),
            text.replace("{0}", &count.to_string()),
        ]
    } else {
        let unit = |v: &str, u: &str| {
            if v.is_empty() {
                String::new()
            } else {
                format!("{v} {u}")
            }
        };
        vec![
            header.course.clone(),
            unit(&header.length_km, "km"),
            unit(&header.climb_m, "m"),
        ]
    }));

    let control_of = |i: usize| {
        let code = &course.stops.get(i)?.control;
        event.controls.iter().find(|c| &c.code == code)
    };
    // The control kind of stop `i` as Purple Pen sees it: a map exchange at a normal control stays a control.
    let kind_of = |i: usize| {
        let control = control_of(i)?;
        Some(match course.stops[i].kind {
            None | Some(ControlKind::MapExchange) => control.kind,
            Some(k) => k,
        })
    };
    let leg_m = |i: usize| -> Option<f64> {
        let leg = course.legs.get(i)?;
        let mut points = vec![control_of(i)?.position?];
        points.extend(&leg.bends);
        points.push(control_of(i + 1)?.position?);
        let mm: f64 = points
            .windows(2)
            .map(|w| {
                let dx = f64::from(w[1].x) - f64::from(w[0].x);
                let dy = f64::from(w[1].y) - f64::from(w[0].y);
                dx.hypot(dy) / 1000.0
            })
            .sum();
        Some(mm * f64::from(map_scale) / 1000.0)
    };
    let mut number = 0;
    let n = course.stops.len();
    for i in order(course, event, score) {
        let (Some(control), Some(kind)) = (control_of(i), kind_of(i)) else {
            continue;
        };
        let prev = i.checked_sub(1).and_then(kind_of);
        let next = kind_of(i + 1);
        let shown = if score {
            matches!(kind, ControlKind::Control | ControlKind::Start)
        } else {
            !(kind == ControlKind::CrossingPoint && prev == Some(ControlKind::CrossingPoint))
                && !(kind == ControlKind::MapExchange && i + 1 == n)
        };
        let columns = control.description.as_ref();
        let column = |c: usize| columns.and_then(|d| d.columns[c].clone());
        if shown {
            match kind {
                ControlKind::Finish => {
                    let marked = i > 0 && course.legs.get(i - 1).is_some_and(|l| l.marked_route);
                    let id = if marked {
                        "14.1".to_string()
                    } else {
                        column(0).unwrap_or_else(|| "14.3".into())
                    };
                    rows.push(Row::Instruction {
                        symbol: known(&id, standard, unknown),
                        text: i
                            .checked_sub(1)
                            .and_then(leg_m)
                            .map(distance_text)
                            .unwrap_or_default(),
                    });
                }
                ControlKind::CrossingPoint => {
                    let id = column(0).unwrap_or_else(|| "13.3".into());
                    rows.push(Row::Instruction {
                        symbol: known(&id, standard, unknown),
                        text: String::new(),
                    });
                }
                _ => {
                    let mut cells: [Cell; 8] = Default::default();
                    if matches!(kind, ControlKind::Start | ControlKind::MapExchange) {
                        cells[0] = Cell::Symbol("start".into());
                    }
                    if kind == ControlKind::Control {
                        number += 1;
                        if !score {
                            cells[0] = Cell::Text(number.to_string());
                        }
                        cells[1] = Cell::Text(control.code.clone());
                    }
                    for c in 0..6 {
                        if let Some(id) = column(c)
                            && let Some(id) = known(&id, standard, unknown)
                        {
                            cells[c + 2] = Cell::Symbol(id);
                        }
                    }
                    if let Some(text) = columns.and_then(|d| d.column_f_text.clone()) {
                        cells[5] = Cell::Text(text);
                    }
                    rows.push(Row::Control(Box::new(cells)));
                }
            }
        }
        if score {
            continue;
        }
        // Map exchange rows.
        if next == Some(ControlKind::MapExchange) {
            rows.push(Row::Instruction {
                symbol: known("13.5", standard, unknown),
                text: leg_m(i).map(distance_text).unwrap_or_default(),
            });
        } else if course.stops[i].kind == Some(ControlKind::MapExchange)
            && kind == ControlKind::Control
            && i > 0
        {
            // A map flip has its own symbol from 2018 on (Purple Pen's `exchangeIsFlip`).
            rows.push(
                if course.stops[i].map_flip && standard != Standard::Iof2004 {
                    Row::Instruction {
                        symbol: known("15.6", standard, unknown),
                        text: String::new(),
                    }
                } else {
                    Row::Instruction {
                        symbol: known("13.5control", standard, unknown),
                        text: "0 m".into(),
                    }
                },
            );
        }
        // A marked route that does not end at the finish or a map exchange.
        if course.legs.get(i).is_some_and(|l| l.marked_route)
            && !matches!(
                next,
                Some(ControlKind::Finish | ControlKind::MapExchange) | None
            )
        {
            rows.push(Row::Instruction {
                symbol: known("13.2", standard, unknown),
                text: leg_m(i).map(distance_text).unwrap_or_default(),
            });
        }
    }
    rows
}

/// The drawing of `rows`: white background, lines, symbols and texts.
fn draw(rows: &[Row], cell: f64, standard: Standard) -> PlacedElement {
    let thick = THICK * cell;
    let thin = THIN * cell;
    let (ox, oy) = (thick / 2.0, thick / 2.0);
    let width = WIDTH * cell + thick;
    let height = rows.len() as f64 * cell + thick;
    let rect = PaperBox {
        min: (0.0, 0.0),
        max: (width, height),
    };
    let mut items = vec![SheetItem::Shape {
        colour: DirectColour::WHITE,
        shape: Shape::Fill {
            rings: vec![vec![
                (0.0, 0.0),
                (width, 0.0),
                (width, height),
                (0.0, height),
            ]],
            rule: FillRule::EvenOdd,
        },
    }];
    let mut line = |a: Pt, b: Pt, w: f64| {
        items.push(SheetItem::Shape {
            colour: DirectColour::BLACK,
            shape: Shape::Stroke {
                path: vec![a, b],
                closed: false,
                width: w,
                cap: Cap::Flat,
                join: Join::Miter,
            },
        });
    };
    // Purple Pen's thick line counter: a thick top line whenever it is 0.
    let mut counter = 0;
    for (r, row) in rows.iter().enumerate() {
        let y0 = oy + r as f64 * cell;
        let x = |cells: f64| ox + cells * cell;
        let title_after_title = r > 0
            && matches!(
                (&rows[r - 1], row),
                (Row::Title(_), Row::Title(_)) | (Row::SecondaryTitle(_), Row::SecondaryTitle(_))
            );
        if !title_after_title {
            let w = if !matches!(row, Row::Control(_)) || counter == 0 {
                thick
            } else {
                thin
            };
            line((x(0.0), y0), (x(WIDTH), y0), w);
        }
        if r + 1 == rows.len() {
            line((x(0.0), y0 + cell), (x(WIDTH), y0 + cell), thick);
        }
        let (top, bottom) = (y0 - thick / 2.0, y0 + cell + thick / 2.0);
        let mut vertical = |at: f64, w: f64| line((x(at), top), (x(at), bottom), w);
        vertical(0.0, thick);
        vertical(WIDTH, thick);
        match row {
            Row::Header(boxes) if boxes.len() == 3 => {
                vertical(3.0, thick);
                vertical(6.0, thick);
            }
            Row::Header(_) => vertical(3.0, thick),
            Row::Control(_) => {
                for (at, w) in [
                    (1.0, thin),
                    (2.0, thin),
                    (3.0, thick),
                    (4.0, thin),
                    (5.0, thin),
                    (6.0, thick),
                    (7.0, thin),
                ] {
                    vertical(at, w);
                }
            }
            _ => {}
        }
        counter = match row {
            Row::Control(cells) if matches!(cells[0], Cell::Symbol(_)) => 0,
            Row::Control(_) => (counter + 1) % 3,
            _ => 0,
        };
    }

    for (r, row) in rows.iter().enumerate() {
        let y0 = oy + r as f64 * cell;
        let x = |cells: f64| ox + cells * cell;
        let span = |from: f64, to: f64| (x(from), y0, x(to), y0 + cell);
        match row {
            Row::Title(t) | Row::SecondaryTitle(t) => {
                items.extend(text(t, span(0.0, WIDTH), TITLE_EM * cell, true));
            }
            Row::Header(boxes) => {
                let edges: &[f64] = if boxes.len() == 3 {
                    &[0.0, 3.0, 6.0, 8.0]
                } else {
                    &[0.0, 3.0, 8.0]
                };
                for (i, b) in boxes.iter().enumerate() {
                    items.extend(text(b, span(edges[i], edges[i + 1]), TITLE_EM * cell, true));
                }
            }
            Row::Control(cells) => {
                for (i, c) in cells.iter().enumerate() {
                    let at = span(i as f64, i as f64 + 1.0);
                    match c {
                        Cell::Empty => {}
                        Cell::Symbol(id) => {
                            if let Some(g) = symbol(id, standard) {
                                items.extend(symbol_items(g, (at.0, at.1), cell));
                            }
                        }
                        Cell::Text(t) if i == 0 => {
                            items.extend(text(t, at, COLUMN_A_EM * cell, true));
                        }
                        Cell::Text(t) if i == 5 => {
                            items.extend(column_f(t, at, cell, thin));
                        }
                        Cell::Text(t) => items.extend(text(t, at, COLUMN_B_EM * cell, false)),
                    }
                }
            }
            Row::Instruction {
                symbol: id,
                text: t,
            } => {
                if let Some(g) = id.as_deref().and_then(|id| symbol(id, standard)) {
                    items.extend(symbol_items(g, (x(0.0), y0), cell));
                }
                items.extend(text(t, span(3.0, 5.0), INSTRUCTION_EM * cell, true));
            }
        }
    }
    PlacedElement { items, rect }
}

/// One line of `s` centred in `rect` (left, top, right, bottom) as Purple Pen's `DrawSingleLineText` places it:
/// the middle of ascent and descent on the middle of the rectangle. Text wider than the rectangle shrinks to
/// 95 % of the size that fits. `None` for an empty text.
fn text(s: &str, rect: (f64, f64, f64, f64), em: f64, bold: bool) -> Option<SheetItem> {
    if s.is_empty() {
        return None;
    }
    let f = face(bold, false);
    let available = rect.2 - rect.0;
    let mut size = em;
    let mut w = text_width(s, f) * size;
    if w > available {
        size *= available / w * 0.95;
        w = text_width(s, f) * size;
    }
    let cx = (rect.0 + rect.2) / 2.0;
    let cy = (rect.1 + rect.3) / 2.0;
    Some(SheetItem::Shape {
        colour: DirectColour::BLACK,
        shape: Shape::Text {
            origin: (cx - w / 2.0, cy + (ASCENT - DESCENT) / 2.0 * size),
            size,
            bold,
            italic: false,
            text: s.to_string(),
            rotation: 0.0,
            stroke: None,
        },
    })
}

/// Column F text in the forms of Purple Pen's `RenderColumnFText`: `a|b` as two lines, `a/b` with a diagonal
/// from the top right to the bottom left, `a` above it in the top left and `b` below in the bottom right;
/// otherwise one line. Every `x` prints as `×`.
fn column_f(s: &str, rect: (f64, f64, f64, f64), cell: f64, thin: f64) -> Vec<SheetItem> {
    let s = s.replace('x', "\u{d7}");
    let (l, t, r, b) = rect;
    let (w, h) = (r - l, b - t);
    let small = COLUMN_F_SMALL_EM * cell;
    if let Some((first, second)) = s.split_once('|') {
        let mid = t + h / 2.0;
        return [
            text(first, (l, t, r, mid), small, false),
            text(second, (l, mid, r, b), small, false),
        ]
        .into_iter()
        .flatten()
        .collect();
    }
    if let Some((first, second)) = s.split_once('/') {
        let mid = t + h / 2.0;
        let mut items = vec![SheetItem::Shape {
            colour: DirectColour::BLACK,
            shape: Shape::Stroke {
                path: vec![(r, t), (l, b)],
                closed: false,
                width: thin,
                cap: Cap::Flat,
                join: Join::Miter,
            },
        }];
        items.extend(text(first, (l, t, l + w * 0.63, mid), small, false));
        items.extend(text(second, (l + w * 0.37, mid, r, b), small, false));
        return items;
    }
    text(&s, rect, COLUMN_F_EM * cell, false)
        .into_iter()
        .collect()
}

/// The shapes of symbol `g` with its top-left corner at `at`, `cell` mm per cell.
fn symbol_items(g: &SymbolGeometry, at: Pt, cell: f64) -> Vec<SheetItem> {
    let p = |q: &Pt| (at.0 + q.0 * cell, at.1 + q.1 * cell);
    let black = |shape| SheetItem::Shape {
        colour: DirectColour::BLACK,
        shape,
    };
    let mut items = Vec::new();
    for ring in &g.fills {
        items.push(black(Shape::Fill {
            rings: vec![ring.iter().map(p).collect()],
            rule: FillRule::EvenOdd,
        }));
    }
    for s in &g.strokes {
        items.push(black(Shape::Stroke {
            path: s.points.iter().map(p).collect(),
            closed: s.closed,
            width: s.width * cell,
            cap: if s.round_ends { Cap::Round } else { Cap::Flat },
            join: if s.round_corners {
                Join::Round
            } else {
                Join::Miter
            },
        }));
    }
    for c in &g.circles {
        let centre = p(&c.centre);
        let ring: Vec<Pt> = (0..CIRCLE_SEGMENTS)
            .map(|i| {
                let a = i as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
                (
                    centre.0 + c.radius * cell * a.cos(),
                    centre.1 + c.radius * cell * a.sin(),
                )
            })
            .collect();
        items.push(black(match c.stroke {
            Some(w) => Shape::Stroke {
                path: ring,
                closed: true,
                width: w * cell,
                cap: Cap::Flat,
                join: Join::Round,
            },
            None => Shape::Fill {
                rings: vec![ring],
                rule: FillRule::EvenOdd,
            },
        }));
    }
    items
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::course::{
        Control, ControlKind, CourseKind, CourseSource, Description, Leg, SourceFormat, Stop,
    };
    use crate::render::page::{Shape, SheetItem};

    /// A control at (`x`, `y`) map mm with the description symbols `cols` (columns C to H).
    pub(crate) fn control(
        code: &str,
        kind: ControlKind,
        at: (i32, i32),
        cols: [Option<&str>; 6],
    ) -> Control<MapCoord> {
        let any = cols.iter().any(Option::is_some);
        Control {
            id: None,
            code: code.into(),
            kind,
            position: Some(MapCoord::new(at.0 * 1000, at.1 * 1000)),
            geo: None,
            description: any.then(|| Description {
                columns: cols.map(|c| c.map(str::to_string)),
                standard: "2018".into(),
                column_f_text: None,
            }),
        }
    }

    fn stop(code: &str) -> Stop {
        Stop {
            control: code.into(),
            kind: None,
            number: None,
            number_text_at: None,
            circle_gaps: Vec::new(),
            map_flip: false,
        }
    }

    /// A Purple Pen event with one normal course over `controls` in order, straight legs.
    pub(crate) fn event(controls: Vec<Control<MapCoord>>) -> FittedEvent {
        let stops: Vec<Stop> = controls.iter().map(|c| stop(&c.code)).collect();
        let legs = (1..stops.len())
            .map(|_| Leg {
                bends: Vec::new(),
                gaps: Vec::new(),
                marked_route: false,
            })
            .collect();
        FittedEvent {
            controls,
            courses: vec![Course {
                id: None,
                name: "Lang".into(),
                family: None,
                kind: CourseKind::Normal,
                legs,
                stops,
                climb_m: Some(25.0),
                length_m: None,
                print_scale_hint: None,
                course_obj_ratio: None,
                secondary_title: None,
            }],
            specials: Vec::new(),
            source: CourseSource {
                format: SourceFormat::PurplePen,
                event_name: None,
                map_scale: Some(4000),
                item_scaling: None,
            },
            notes: Vec::new(),
        }
    }

    const D: [Option<&str>; 6] = [None, Some("1.1"), None, None, None, None];

    /// Start, seven controls 100 mm apart along x, finish.
    fn seven() -> FittedEvent {
        let mut cs = vec![control("STA1", ControlKind::Start, (0, 0), D)];
        for i in 1..=7 {
            cs.push(control(
                &format!("{}", 30 + i),
                ControlKind::Control,
                (100 * i, 0),
                [
                    Some("0.1N"),
                    Some("1.1"),
                    Some("8.1"),
                    None,
                    Some("11.1N"),
                    Some("12.1"),
                ],
            ));
        }
        cs.push(control(
            "FIN1",
            ControlKind::Finish,
            (800, 0),
            [Some("14.3"), None, None, None, None, None],
        ));
        event(cs)
    }

    fn header() -> DescriptionHeader {
        DescriptionHeader {
            event: "Lübeck Training".into(),
            course: "Lang".into(),
            length_km: "3,2".into(),
            climb_m: "25".into(),
        }
    }

    fn boxed(ev: &FittedEvent, cell: f64) -> DescriptionBox {
        description_box(&ev.courses[0], ev, cell, SheetLang::De, &header(), 4000)
    }

    /// Horizontal strokes as (y, width) and vertical ones as (x, width, top y).
    type Lines = (Vec<(f64, f64)>, Vec<(f64, f64, f64)>);

    fn lines(b: &DescriptionBox) -> Lines {
        let mut h = Vec::new();
        let mut v = Vec::new();
        for item in &b.element.items {
            if let SheetItem::Shape {
                shape: Shape::Stroke { path, width, .. },
                ..
            } = item
                && path.len() == 2
            {
                let (a, z) = (path[0], path[1]);
                if (a.1 - z.1).abs() < 1e-9 && (z.0 - a.0).abs() > 40.0 {
                    h.push((a.1, *width));
                } else if (a.0 - z.0).abs() < 1e-9 {
                    v.push((a.0, *width, a.1.min(z.1)));
                }
            }
        }
        (h, v)
    }

    fn texts(b: &DescriptionBox) -> Vec<(String, f64, f64, f64, bool)> {
        b.element
            .items
            .iter()
            .filter_map(|i| match i {
                SheetItem::Shape {
                    shape:
                        Shape::Text {
                            text,
                            origin,
                            size,
                            bold,
                            ..
                        },
                    ..
                } => Some((text.clone(), origin.0, origin.1, *size, *bold)),
                _ => None,
            })
            .collect()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn box_rows_and_columns() {
        let ev = seven();
        let b = boxed(&ev, 6.0);
        // Title, header, start, seven controls, finish.
        assert_eq!(b.rows.len(), 11, "{:?}", b.rows);
        assert_eq!(b.rows[0], Row::Title("Lübeck Training".into()));
        assert_eq!(
            b.rows[1],
            Row::Header(vec!["Lang".into(), "3,2 km".into(), "25 m".into()])
        );
        let Row::Control(start) = &b.rows[2] else {
            panic!("{:?}", b.rows[2])
        };
        assert_eq!(start[0], Cell::Symbol("start".into()));
        assert_eq!(start[1], Cell::Empty);
        assert_eq!(start[3], Cell::Symbol("1.1".into()));
        let Row::Control(third) = &b.rows[5] else {
            panic!()
        };
        assert_eq!(third[0], Cell::Text("3".into()));
        assert_eq!(third[1], Cell::Text("33".into()));
        assert_eq!(third[2], Cell::Symbol("0.1N".into()));
        assert_eq!(third[7], Cell::Symbol("12.1".into()));
        // The finish row shows the length of the last leg, 100 mm at 1:4000, rounded to 10 m.
        assert_eq!(
            b.rows[10],
            Row::Instruction {
                symbol: Some("14.3".into()),
                text: "400 m".into()
            }
        );
        assert!(b.unknown_symbols.is_empty(), "{:?}", b.unknown_symbols);

        // Cells of 6 mm; Purple Pen's lines are 5 % (thick) and 2.5 % (thin) of a cell.
        let (thick, thin) = (0.3, 0.15);
        let r = b.element.rect;
        assert_eq!(r.min, (0.0, 0.0));
        assert!(
            close(r.max.0, 48.0 + thick) && close(r.max.1, 66.0 + thick),
            "{r:?}"
        );
        // The white background covers the whole box first.
        let SheetItem::Shape { colour, shape } = &b.element.items[0] else {
            panic!()
        };
        assert_eq!(*colour, crate::render::page::DirectColour::WHITE);
        assert!(matches!(shape, Shape::Fill { .. }));

        let (h, v) = lines(&b);
        let top = |row: usize| thick / 2.0 + 6.0 * row as f64;
        let width_at = |y: f64| -> Vec<f64> {
            h.iter()
                .filter(|(hy, _)| close(*hy, y))
                .map(|(_, w)| *w)
                .collect()
        };
        let one = |ws: Vec<f64>, w: f64| ws.len() == 1 && close(ws[0], w);
        // The title has a thick top line; every row but a normal control row starts with one, and so does the
        // row after the start and after every third control (rows 3, 6 of the controls).
        for row in [0, 1, 2, 3, 6, 9, 10, 11] {
            assert!(one(width_at(top(row)), thick), "row {row}: {h:?}");
        }
        for row in [4, 5, 7, 8] {
            assert!(one(width_at(top(row)), thin), "row {row}: {h:?}");
        }
        assert_eq!(h.len(), 12);
        // Control rows: thin lines between A, B, C | D, E, F | G, H, thick after B and after E.
        let row_top = top(5) - thick / 2.0;
        let mut inner: Vec<(f64, f64)> = v
            .iter()
            .filter(|(x, _, y)| close(*y, row_top) && *x > 1.0 && *x < 47.0)
            .map(|(x, w, _)| ((x - thick / 2.0) / 6.0, *w))
            .collect();
        inner.sort_by(|a, b| a.0.total_cmp(&b.0));
        let expected = [
            (1.0, thin),
            (2.0, thin),
            (3.0, thick),
            (4.0, thin),
            (5.0, thin),
            (6.0, thick),
            (7.0, thin),
        ];
        assert_eq!(inner.len(), expected.len(), "{inner:?}");
        for ((x, w), (ex, ew)) in inner.iter().zip(expected) {
            assert!(close(*x, ex) && close(*w, ew), "{inner:?}");
        }
        // Texts: number bold in column A, code in column B, centred in their cells.
        let t = texts(&b);
        let three = t.iter().find(|x| x.0 == "3").unwrap();
        assert!(three.4 && close(three.3, 0.63 * 6.0), "{three:?}");
        let code = t.iter().find(|x| x.0 == "33").unwrap();
        assert!(!code.4 && close(code.3, 0.56 * 6.0), "{code:?}");
        let code_w =
            crate::render::fonts::text_width("33", crate::render::fonts::FontFace::Regular)
                * code.3;
        assert!(close(code.1 + code_w / 2.0, thick / 2.0 + 9.0), "{code:?}");
        assert!(t.iter().any(|x| x.0 == "Lübeck Training" && x.4));
        assert!(t.iter().any(|x| x.0 == "400 m" && x.4));

        // The size follows the cell: 5 mm cells give a 40 mm wide box.
        let b5 = boxed(&ev, 5.0);
        assert!(close(b5.element.rect.max.0, 40.0 + 0.25));
    }

    #[test]
    fn marked_route_row_has_length() {
        let mut ev = seven();
        let course = &mut ev.courses[0];
        // Control 32 to 33 is a marked route with a bend 30 mm off the line: 2 × √(50² + 30²) mm = 116.62 mm,
        // 466.5 m at 1:4000, printed as 470 m.
        course.legs[2].marked_route = true;
        course.legs[2].bends = vec![MapCoord::new(250_000, 30_000)];
        // The marked route to the finish turns the finish symbol into 14.1.
        course.legs[7].marked_route = true;
        let b = boxed(&ev, 6.0);
        assert_eq!(b.rows.len(), 12);
        assert_eq!(
            b.rows[5],
            Row::Instruction {
                symbol: Some("13.2".into()),
                text: "470 m".into()
            }
        );
        assert_eq!(
            b.rows[11],
            Row::Instruction {
                symbol: Some("14.1".into()),
                text: "400 m".into()
            }
        );
        // The route row draws its symbol across all eight cells and its length in cells D and E.
        let t = texts(&b);
        let len = t.iter().find(|x| x.0 == "470 m").unwrap();
        let w =
            crate::render::fonts::text_width("470 m", crate::render::fonts::FontFace::Bold) * len.3;
        assert!(close(len.1 + w / 2.0, 0.15 + 24.0), "{len:?}");
        // An instruction row has a thick top line, and the control after it too.
        let (h, _) = lines(&b);
        for row in [5, 6] {
            let y = 0.15 + 6.0 * row as f64;
            assert!(
                h.iter().any(|(hy, w)| close(*hy, y) && close(*w, 0.3)),
                "{row}"
            );
        }
    }

    #[test]
    fn unknown_symbol_id_noted() {
        let mut ev = seven();
        ev.controls[2].description.as_mut().unwrap().columns[1] = Some("99.9".into());
        // 2.10 exists in 2018 only; in a 2004 file it is unknown.
        ev.controls[3].description.as_mut().unwrap().columns[1] = Some("2.10".into());
        for c in &mut ev.controls {
            if let Some(d) = &mut c.description {
                d.standard = "2004".into();
            }
        }
        let b = boxed(&ev, 6.0);
        assert_eq!(
            b.unknown_symbols,
            vec!["99.9".to_string(), "2.10".to_string()]
        );
        let Row::Control(cells) = &b.rows[4] else {
            panic!()
        };
        assert_eq!(cells[3], Cell::Empty);
        // Nothing is drawn in cell D of that row.
        let (x0, y0) = (0.15 + 18.0, 0.15 + 24.0);
        let inside =
            |p: &(f64, f64)| p.0 > x0 + 0.5 && p.0 < x0 + 5.5 && p.1 > y0 + 0.5 && p.1 < y0 + 5.5;
        let drawn = b.element.items.iter().any(|i| match i {
            SheetItem::Shape {
                shape: Shape::Stroke { path, .. },
                ..
            } => path.iter().any(inside),
            SheetItem::Shape {
                shape: Shape::Fill { rings, .. },
                ..
            } => rings.iter().flatten().any(inside),
            _ => false,
        });
        assert!(!drawn);
    }

    #[test]
    fn column_f_text_and_score_courses() {
        let mut ev = seven();
        ev.controls[1].description.as_mut().unwrap().column_f_text = Some("5x5|3".into());
        ev.controls[2].description.as_mut().unwrap().column_f_text = Some("2/4".into());
        let b = boxed(&ev, 6.0);
        let Row::Control(cells) = &b.rows[3] else {
            panic!()
        };
        assert_eq!(cells[5], Cell::Text("5x5|3".into()));
        let t = texts(&b);
        // Two half-height lines with the multiplication sign, in the small font.
        assert!(
            t.iter().any(|x| x.0 == "5×5" && close(x.3, 0.42 * 6.0)),
            "{t:?}"
        );
        assert!(t.iter().any(|x| x.0 == "3"));
        assert!(t.iter().any(|x| x.0 == "2") && t.iter().any(|x| x.0 == "4"));

        // A score course: header with the number of controls, column A empty, no finish row.
        ev.courses[0].kind = CourseKind::Score;
        let b = description_box(&ev.courses[0], &ev, 6.0, SheetLang::En, &header(), 4000);
        assert_eq!(
            b.rows[1],
            Row::Header(vec!["Lang".into(), "7 controls".into()])
        );
        assert_eq!(b.rows.len(), 10);
        let Row::Control(cells) = &b.rows[3] else {
            panic!()
        };
        assert_eq!(cells[0], Cell::Empty);
        assert_eq!(cells[1], Cell::Text("31".into()));
        assert!(matches!(b.rows[9], Row::Control(_)));
    }

    /// `sampleevent8.ppen` read and its points taken as map mm (y flipped), without the fit.
    fn sample_event() -> FittedEvent {
        let path = format!(
            "{}/tests/fixtures/courses/ppen/sampleevent8.ppen",
            env!("CARGO_MANIFEST_DIR")
        );
        let raw = crate::course::read_ppen(&std::fs::read(path).unwrap()).unwrap();
        raw.map_points(
            |p| MapCoord::new((p.x_mm * 1000.0) as i32, (-p.y_mm * 1000.0) as i32),
            |o| o,
        )
    }

    /// The row after the control row with code `code`.
    fn after(b: &DescriptionBox, code: &str) -> Row {
        let i = b
            .rows
            .iter()
            .position(|r| matches!(r, Row::Control(c) if c[1] == Cell::Text(code.into())))
            .unwrap();
        b.rows[i + 1].clone()
    }

    #[test]
    fn titles_split_and_secondary_title() {
        let mut ev = seven();
        ev.courses[0].secondary_title = Some("Damen|Lang".into());
        let mut h = header();
        h.event = "Lübeck|Training".into();
        let b = description_box(&ev.courses[0], &ev, 6.0, SheetLang::De, &h, 4000);
        assert_eq!(
            b.rows[..4],
            [
                Row::Title("Lübeck".into()),
                Row::Title("Training".into()),
                Row::SecondaryTitle("Damen".into()),
                Row::SecondaryTitle("Lang".into()),
            ]
        );
        // No line between lines of one title, a thick one between the two titles.
        let (lines_h, _) = lines(&b);
        let at = |row: usize| {
            lines_h
                .iter()
                .any(|(y, _)| close(*y, 0.15 + 6.0 * row as f64))
        };
        assert!(at(0) && !at(1) && at(2) && !at(3) && at(4));
        // An empty event title still takes its row.
        h.event = String::new();
        let b = description_box(&ev.courses[0], &ev, 6.0, SheetLang::De, &h, 4000);
        assert_eq!(b.rows[0], Row::Title(String::new()));
    }

    #[test]
    fn score_rows_sorted_like_purple_pen() {
        use std::cmp::Ordering::*;
        assert_eq!(compare_codes("9", "31"), Less);
        assert_eq!(compare_codes("101", "31"), Greater);
        assert_eq!(compare_codes("101", "A1"), Less);
        assert_eq!(compare_codes("A1", "B"), Less);
        assert_eq!(compare_codes("+5", "4"), Greater);
        assert_eq!(compare_codes("99999999999", "1"), Greater);
        // NumberStyles.None: leading zeros are fine, spaces are not.
        assert_eq!(compare_codes("031", "31"), Equal);
        assert_eq!(compare_codes(" 5", "40"), Greater);
        let mut ev = seven();
        let codes = ["101", "31", "A1", "9", "32", "B2", "40"];
        for (c, code) in ev.controls[1..8].iter_mut().zip(codes) {
            c.code = code.into();
        }
        let course = &mut ev.courses[0];
        course.kind = CourseKind::Score;
        for (s, code) in course.stops[1..8].iter_mut().zip(codes) {
            s.control = code.into();
        }
        // The start last in the file still comes first.
        course.stops.rotate_left(1);
        let b = description_box(&ev.courses[0], &ev, 6.0, SheetLang::De, &header(), 4000);
        let order: Vec<Cell> = b.rows[2..]
            .iter()
            .map(|r| match r {
                Row::Control(c) => c[1].clone(),
                other => panic!("{other:?}"),
            })
            .collect();
        let text = |t: &str| Cell::Text(t.into());
        assert_eq!(
            order,
            [
                Cell::Empty,
                text("9"),
                text("31"),
                text("32"),
                text("40"),
                text("101"),
                text("A1"),
                text("B2")
            ]
        );
    }

    #[test]
    fn score_start_rows_keep_the_file_order_of_their_controls() {
        // Purple Pen gives a start no code: two starts compare equal by code and then go by control id, the order
        // in which the file defines them, not the order of the course.
        let mut ev = seven();
        let mut second = ev.controls[0].clone();
        second.code = "STA2".into();
        second.description.as_mut().unwrap().columns[3] = Some("1.3".into());
        ev.controls.push(second);
        let course = &mut ev.courses[0];
        course.kind = CourseKind::Score;
        let mut stop = course.stops[0].clone();
        stop.control = "STA2".into();
        course.stops.insert(0, stop);
        course.legs.insert(0, course.legs[0].clone());
        let b = description_box(&ev.courses[0], &ev, 6.0, SheetLang::De, &header(), 4000);
        let starts: Vec<Option<Cell>> = b.rows[2..4]
            .iter()
            .map(|r| match r {
                Row::Control(c) => Some(c[5].clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            starts,
            [Some(Cell::Empty), Some(Cell::Symbol("1.3".into()))],
            "{:?}",
            &b.rows[2..4]
        );
    }

    #[test]
    fn map_flip_has_its_own_row() {
        // sampleevent8: 41 is a map exchange at the control, 75 a map exchange that flips the map (2018).
        let ev = sample_event();
        let b = description_box(&ev.courses[0], &ev, 6.0, SheetLang::De, &header(), 10000);
        assert!(b.unknown_symbols.is_empty(), "{:?}", b.unknown_symbols);
        assert_eq!(
            after(&b, "75"),
            Row::Instruction {
                symbol: Some("15.6".into()),
                text: String::new()
            }
        );
        assert_eq!(
            after(&b, "41"),
            Row::Instruction {
                symbol: Some("13.5control".into()),
                text: "0 m".into()
            }
        );
        // 2004 has no 15.6: the flip is an ordinary exchange there.
        let mut ev = ev;
        for c in &mut ev.controls {
            if let Some(d) = &mut c.description {
                d.standard = "2004".into();
            }
        }
        let b = description_box(&ev.courses[0], &ev, 6.0, SheetLang::De, &header(), 10000);
        assert_eq!(
            after(&b, "75"),
            Row::Instruction {
                symbol: Some("13.5control".into()),
                text: "0 m".into()
            }
        );
    }

    #[test]
    fn map_exchanges_and_crossing_points() {
        let mut ev = seven();
        // 33 is a map exchange at the control; a crossing point between 35 and 36.
        ev.courses[0].stops[3].kind = Some(ControlKind::MapExchange);
        ev.controls.push(control(
            "CROSS1",
            ControlKind::CrossingPoint,
            (550, 0),
            [Some("13.3"), None, None, None, None, None],
        ));
        let course = &mut ev.courses[0];
        course.stops.insert(6, stop("CROSS1"));
        course.legs.insert(
            6,
            Leg {
                bends: Vec::new(),
                gaps: Vec::new(),
                marked_route: false,
            },
        );
        let b = boxed(&ev, 6.0);
        assert_eq!(
            b.rows[6],
            Row::Instruction {
                symbol: Some("13.5control".into()),
                text: "0 m".into()
            }
        );
        assert_eq!(
            b.rows[9],
            Row::Instruction {
                symbol: Some("13.3".into()),
                text: String::new()
            }
        );
        // The crossing point does not count: the control after it is number 6.
        let Row::Control(cells) = &b.rows[10] else {
            panic!("{:?}", b.rows)
        };
        assert_eq!(cells[0], Cell::Text("6".into()));
    }
}
