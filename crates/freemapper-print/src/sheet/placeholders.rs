//! Placeholders in sheet texts: `{event}`, `{date}`, `{organiser}`, `{print_scale}`, `{map_scale}`,
//! `{contour_interval}` and `{layout}`, and on a course sheet also `{course}`, `{length_km}`, `{climb_m}` and
//! `{controls}`. A name that is none of these stays as written and is reported. The course placeholders are
//! always known: on a sheet without a course they are filled with nothing, and the sheet notes that.

use toml::value::Date;

use crate::project::{LoadedProject, SheetLang};

/// Longest text between braces that counts as a placeholder name.
const MAX_NAME_CHARS: usize = 40;

/// The grouping space of scale denominators: U+202F, the narrow no-break space.
const THIN_SPACE: char = '\u{202f}';

/// The placeholders that only a course sheet fills.
pub const COURSE_PLACEHOLDERS: [&str; 4] = ["course", "length_km", "climb_m", "controls"];

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
    /// The course of a course sheet; `None` on a layout sheet, where the course placeholders are empty.
    pub course: Option<CourseValues>,
}

/// What the course placeholders stand for on one course sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct CourseValues {
    /// `{course}`: the course name.
    pub name: String,
    /// `{length_km}`: the length in km with one decimal (`3,4` on German sheets); empty when unknown.
    pub length_km: String,
    /// `{climb_m}`: the climb in whole metres; empty when the course file gives none.
    pub climb_m: String,
    /// `{controls}`: the number of controls, start and finish not counted.
    pub controls: String,
}

impl CourseValues {
    /// The values of a course: `length_m` and `climb_m` in metres when known, `controls` counted without start
    /// and finish.
    pub fn new(
        name: &str,
        length_m: Option<f64>,
        climb_m: Option<f64>,
        controls: usize,
        lang: SheetLang,
    ) -> Self {
        CourseValues {
            name: name.to_string(),
            length_km: length_m
                .filter(|m| m.is_finite())
                .map(|m| format_km(m, lang))
                .unwrap_or_default(),
            climb_m: climb_m
                .filter(|m| m.is_finite())
                .map(|m| format!("{}", m.round().max(0.0) as u64))
                .unwrap_or_default(),
            controls: controls.to_string(),
        }
    }
}

/// `3400` m as `3,4` on German sheets and `3.4` on English ones: km with one decimal.
pub fn format_km(metres: f64, lang: SheetLang) -> String {
    let number = format!("{:.1}", metres / 1000.0);
    match lang {
        SheetLang::De => number.replace('.', ","),
        SheetLang::En => number,
    }
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
            course: None,
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
            "course" => self.course.as_ref().map_or("", |c| &c.name),
            "length_km" => self.course.as_ref().map_or("", |c| &c.length_km),
            "climb_m" => self.course.as_ref().map_or("", |c| &c.climb_m),
            "controls" => self.course.as_ref().map_or("", |c| &c.controls),
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
            course: None,
        }
    }

    #[test]
    fn course_placeholders_only_on_course_sheets() {
        let mut v = values();
        // Known everywhere, empty without a course.
        let (text, unknown) = fill("[{course}|{length_km}|{climb_m}|{controls}]", &v);
        assert_eq!(text, "[|||]");
        assert!(unknown.is_empty());
        v.course = Some(CourseValues::new(
            "Lang",
            Some(3449.0),
            Some(85.4),
            12,
            SheetLang::De,
        ));
        let (text, unknown) = fill(
            "{course}: {length_km} km, {climb_m} m, {controls} Posten",
            &v,
        );
        assert_eq!(text, "Lang: 3,4 km, 85 m, 12 Posten");
        assert!(unknown.is_empty());
        v.course = Some(CourseValues::new(
            "Long",
            Some(3450.1),
            None,
            3,
            SheetLang::En,
        ));
        assert_eq!(fill("{length_km}|{climb_m}", &v).0, "3.5|");
        v.course = Some(CourseValues::new("Score", None, None, 0, SheetLang::En));
        assert_eq!(fill("{length_km}|{controls}", &v).0, "|0");
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
        let (text, unknown) = fill("{curse} {event} {curse} {x y} {}", &values());
        assert_eq!(text, "{curse} Lübeck Sprint {curse} {x y} {}");
        assert_eq!(unknown, ["curse"]);
        let (text, unknown) = fill("{Event} {print-scale} {event}", &values());
        assert_eq!(text, "{Event} {print-scale} Lübeck Sprint");
        assert_eq!(unknown, ["Event", "print-scale"]);
        let long = format!("{{{}}}", "a".repeat(41));
        assert!(fill(&long, &values()).1.is_empty());
        assert_eq!(fill("open {event", &values()).0, "open {event");
    }

    #[test]
    fn forty_characters_still_name_a_placeholder() {
        // 40 characters are a name, 41 are plain text; characters count, not bytes.
        for c in ['a', 'ä'] {
            let name: String = std::iter::repeat_n(c, 40).collect();
            let (text, unknown) = fill(&format!("{{{name}}}"), &values());
            assert_eq!(unknown, std::slice::from_ref(&name));
            assert_eq!(text, format!("{{{name}}}"));
            let longer = format!("{{{name}{c}}}");
            assert!(fill(&longer, &values()).1.is_empty());
        }
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
