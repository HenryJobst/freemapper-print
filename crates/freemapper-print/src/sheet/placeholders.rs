//! Placeholders in sheet texts: `{event}`, `{date}`, `{organiser}`, `{print_scale}`, `{map_scale}`,
//! `{contour_interval}` and `{layout}`. A name that is none of these stays as written and is
//! reported; the placeholders of the courses (`{course}`, M3) are unknown until then.

use toml::value::Date;

use crate::project::{LoadedProject, SheetLang};

/// Longest text between braces that counts as a placeholder name.
const MAX_NAME_CHARS: usize = 40;

/// The grouping space of scale denominators: U+202F, the narrow no-break space.
const THIN_SPACE: char = '\u{202f}';

/// What each placeholder stands for on one sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceholderValues {
    pub event: String,
    pub date: String,
    pub organiser: String,
    pub print_scale: String,
    pub map_scale: String,
    pub contour_interval: String,
    pub layout: String,
}

impl PlaceholderValues {
    pub fn new(project: &LoadedProject, layout_id: &str, print_scale: u32, map_scale: u32) -> Self {
        let p = &project.project;
        let lang = p.event.lang;
        PlaceholderValues {
            event: p.event.title.clone(),
            date: p
                .event
                .date
                .map(|d| format_date(d, lang))
                .unwrap_or_default(),
            organiser: p.event.organiser.clone().unwrap_or_default(),
            print_scale: group_thousands(print_scale),
            map_scale: group_thousands(map_scale),
            contour_interval: p
                .map
                .contour_interval_m
                .map(|m| format_contour_interval(m, lang))
                .unwrap_or_default(),
            layout: layout_id.to_string(),
        }
    }

    fn get(&self, name: &str) -> Option<&str> {
        Some(match name {
            "event" => &self.event,
            "date" => &self.date,
            "organiser" => &self.organiser,
            "print_scale" => &self.print_scale,
            "map_scale" => &self.map_scale,
            "contour_interval" => &self.contour_interval,
            "layout" => &self.layout,
            _ => return None,
        })
    }
}

/// `template` with its placeholders filled, and the names of the unknown ones (each once, in
/// order of appearance). Any `{…}` that is no known name is unknown (`{Event}`, `{print-scale}`)
/// and stays in the text as written. Braces around something with whitespace, nothing, or more
/// than 40 characters are plain text.
pub fn fill(template: &str, values: &PlaceholderValues) -> (String, Vec<String>) {
    let mut out = String::with_capacity(template.len());
    let mut unknown: Vec<String> = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        // A name runs to the next `}`; whitespace, another `{` or more than 40 characters make
        // it plain text.
        let name_len = after.find('}').filter(|&n| {
            n > 0
                && after[..n].chars().count() <= MAX_NAME_CHARS
                && !after[..n].contains(|c: char| c.is_whitespace() || c == '{')
        });
        if let Some(name_len) = name_len {
            let name = &after[..name_len];
            match values.get(name) {
                Some(v) => out.push_str(v),
                None => {
                    out.push('{');
                    out.push_str(name);
                    out.push('}');
                    if !unknown.iter().any(|u| u == name) {
                        unknown.push(name.to_string());
                    }
                }
            }
            rest = &after[name_len + 1..];
        } else {
            out.push('{');
            rest = after;
        }
    }
    out.push_str(rest);
    (out, unknown)
}

/// Whether `template` uses the placeholder `name` (without braces).
pub fn mentions(template: &str, name: &str) -> bool {
    template.contains(&format!("{{{name}}}"))
}

/// `4000` as is, `15000` as `15 000` with a narrow no-break space: four digits stay together.
pub fn group_thousands(n: u32) -> String {
    let digits = n.to_string();
    if digits.len() < 5 {
        return digits;
    }
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(THIN_SPACE);
        }
        out.push(c);
    }
    out
}

/// `14.11.2026` on German sheets, `14 Nov 2026` on English ones.
pub fn format_date(date: Date, lang: SheetLang) -> String {
    match lang {
        SheetLang::De => format!("{:02}.{:02}.{}", date.day, date.month, date.year),
        SheetLang::En => {
            const MONTHS: [&str; 12] = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            let month = MONTHS[usize::from(date.month.clamp(1, 12)) - 1];
            format!("{} {month} {}", date.day, date.year)
        }
    }
}

/// `2,5 m` on German sheets, `2.5 m` on English ones; one decimal at most, whole metres without.
pub fn format_contour_interval(metres: f64, lang: SheetLang) -> String {
    let number = format!("{}", (metres * 10.0).round() / 10.0);
    match lang {
        SheetLang::De => format!("{} m", number.replace('.', ",")),
        SheetLang::En => format!("{number} m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> PlaceholderValues {
        PlaceholderValues {
            event: "Lübeck Sprint".into(),
            date: "14.11.2026".into(),
            organiser: String::new(),
            print_scale: "4000".into(),
            map_scale: "15\u{202f}000".into(),
            contour_interval: "2,5 m".into(),
            layout: "a4".into(),
        }
    }

    #[test]
    fn placeholders_filled() {
        let (text, unknown) = fill(
            "{event} 1:{print_scale} ({map_scale}) {contour_interval}",
            &values(),
        );
        assert_eq!(text, "Lübeck Sprint 1:4000 (15\u{202f}000) 2,5 m");
        assert!(unknown.is_empty());
        assert_eq!(fill("[{organiser}]", &values()).0, "[]");
    }

    #[test]
    fn unknown_placeholder_reported() {
        let (text, unknown) = fill("{course} {event} {course} {x y} {}", &values());
        assert_eq!(text, "{course} Lübeck Sprint {course} {x y} {}");
        assert_eq!(unknown, ["course"]);
        let (text, unknown) = fill("{Event} {print-scale} {event}", &values());
        assert_eq!(text, "{Event} {print-scale} Lübeck Sprint");
        assert_eq!(unknown, ["Event", "print-scale"]);
        let long = format!("{{{}}}", "a".repeat(41));
        assert!(fill(&long, &values()).1.is_empty());
        assert_eq!(fill("open {event", &values()).0, "open {event");
    }

    #[test]
    fn numbers_and_dates() {
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(4000), "4000");
        assert_eq!(group_thousands(15000), "15\u{202f}000");
        assert_eq!(group_thousands(1_000_000), "1\u{202f}000\u{202f}000");
        let d = Date {
            year: 2026,
            month: 11,
            day: 14,
        };
        assert_eq!(format_date(d, SheetLang::De), "14.11.2026");
        assert_eq!(format_date(d, SheetLang::En), "14 Nov 2026");
        assert_eq!(format_contour_interval(2.5, SheetLang::De), "2,5 m");
        assert_eq!(format_contour_interval(2.5, SheetLang::En), "2.5 m");
        assert_eq!(format_contour_interval(5.0, SheetLang::De), "5 m");
        assert_eq!(format_contour_interval(2.505, SheetLang::De), "2,5 m");
        assert_eq!(format_contour_interval(2.505, SheetLang::En), "2.5 m");
        assert_eq!(format_contour_interval(0.5, SheetLang::En), "0.5 m");
    }

    #[test]
    fn grouping_space_has_a_glyph() {
        use crate::render::fonts::{FontFace, char_width};
        let notdef = char_width('\u{ffff}', FontFace::Regular);
        let space = char_width(THIN_SPACE, FontFace::Regular);
        assert!(
            space > 0.0 && (space - notdef).abs() > 1e-9,
            "{space} {notdef}"
        );
    }
}
