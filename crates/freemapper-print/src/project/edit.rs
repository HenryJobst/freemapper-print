//! Changes to one table of the project file: the keys of a layout ([`set_layout`]), a sheet element
//! ([`set_element`]) and the assignment of a course ([`set_assignment`]). Like [`super::append_mask`] they edit the
//! text, so whatever they do not change stays byte for byte as it was, comments included. A key whose value stays
//! the same keeps its line, a changed key gets its new line in the place of the old one, a new key goes after the
//! last key of its table, a key that goes away loses its line, and a new table follows the last table of its kind.
//! A changed key written as a sub-table (`[layouts.map_frame]`) becomes one line, and the comment lines above and
//! inside the sub-table move above that line. Strings with a line break are written on one line with `\n`, so the
//! file keeps its line breaks (CRLF or LF).
//!
//! Each change is checked twice before the file is replaced: the changed project must pass the checks of
//! [`super::load`], and the changed text must read back as that project. Only then is the file replaced, through
//! [`super::rewrite`], which loads it once more and puts the old content back when it does not read back.

use std::ops::Range;
use std::path::Path;

use serde::Serialize;

use super::{
    Assignment, EditTarget, ElementSpec, LayoutSpec, LoadedProject, MAX_PROJECT_FILE_BYTES,
    Project, ProjectError, load, load_text, rewrite, validate, with_newlines,
};

/// What a change did to the project file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// A new table was added.
    Added,
    /// A table the file had was changed.
    Changed,
    /// The file has it this way already; nothing was written.
    Unchanged,
}

/// Adds the layout `layout` to the project file at `path`, or changes the layout of the same id. The keys of the
/// layout come from `layout`; an existing layout keeps its elements as written (`layout.elements` is used for a
/// new layout only). Returns the project as it reads back and what changed.
pub fn set_layout(
    path: &Path,
    layout: &LayoutSpec,
) -> Result<(LoadedProject, Change), ProjectError> {
    let file = path.display().to_string();
    let (old, before) = read(path)?;
    let at = before
        .project
        .layouts
        .iter()
        .position(|l| l.id == layout.id);
    let mut new = layout.clone();
    if let Some(i) = at {
        new.elements = before.project.layouts[i].elements.clone();
        if before.project.layouts[i] == new {
            return Ok((before, Change::Unchanged));
        }
    }
    let mut expected = before.project.clone();
    match at {
        Some(i) => expected.layouts[i] = new.clone(),
        None => expected.layouts.push(new.clone()),
    }
    check(&file, &before, &expected)?;
    let not_editable = || ProjectError::NotEditable {
        file: file.clone(),
        target: EditTarget::Layout(layout.id.clone()),
    };
    let doc = Doc::new(text_of(&old).ok_or_else(not_editable)?);
    let layouts = doc.arrays(&["layouts"], 0..doc.lines.len());
    if layouts.len() != before.project.layouts.len() {
        return Err(not_editable());
    }
    let mut edits = Vec::new();
    let change = match at {
        Some(i) => {
            let h = layouts[i];
            doc.edit_keys(
                h,
                doc.group_end(h),
                &layout_keys(&before.project.layouts[i])?,
                &layout_keys(&new)?,
                &mut edits,
            );
            Change::Changed
        }
        None => {
            let last = *layouts.last().ok_or_else(not_editable)?;
            let at = doc.byte(doc.trimmed(last, doc.group_end(last)));
            edits.push((at..at, format!("\n{}", layout_table(&new)?)));
            Change::Added
        }
    };
    finish(path, &file, &old, edits, &expected, not_editable).map(|lp| (lp, change))
}

/// Adds the sheet element `element` to the layout `layout` (`None`: to the shared elements), or replaces the
/// element of the same id there. Returns the project as it reads back and what changed.
pub fn set_element(
    path: &Path,
    layout: Option<&str>,
    element: &ElementSpec,
) -> Result<(LoadedProject, Change), ProjectError> {
    let file = path.display().to_string();
    let (old, before) = read(path)?;
    let at_layout = match layout {
        None => None,
        Some(id) => Some(
            before
                .project
                .layouts
                .iter()
                .position(|l| l.id == id)
                .ok_or_else(|| ProjectError::Invalid {
                    file: file.clone(),
                    field: "layouts".into(),
                    message: format!("there is no layout with the id `{id}`"),
                })?,
        ),
    };
    let list = |p: &Project| match at_layout {
        None => p.shared.clone(),
        Some(i) => p.layouts[i].elements.clone(),
    };
    let old_list = list(&before.project);
    let at = old_list.iter().position(|e| e.id == element.id);
    if at.is_some_and(|j| old_list[j] == *element) {
        return Ok((before, Change::Unchanged));
    }
    let mut expected = before.project.clone();
    let target = match at_layout {
        None => &mut expected.shared,
        Some(i) => &mut expected.layouts[i].elements,
    };
    match at {
        Some(j) => target[j] = element.clone(),
        None => target.push(element.clone()),
    }
    check(&file, &before, &expected)?;
    let table = if at_layout.is_some() {
        "layouts.elements"
    } else {
        "shared"
    };
    let not_editable = || ProjectError::NotEditable {
        file: file.clone(),
        target: match layout {
            Some(l) => EditTarget::LayoutElements(l.to_string()),
            None => EditTarget::SharedElements,
        },
    };
    let doc = Doc::new(text_of(&old).ok_or_else(not_editable)?);
    let all = 0..doc.lines.len();
    // The element tables, and where a new one goes when there is none yet.
    let (elements, after_last) = match at_layout {
        None => {
            let shared = doc.arrays(&["shared"], all.clone());
            // Before the first layout, after whatever comes before it.
            let first_layout = doc.arrays(&["layouts"], all.clone()).first().copied();
            let fallback = match first_layout {
                Some(h) => {
                    let before_it = doc.headers().into_iter().rfind(|&g| g < h).unwrap_or(0);
                    doc.trimmed(before_it, h)
                }
                None => doc.lines.len(),
            };
            (shared, fallback)
        }
        Some(i) => {
            let layouts = doc.arrays(&["layouts"], all.clone());
            if layouts.len() != before.project.layouts.len() {
                return Err(not_editable());
            }
            let h = layouts[i];
            let end = doc.group_end(h);
            (
                doc.arrays(&["layouts", "elements"], h + 1..end),
                doc.trimmed(h, end),
            )
        }
    };
    if elements.len() != old_list.len() {
        return Err(not_editable());
    }
    let mut edits = Vec::new();
    let change = match at {
        Some(j) => {
            let h = elements[j];
            doc.edit_keys(
                h,
                doc.group_end(h),
                &element_keys(&old_list[j])?,
                &element_keys(element)?,
                &mut edits,
            );
            Change::Changed
        }
        None => {
            let line = match elements.last() {
                Some(&last) => doc.trimmed(last, doc.group_end(last)),
                None => after_last,
            };
            let at = doc.byte(line);
            edits.push((
                at..at,
                format!("\n{}", table_text(table, &element_keys(element)?)),
            ));
            Change::Added
        }
    };
    finish(path, &file, &old, edits, &expected, not_editable).map(|lp| (lp, change))
}

/// Sets the assignment of `assignment.course` to `assignment`: its layout and mask. A course without an assignment
/// gets a new `[[assignments]]` table. Returns the project as it reads back and what changed.
pub fn set_assignment(
    path: &Path,
    assignment: &Assignment,
) -> Result<(LoadedProject, Change), ProjectError> {
    let file = path.display().to_string();
    let (old, before) = read(path)?;
    let at = before
        .project
        .assignments
        .iter()
        .position(|a| a.course == assignment.course);
    if at.is_some_and(|i| before.project.assignments[i] == *assignment) {
        return Ok((before, Change::Unchanged));
    }
    let mut expected = before.project.clone();
    match at {
        Some(i) => expected.assignments[i] = assignment.clone(),
        None => expected.assignments.push(assignment.clone()),
    }
    check(&file, &before, &expected)?;
    let not_editable = || ProjectError::NotEditable {
        file: file.clone(),
        target: EditTarget::Assignment(assignment.course.clone()),
    };
    let doc = Doc::new(text_of(&old).ok_or_else(not_editable)?);
    let assignments = doc.arrays(&["assignments"], 0..doc.lines.len());
    if assignments.len() != before.project.assignments.len() {
        return Err(not_editable());
    }
    let mut edits = Vec::new();
    let change = match at {
        Some(i) => {
            let h = assignments[i];
            doc.edit_keys(
                h,
                doc.group_end(h),
                &assignment_keys(&before.project.assignments[i])?,
                &assignment_keys(assignment)?,
                &mut edits,
            );
            Change::Changed
        }
        None => {
            let at = match assignments.last() {
                Some(&last) => doc.byte(doc.trimmed(last, doc.group_end(last))),
                None => doc.text.len(),
            };
            edits.push((
                at..at,
                format!(
                    "\n{}",
                    table_text("assignments", &assignment_keys(assignment)?)
                ),
            ));
            Change::Added
        }
    };
    finish(path, &file, &old, edits, &expected, not_editable).map(|lp| (lp, change))
}

/// The bytes of the project file and the project they hold.
fn read(path: &Path) -> Result<(Vec<u8>, LoadedProject), ProjectError> {
    let old = std::fs::read(path).map_err(|source| ProjectError::Io {
        file: path.display().to_string(),
        source,
    })?;
    let before = load(path)?;
    Ok((old, before))
}

fn text_of(old: &[u8]) -> Option<&str> {
    std::str::from_utf8(old).ok()
}

/// The checks of [`super::load`] on the changed project: nothing is written when it would not load.
fn check(file: &str, before: &LoadedProject, expected: &Project) -> Result<(), ProjectError> {
    let changed = LoadedProject {
        project: expected.clone(),
        ..before.clone()
    };
    validate(file, &changed)
}

/// Applies `edits` to `old`, checks that the text reads back as `expected`, and replaces the file with it.
fn finish(
    path: &Path,
    file: &str,
    old: &[u8],
    edits: Vec<Edit>,
    expected: &Project,
    not_editable: impl Fn() -> ProjectError,
) -> Result<LoadedProject, ProjectError> {
    let text = text_of(old).ok_or_else(&not_editable)?;
    let new = apply(text, edits, old);
    if new.len() as u64 > MAX_PROJECT_FILE_BYTES {
        return Err(ProjectError::TooLarge {
            file: file.to_string(),
            bytes: new.len() as u64,
        });
    }
    // The text must read back as the project that passed the checks; otherwise the file is written in a way the
    // edits do not handle (for example a key that is not on a line of its own), and nothing is written.
    match load_text(path, file, &new) {
        Ok(lp) if lp.project == *expected => {}
        _ => return Err(not_editable()),
    }
    rewrite(path, old, new.as_bytes(), |lp| lp.project == *expected)
}

/// A replacement of a byte range of the text.
type Edit = (Range<usize>, String);

/// `text` with `edits` applied (none overlap); the new lines end as the lines of `old` do.
fn apply(text: &str, mut edits: Vec<Edit>, old: &[u8]) -> String {
    // From the back, so that the ranges stay valid; at one place a replacement before an insertion.
    edits.sort_by_key(|a| std::cmp::Reverse((a.0.start, a.0.end)));
    let mut out = text.to_string();
    let mut limit = usize::MAX;
    for (range, with) in edits {
        debug_assert!(range.end <= limit, "edits overlap");
        limit = range.start;
        let mut with = with_newlines(old, &with);
        // A text that ends without a line break gets one before anything added at its end.
        if !with.is_empty() && range.start > 0 && !out[..range.start].ends_with('\n') {
            with.insert_str(0, super::newline_of(old));
        }
        out.replace_range(range, &with);
    }
    out
}

/// The keys of a table in the order they are written, each with its value as TOML text; `None` for a key that is
/// left out.
type Keys = Vec<(&'static str, Option<String>)>;

/// `v` as an inline TOML value. A string with a line break is written on one line, with `\n` escaped.
fn value(v: &impl Serialize) -> Result<String, ProjectError> {
    let mut out = String::new();
    v.serialize(toml::ser::ValueSerializer::new(&mut out))
        .map_err(|e| ProjectError::Serialize(e.to_string()))?;
    Ok(one_line_strings(&out))
}

/// `text`, a TOML value, with each multi-line string rewritten as a basic string on one line. The line breaks
/// left in the value are then outside strings, so they can take the file's (CRLF or LF) without changing a string:
/// a CRLF file would otherwise turn `"a\nb"` into `"a\r\nb"`.
fn one_line_strings(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let (mut i, mut copied) = (0, 0);
    while i < b.len() {
        let q = b[i];
        if q != b'"' && q != b'\'' {
            i += 1;
            continue;
        }
        let multi = b[i..].starts_with(&[q, q, q]);
        let end = string_end(b, i, multi);
        if multi && let Some(decoded) = decoded_string(&text[i..end]) {
            out.push_str(&text[copied..i]);
            out.push_str(&basic_string(&decoded));
            copied = end;
        }
        i = end;
    }
    out.push_str(&text[copied..]);
    out
}

/// The byte after the string that starts at `start` (its opening quote) in `b`.
fn string_end(b: &[u8], start: usize, multi: bool) -> usize {
    let q = b[start];
    let mut i = start + if multi { 3 } else { 1 };
    while i < b.len() {
        if q == b'"' && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if multi && b[i..].starts_with(&[q, q, q]) {
            // Up to two more quotes belong to the string (`"""a""""` ends with a quote).
            let mut end = i + 3;
            while end < b.len() && end < i + 5 && b[end] == q {
                end += 1;
            }
            return end;
        }
        if !multi && b[i] == q {
            return i + 1;
        }
        i += 1;
    }
    b.len()
}

/// The string a TOML string literal holds; `None` when it does not read.
fn decoded_string(literal: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Holder {
        v: String,
    }
    toml::from_str::<Holder>(&format!("v = {literal}"))
        .ok()
        .map(|h| h.v)
}

/// `s` as a TOML basic string on one line.
fn basic_string(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A list of strings, left out when empty.
fn list(items: &[String]) -> Result<Option<String>, ProjectError> {
    (!items.is_empty()).then(|| value(&items)).transpose()
}

fn layout_keys(l: &LayoutSpec) -> Result<Keys, ProjectError> {
    // Every field named, so that a new one cannot be forgotten here.
    let LayoutSpec {
        id,
        paper,
        orientation,
        print_scale,
        map_frame,
        north_lines,
        hide_parts,
        elements: _,
        hide_shared,
    } = l;
    Ok(vec![
        ("id", Some(value(id)?)),
        ("paper", Some(value(paper)?)),
        ("orientation", Some(value(orientation)?)),
        ("print_scale", print_scale.map(|s| value(&s)).transpose()?),
        ("map_frame", Some(value(map_frame)?)),
        ("north_lines", Some(value(north_lines)?)),
        ("hide_parts", list(hide_parts)?),
        ("hide_shared", list(hide_shared)?),
    ])
}

fn element_keys(e: &ElementSpec) -> Result<Keys, ProjectError> {
    let ElementSpec {
        id,
        kind,
        anchor,
        lines,
        size_pt,
        box_white,
        file,
        width_mm,
        cell_mm,
    } = e;
    // Two lines or more, one per line, as the starter writes them.
    let lines = match lines.len() {
        0 => None,
        1 => Some(value(lines)?),
        _ => {
            let mut text = String::from("[\n");
            for line in lines {
                text.push_str(&format!("    {},\n", value(line)?));
            }
            text.push(']');
            Some(text)
        }
    };
    Ok(vec![
        ("id", Some(value(id)?)),
        ("kind", Some(value(kind)?)),
        ("lines", lines),
        ("size_pt", size_pt.map(|v| value(&v)).transpose()?),
        ("box_white", box_white.map(|v| value(&v)).transpose()?),
        ("file", file.as_ref().map(value).transpose()?),
        ("width_mm", width_mm.map(|v| value(&v)).transpose()?),
        ("cell_mm", cell_mm.map(|v| value(&v)).transpose()?),
        ("anchor", Some(value(anchor)?)),
    ])
}

fn assignment_keys(a: &Assignment) -> Result<Keys, ProjectError> {
    let Assignment {
        course,
        layout,
        mask,
    } = a;
    Ok(vec![
        ("course", Some(value(course)?)),
        ("layout", Some(value(layout)?)),
        ("mask", mask.as_ref().map(value).transpose()?),
    ])
}

/// `[[name]]` with the keys that are set.
fn table_text(name: &str, keys: &Keys) -> String {
    let mut out = format!("[[{name}]]\n");
    for (key, v) in keys {
        if let Some(v) = v {
            out.push_str(&format!("{key} = {v}\n"));
        }
    }
    out
}

/// A new layout with its elements.
fn layout_table(layout: &LayoutSpec) -> Result<String, ProjectError> {
    let mut out = table_text("layouts", &layout_keys(layout)?);
    for e in &layout.elements {
        out.push('\n');
        out.push_str(&table_text("layouts.elements", &element_keys(e)?));
    }
    Ok(out)
}

/// A line of the text: its bytes with the line break, and whether it starts outside any string, array and inline
/// table. Only such a line can be a header, a key or a comment of its own.
struct Line {
    start: usize,
    end: usize,
    top: bool,
}

/// What a line is.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Blank,
    Comment,
    /// A table header: the key path, and whether it is `[[...]]`.
    Header(Vec<String>, bool),
    /// A key, by the first part of its path (`map_frame` for `map_frame.margin_mm = 3`).
    Key(String),
    /// A line inside a value that started on a line above.
    Inner,
}

/// The lines of the project file text, sorted into headers, keys and the rest.
struct Doc<'a> {
    text: &'a str,
    lines: Vec<Line>,
    kinds: Vec<Kind>,
}

impl<'a> Doc<'a> {
    fn new(text: &'a str) -> Self {
        let lines = split_lines(text);
        let kinds = lines.iter().map(|l| kind_of(text, l)).collect();
        Doc { text, lines, kinds }
    }

    /// The byte where line `i` starts; the end of the text for the line after the last.
    fn byte(&self, i: usize) -> usize {
        self.lines.get(i).map_or(self.text.len(), |l| l.start)
    }

    /// Line `i` with its line break.
    fn line_text(&self, i: usize) -> &str {
        &self.text[self.lines[i].start..self.lines[i].end]
    }

    fn name(&self, i: usize) -> Option<&[String]> {
        match self.kinds.get(i)? {
            Kind::Header(name, _) => Some(name),
            _ => None,
        }
    }

    /// The lines that are table headers.
    fn headers(&self) -> Vec<usize> {
        (0..self.lines.len())
            .filter(|&i| self.name(i).is_some())
            .collect()
    }

    /// The headers `[[name]]` in the lines `within`.
    fn arrays(&self, name: &[&str], within: Range<usize>) -> Vec<usize> {
        within
            .filter(|&i| {
                matches!(&self.kinds[i], Kind::Header(n, true) if n.iter().map(String::as_str).eq(name.iter().copied()))
            })
            .collect()
    }

    /// The line after the table whose header is line `h`: the next header, or the end.
    fn table_end(&self, h: usize) -> usize {
        (h + 1..self.lines.len())
            .find(|&i| self.name(i).is_some())
            .unwrap_or(self.lines.len())
    }

    /// The line after the table at `h` and the tables inside it (`[layouts.map_frame]` after `[[layouts]]`).
    fn group_end(&self, h: usize) -> usize {
        let name = self.name(h).unwrap_or_default();
        let mut end = self.table_end(h);
        while let Some(inner) = self.name(end) {
            if inner.len() > name.len() && inner.starts_with(name) {
                end = self.table_end(end);
            } else {
                break;
            }
        }
        end
    }

    /// `end` moved back over blank and comment lines, not before the line after `from`: those lines belong to the
    /// table that follows.
    fn trimmed(&self, from: usize, end: usize) -> usize {
        let mut end = end;
        while end > from + 1 && matches!(self.kinds[end - 1], Kind::Blank | Kind::Comment) {
            end -= 1;
        }
        end
    }

    /// The keys of the lines `from..to` (a table body): the first part of each key and its lines.
    fn entries(&self, from: usize, to: usize) -> Vec<(String, Range<usize>)> {
        let mut out: Vec<(String, Range<usize>)> = Vec::new();
        for i in from..to {
            match &self.kinds[i] {
                Kind::Key(key) => out.push((key.clone(), i..i + 1)),
                Kind::Inner => {
                    if let Some((_, range)) = out.last_mut()
                        && range.end == i
                    {
                        range.end = i + 1;
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The edits that turn the table at header `h` (with its inner tables up to line `group_end`) from `old` into
    /// `new`, both from the same `*_keys` function. A key of the same value is left alone.
    fn edit_keys(&self, h: usize, group_end: usize, old: &Keys, new: &Keys, edits: &mut Vec<Edit>) {
        let name = self.name(h).unwrap_or_default().to_vec();
        let body_end = self.table_end(h);
        let entries = self.entries(h + 1, body_end);
        let insert_at = entries
            .last()
            .map_or(self.lines[h].end, |(_, r)| self.byte(r.end));
        let mut added = String::new();
        for ((key, before), (_, after)) in old.iter().zip(new) {
            if before == after {
                continue;
            }
            let line = after.as_ref().map(|v| format!("{key} = {v}\n"));
            let mut placed = false;
            for (_, range) in entries.iter().filter(|(k, _)| k == key) {
                let with = if placed {
                    String::new()
                } else {
                    placed = true;
                    line.clone().unwrap_or_default()
                };
                edits.push((self.byte(range.start)..self.byte(range.end), with));
            }
            // The key written as an inner table `[name.key]`, with the blank and comment lines before it. Its comment
            // lines, those above the header and those inside, move to the line that replaces it.
            let mut path = name.clone();
            path.push((*key).to_string());
            let mut comments = String::new();
            for inner in (body_end..group_end).filter(|&i| {
                self.name(i)
                    .is_some_and(|n| n.len() >= path.len() && n.starts_with(&path))
            }) {
                let mut start = inner;
                while start > h + 1 && matches!(self.kinds[start - 1], Kind::Blank | Kind::Comment)
                {
                    start -= 1;
                }
                let end = self.trimmed(inner, self.table_end(inner));
                for i in (start..end).filter(|&i| self.kinds[i] == Kind::Comment) {
                    comments.push_str(self.line_text(i).trim_end_matches(['\r', '\n']));
                    comments.push('\n');
                }
                edits.push((self.byte(start)..self.byte(end), String::new()));
            }
            if !placed && let Some(line) = line {
                added.push_str(&comments);
                added.push_str(&line);
            }
        }
        if !added.is_empty() {
            edits.push((insert_at..insert_at, added));
        }
    }
}

/// The lines of `text`, each marked whether it starts outside strings, arrays and inline tables.
fn split_lines(text: &str) -> Vec<Line> {
    #[derive(Clone, Copy, PartialEq)]
    enum Str {
        None,
        Basic,
        Literal,
        MultiBasic,
        MultiLiteral,
    }
    let b = text.as_bytes();
    let mut lines = Vec::new();
    let mut depth = 0usize;
    let mut s = Str::None;
    let mut start = 0;
    let mut top = true;
    let mut i = 0;
    // Skips the closing quotes of a multi-line string at `i` and up to two more quotes, which belong to the
    // string (`"""a""""` ends with a quote).
    let close = |i: &mut usize, q: u8| {
        *i += 2;
        let mut extra = 0;
        while extra < 2 && b.get(*i + 1) == Some(&q) {
            *i += 1;
            extra += 1;
        }
    };
    while i < b.len() {
        let c = b[i];
        match s {
            Str::None => match c {
                b'#' => {
                    while i + 1 < b.len() && b[i + 1] != b'\n' {
                        i += 1;
                    }
                }
                b'"' if b[i..].starts_with(b"\"\"\"") => {
                    s = Str::MultiBasic;
                    i += 2;
                }
                b'"' => s = Str::Basic,
                b'\'' if b[i..].starts_with(b"'''") => {
                    s = Str::MultiLiteral;
                    i += 2;
                }
                b'\'' => s = Str::Literal,
                b'[' | b'{' => depth += 1,
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            },
            Str::Basic | Str::MultiBasic if c == b'\\' => {
                // The escaped byte, unless it is a line break (a line-ending backslash).
                if b.get(i + 1).is_some_and(|&n| n != b'\n') {
                    i += 1;
                }
            }
            Str::Basic if c == b'"' => s = Str::None,
            Str::Literal if c == b'\'' => s = Str::None,
            // A string on one line that runs to the line break is broken; the line ends it.
            Str::Basic | Str::Literal if c == b'\n' => s = Str::None,
            Str::MultiBasic if b[i..].starts_with(b"\"\"\"") => {
                close(&mut i, b'"');
                s = Str::None;
            }
            Str::MultiLiteral if b[i..].starts_with(b"'''") => {
                close(&mut i, b'\'');
                s = Str::None;
            }
            _ => {}
        }
        if b.get(i) == Some(&b'\n') {
            lines.push(Line {
                start,
                end: i + 1,
                top,
            });
            start = i + 1;
            top = depth == 0 && s == Str::None;
        }
        i += 1;
    }
    if start < b.len() {
        lines.push(Line {
            start,
            end: b.len(),
            top,
        });
    }
    lines
}

fn kind_of(text: &str, line: &Line) -> Kind {
    if !line.top {
        return Kind::Inner;
    }
    // A byte order mark before the first line is no part of it.
    let t = text[line.start..line.end]
        .trim_start_matches('\u{feff}')
        .trim();
    if t.is_empty() {
        Kind::Blank
    } else if t.starts_with('#') {
        Kind::Comment
    } else if let Some(rest) = t.strip_prefix("[[") {
        Kind::Header(key_path(rest.split("]]").next().unwrap_or_default()), true)
    } else if let Some(rest) = t.strip_prefix('[') {
        Kind::Header(key_path(rest.split(']').next().unwrap_or_default()), false)
    } else {
        let key = until_outside_quotes(t, b'=');
        Kind::Key(key_path(key).into_iter().next().unwrap_or_default())
    }
}

/// The part of `t` before the first `stop` byte outside quotes.
fn until_outside_quotes(t: &str, stop: u8) -> &str {
    let mut quote = None;
    for (i, c) in t.bytes().enumerate() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == b'"' || c == b'\'' => quote = Some(c),
            None if c == stop => return &t[..i],
            None => {}
        }
    }
    t
}

/// The parts of a dotted key, without the quotes of quoted parts.
fn key_path(key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = key;
    loop {
        let part = until_outside_quotes(rest, b'.');
        let trimmed = part.trim();
        let unquoted = trimmed
            .strip_prefix('"')
            .and_then(|p| p.strip_suffix('"'))
            .or_else(|| {
                trimmed
                    .strip_prefix('\'')
                    .and_then(|p| p.strip_suffix('\''))
            })
            .unwrap_or(trimmed);
        out.push(unquoted.to_string());
        if part.len() == rest.len() {
            return out;
        }
        rest = &rest[part.len() + 1..];
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        Anchor, AnchorTarget, Corner, ElementKind, FILE_NAME, MapFrameSpec, NorthLines,
    };
    use super::*;
    use crate::print::{Orientation, Paper};

    const PROJECT: &str = r#"# head comment
format = 1

[event]
title = "T"

[sources]
map = "m.omap"

[map]
kind = "forest"

[print]
copyright = """
[[layouts]]
not a header
"""

# shared comment
[[shared]]
id = "title"
kind = "title_block"
lines = [
    "{event}",
    "[[layouts]]",
]
anchor = { to = "map_frame", corner = "top_left" }

# layout comment
[[layouts]]
id = "a4"
paper = "A4"   # paper comment
# frame comment
map_frame = { margin_mm = 12.0 }

[[layouts.elements]]
id = "bar"
kind = "scale_check_bar"
# anchor comment
anchor = { to = "page", corner = "bottom_right" }

# assignments comment
[[assignments]]
course = "A"
layout = "a4"
"#;

    fn temp(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("m.omap"), "x").unwrap();
        let path = dir.path().join(FILE_NAME);
        std::fs::write(&path, body).unwrap();
        (dir, path)
    }

    fn a4(lp: &LoadedProject) -> LayoutSpec {
        lp.project.layouts[0].clone()
    }

    fn text(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn element(id: &str, kind: ElementKind) -> ElementSpec {
        ElementSpec {
            id: id.into(),
            kind,
            anchor: Anchor {
                to: AnchorTarget::Page,
                corner: Corner::TopRight,
                offset_mm: [3.0, 4.0],
            },
            lines: Vec::new(),
            size_pt: None,
            box_white: None,
            file: None,
            width_mm: None,
            cell_mm: None,
        }
    }

    #[test]
    fn a_new_key_goes_after_the_last_key_and_the_rest_stays() {
        let (_d, path) = temp(PROJECT);
        let mut layout = a4(&load(&path).unwrap());
        layout.print_scale = Some(5000);
        let (lp, change) = set_layout(&path, &layout).unwrap();
        assert_eq!(change, Change::Changed);
        assert_eq!(lp.project.layouts[0].print_scale, Some(5000));
        let expected = PROJECT.replace(
            "map_frame = { margin_mm = 12.0 }\n",
            "map_frame = { margin_mm = 12.0 }\nprint_scale = 5000\n",
        );
        assert_eq!(text(&path), expected);
        // Asked again, nothing is written.
        let (_, change) = set_layout(&path, &layout).unwrap();
        assert_eq!(change, Change::Unchanged);
        assert_eq!(text(&path), expected);
    }

    #[test]
    fn a_changed_key_keeps_its_place_and_the_comment_above_it() {
        let (_d, path) = temp(PROJECT);
        let mut layout = a4(&load(&path).unwrap());
        layout.map_frame.margin_mm = 10.0;
        layout.orientation = Orientation::Landscape;
        set_layout(&path, &layout).unwrap();
        let expected = PROJECT.replace(
            "# frame comment\nmap_frame = { margin_mm = 12.0 }\n",
            "# frame comment\nmap_frame = { margin_mm = 10.0, border_mm = 0.25 }\norientation = \"landscape\"\n",
        );
        assert_eq!(text(&path), expected);
        assert_eq!(load(&path).unwrap().project.layouts[0], layout);
    }

    #[test]
    fn a_key_that_goes_away_loses_its_line() {
        let (_d, path) =
            temp(&PROJECT.replace("paper = \"A4\"", "print_scale = 4000\npaper = \"A4\""));
        let mut layout = a4(&load(&path).unwrap());
        layout.print_scale = None;
        set_layout(&path, &layout).unwrap();
        assert_eq!(text(&path), PROJECT);
    }

    #[test]
    fn a_new_layout_follows_the_last_one_and_its_elements() {
        let (_d, path) = temp(PROJECT);
        let layout = LayoutSpec {
            id: "a3".into(),
            paper: Paper::A3,
            orientation: Orientation::Auto,
            print_scale: Some(7500),
            map_frame: MapFrameSpec::default(),
            north_lines: NorthLines::Auto,
            hide_parts: Vec::new(),
            elements: vec![element("note", ElementKind::Text)],
            hide_shared: vec!["title".into()],
        };
        let (lp, change) = set_layout(&path, &layout).unwrap();
        assert_eq!(change, Change::Added);
        assert_eq!(lp.project.layouts[1], layout);
        let new = "\n[[layouts]]\nid = \"a3\"\npaper = \"A3\"\norientation = \"auto\"\nprint_scale = 7500\n\
                   map_frame = { margin_mm = 8.0, border_mm = 0.25 }\nnorth_lines = \"auto\"\n\
                   hide_shared = [\"title\"]\n\n[[layouts.elements]]\nid = \"note\"\nkind = \"text\"\n\
                   anchor = { to = \"page\", corner = \"top_right\", offset_mm = [3.0, 4.0] }\n";
        let at = PROJECT.find("\n# assignments comment").unwrap();
        let mut expected = PROJECT.to_string();
        expected.insert_str(at, new);
        assert_eq!(text(&path), expected);
    }

    #[test]
    fn a_map_frame_written_as_a_table_becomes_one_line() {
        let body = PROJECT.replace(
            "# frame comment\nmap_frame = { margin_mm = 12.0 }\n",
            "\n[layouts.map_frame]\nmargin_mm = 12.0\n",
        );
        let (_d, path) = temp(&body);
        let mut layout = a4(&load(&path).unwrap());
        layout.map_frame.crop = true;
        set_layout(&path, &layout).unwrap();
        let new = text(&path);
        assert!(!new.contains("[layouts.map_frame]"), "{new}");
        assert!(
            new.contains(
                "paper = \"A4\"   # paper comment\nmap_frame = { margin_mm = 12.0, border_mm = 0.25, crop = true }\n\n[[layouts.elements]]"
            ),
            "{new}"
        );
        assert_eq!(load(&path).unwrap().project.layouts[0], layout);
    }

    #[test]
    fn a_layout_that_would_not_load_is_refused_before_anything_is_written() {
        let (_d, path) = temp(PROJECT);
        let mut layout = a4(&load(&path).unwrap());
        layout.map_frame.margin_mm = 200.0;
        let err = set_layout(&path, &layout).unwrap_err();
        assert!(
            matches!(&err, ProjectError::Invalid { field, .. } if field == "layouts[0].map_frame.margin_mm"),
            "{err:?}"
        );
        assert_eq!(text(&path), PROJECT);
    }

    #[test]
    fn layouts_written_inline_are_left_to_the_user() {
        let body = "format = 1\nlayouts = [{ id = \"a4\" }]\n[event]\ntitle = \"T\"\n[sources]\nmap = \"m.omap\"\n\
                    [map]\nkind = \"forest\"\n";
        let (_d, path) = temp(body);
        let mut layout = a4(&load(&path).unwrap());
        layout.print_scale = Some(5000);
        let err = set_layout(&path, &layout).unwrap_err();
        assert!(matches!(err, ProjectError::NotEditable { .. }), "{err:?}");
        assert!(err.to_string().contains("by hand"), "{err}");
        assert_eq!(text(&path), body);
    }

    #[test]
    fn an_element_is_replaced_in_place_and_new_ones_follow_their_kind() {
        let (_d, path) = temp(PROJECT);
        let mut bar = load(&path).unwrap().project.layouts[0].elements[0].clone();
        bar.anchor.offset_mm = [5.0, 5.0];
        let (_, change) = set_element(&path, Some("a4"), &bar).unwrap();
        assert_eq!(change, Change::Changed);
        let replaced = PROJECT.replace(
            "# anchor comment\nanchor = { to = \"page\", corner = \"bottom_right\" }\n",
            "# anchor comment\nanchor = { to = \"page\", corner = \"bottom_right\", offset_mm = [5.0, 5.0] }\n",
        );
        assert_eq!(text(&path), replaced);
        // A new element of the layout follows its last one.
        let mut note = element("note", ElementKind::Text);
        note.lines = vec!["one".into(), "two".into()];
        note.size_pt = Some(9.0);
        let (lp, change) = set_element(&path, Some("a4"), &note).unwrap();
        assert_eq!(change, Change::Added);
        assert_eq!(lp.project.layouts[0].elements[1], note);
        let new = "\n[[layouts.elements]]\nid = \"note\"\nkind = \"text\"\nlines = [\n    \"one\",\n    \"two\",\n]\n\
                   size_pt = 9.0\nanchor = { to = \"page\", corner = \"top_right\", offset_mm = [3.0, 4.0] }\n";
        let mut expected = replaced.clone();
        expected.insert_str(replaced.find("\n# assignments comment").unwrap(), new);
        assert_eq!(text(&path), expected);
        // A new shared element follows the last shared one.
        let credit = element("credit", ElementKind::CreditLine);
        let (lp, _) = set_element(&path, None, &credit).unwrap();
        assert_eq!(lp.project.shared[1], credit);
        let new = text(&path);
        let at = new.find("[[shared]]\nid = \"credit\"").unwrap();
        assert!(at < new.find("# layout comment").unwrap(), "{new}");
        assert!(
            at > new.find("anchor = { to = \"map_frame\"").unwrap(),
            "{new}"
        );
    }

    #[test]
    fn the_first_element_of_a_layout_and_the_first_shared_one_find_their_place() {
        let body = PROJECT
            .replace(
                "\n[[layouts.elements]]\nid = \"bar\"\nkind = \"scale_check_bar\"\n# anchor comment\nanchor = { to = \"page\", corner = \"bottom_right\" }\n",
                "",
            )
            .replace(
                "# shared comment\n[[shared]]\nid = \"title\"\nkind = \"title_block\"\nlines = [\n    \"{event}\",\n    \"[[layouts]]\",\n]\nanchor = { to = \"map_frame\", corner = \"top_left\" }\n\n",
                "",
            );
        let (_d, path) = temp(&body);
        let bar = element("bar", ElementKind::ScaleCheckBar);
        set_element(&path, Some("a4"), &bar).unwrap();
        set_element(&path, None, &element("credit", ElementKind::CreditLine)).unwrap();
        let lp = load(&path).unwrap();
        assert_eq!(lp.project.layouts[0].elements, [bar]);
        assert_eq!(lp.project.shared.len(), 1);
        let new = text(&path);
        assert!(
            new.find("[[shared]]").unwrap() < new.find("# layout comment").unwrap(),
            "{new}"
        );
    }

    #[test]
    fn an_element_id_taken_by_a_shared_element_or_an_unknown_layout_is_refused() {
        let (_d, path) = temp(PROJECT);
        let err = set_element(&path, Some("a4"), &element("title", ElementKind::Text)).unwrap_err();
        assert!(matches!(err, ProjectError::DuplicateId { .. }), "{err:?}");
        let err = set_element(&path, Some("a5"), &element("x", ElementKind::Text)).unwrap_err();
        assert!(err.to_string().contains("`a5`"), "{err}");
        assert_eq!(text(&path), PROJECT);
    }

    #[test]
    fn an_assignment_changes_its_keys_or_is_added_after_the_last_one() {
        let (_d, path) = temp(PROJECT);
        let with_mask = Assignment {
            course: "A".into(),
            layout: "a4".into(),
            mask: Some("whiteout-1".into()),
        };
        let (_, change) = set_assignment(&path, &with_mask).unwrap();
        assert_eq!(change, Change::Changed);
        assert_eq!(text(&path), format!("{PROJECT}mask = \"whiteout-1\"\n"),);
        let b = Assignment {
            course: "B".into(),
            layout: "a4".into(),
            mask: None,
        };
        let (lp, change) = set_assignment(&path, &b).unwrap();
        assert_eq!(change, Change::Added);
        assert_eq!(lp.project.assignments[1], b);
        assert_eq!(
            text(&path),
            format!(
                "{PROJECT}mask = \"whiteout-1\"\n\n[[assignments]]\ncourse = \"B\"\nlayout = \"a4\"\n"
            ),
        );
        // Without a mask again: the line goes.
        set_assignment(
            &path,
            &Assignment {
                mask: None,
                ..with_mask
            },
        )
        .unwrap();
        assert_eq!(
            text(&path),
            format!("{PROJECT}\n[[assignments]]\ncourse = \"B\"\nlayout = \"a4\"\n"),
        );
        let err = set_assignment(
            &path,
            &Assignment {
                layout: "a5".into(),
                ..b
            },
        )
        .unwrap_err();
        assert!(matches!(err, ProjectError::Invalid { .. }), "{err:?}");
    }

    #[test]
    fn a_first_assignment_goes_at_the_end_and_line_breaks_stay_crlf() {
        let body = PROJECT
            .replace(
                "# assignments comment\n[[assignments]]\ncourse = \"A\"\nlayout = \"a4\"\n",
                "",
            )
            .replace('\n', "\r\n");
        let (_d, path) = temp(&body);
        let a = Assignment {
            course: "A".into(),
            layout: "a4".into(),
            mask: None,
        };
        set_assignment(&path, &a).unwrap();
        assert_eq!(
            text(&path),
            format!("{body}\r\n[[assignments]]\r\ncourse = \"A\"\r\nlayout = \"a4\"\r\n"),
        );
    }

    #[test]
    fn a_line_with_a_line_break_is_written_escaped_on_lf_and_crlf_files() {
        for newline in ["\n", "\r\n"] {
            let body = PROJECT.replace('\n', newline);
            let (_d, path) = temp(&body);
            let mut note = element("note", ElementKind::Text);
            note.lines = vec!["a\nb".into(), "it's \"quoted\"\r\n\tend".into()];
            let (lp, change) = set_element(&path, Some("a4"), &note).unwrap();
            assert_eq!(change, Change::Added);
            assert_eq!(lp.project.layouts[0].elements[1], note);
            let mut title = lp.project.shared[0].clone();
            title.lines = vec!["x\ny".into()];
            let (lp, change) = set_element(&path, None, &title).unwrap();
            assert_eq!(change, Change::Changed);
            assert_eq!(lp.project.shared[0], title);
            let new = text(&path);
            assert!(new.contains(r#"    "a\nb","#), "{new:?}");
            assert!(new.contains(r#"lines = ["x\ny"]"#), "{new:?}");
            // Every line break of the file is still the file's.
            assert_eq!(
                new.matches('\n').count(),
                new.matches(newline).count(),
                "{new:?}"
            );
        }
    }

    #[test]
    fn a_byte_order_mark_stays_where_it_is() {
        let body = format!("\u{feff}{PROJECT}");
        let (_d, path) = temp(&body);
        let mut layout = a4(&load(&path).unwrap());
        layout.print_scale = Some(5000);
        set_layout(&path, &layout).unwrap();
        assert_eq!(
            text(&path),
            body.replace(
                "map_frame = { margin_mm = 12.0 }\n",
                "map_frame = { margin_mm = 12.0 }\nprint_scale = 5000\n",
            )
        );
        assert_eq!(Doc::new(&body).kinds[0], Kind::Comment);
    }

    #[test]
    fn headers_inside_a_multi_line_literal_string_are_text() {
        let body = PROJECT.replace(
            "    \"[[layouts]]\",\n",
            "    '''\n[[layouts]]\n[layouts.map_frame]\n''',\n",
        );
        let (_d, path) = temp(&body);
        let shared = load(&path).unwrap().project.shared;
        assert_eq!(shared[0].lines[1], "[[layouts]]\n[layouts.map_frame]\n");
        let mut layout = a4(&load(&path).unwrap());
        layout.id = "a3".into();
        let (lp, change) = set_layout(&path, &layout).unwrap();
        assert_eq!(change, Change::Added);
        assert_eq!(lp.project.layouts.len(), 2);
        assert_eq!(lp.project.shared, shared);
        let new = text(&path);
        let at = new.find("[[layouts]]\nid = \"a3\"").unwrap();
        assert!(at > new.find("id = \"bar\"").unwrap(), "{new}");
        assert_eq!(new[..at].trim_end(), body[..at].trim_end());
    }

    #[test]
    fn an_anchor_written_as_a_sub_table_becomes_one_line_and_keeps_its_comments() {
        let body = PROJECT
            .replace(
                "# anchor comment\nanchor = { to = \"page\", corner = \"bottom_right\" }\n",
                "\n# anchor comment\n[layouts.elements.anchor]\n# inside\nto = \"page\" # end of line\n\
                 corner = \"bottom_right\"\n",
            )
            .replace(
                "anchor = { to = \"map_frame\", corner = \"top_left\" }\n",
                "\n[shared.anchor]\nto = \"map_frame\"\ncorner = \"top_left\"\n",
            );
        let (_d, path) = temp(&body);
        let lp = load(&path).unwrap();
        let mut bar = lp.project.layouts[0].elements[0].clone();
        bar.anchor.offset_mm = [5.0, 5.0];
        set_element(&path, Some("a4"), &bar).unwrap();
        let mut title = lp.project.shared[0].clone();
        title.anchor.offset_mm = [1.0, 2.0];
        set_element(&path, None, &title).unwrap();
        let expected = body
            .replace(
                "kind = \"scale_check_bar\"\n\n# anchor comment\n[layouts.elements.anchor]\n# inside\n\
                 to = \"page\" # end of line\ncorner = \"bottom_right\"\n",
                "kind = \"scale_check_bar\"\n# anchor comment\n# inside\n\
                 anchor = { to = \"page\", corner = \"bottom_right\", offset_mm = [5.0, 5.0] }\n",
            )
            .replace(
                "]\n\n[shared.anchor]\nto = \"map_frame\"\ncorner = \"top_left\"\n",
                "]\nanchor = { to = \"map_frame\", corner = \"top_left\", offset_mm = [1.0, 2.0] }\n",
            );
        assert_eq!(text(&path), expected);
        let lp = load(&path).unwrap();
        assert_eq!(lp.project.layouts[0].elements[0], bar);
        assert_eq!(lp.project.shared[0], title);
    }

    #[test]
    fn the_comment_above_a_map_frame_table_moves_with_it() {
        let body = PROJECT.replace(
            "# frame comment\nmap_frame = { margin_mm = 12.0 }\n",
            "\n# frame comment\n[layouts.map_frame]\nmargin_mm = 12.0 # twelve\n",
        );
        let (_d, path) = temp(&body);
        let mut layout = a4(&load(&path).unwrap());
        layout.map_frame.crop = true;
        set_layout(&path, &layout).unwrap();
        assert_eq!(
            text(&path),
            PROJECT.replace(
                "map_frame = { margin_mm = 12.0 }",
                "map_frame = { margin_mm = 12.0, border_mm = 0.25, crop = true }",
            )
        );
    }

    #[test]
    fn a_map_frame_table_after_the_elements_of_its_layout_is_found() {
        let body = PROJECT
            .replace("# frame comment\nmap_frame = { margin_mm = 12.0 }\n", "")
            .replace(
                "anchor = { to = \"page\", corner = \"bottom_right\" }\n",
                "anchor = { to = \"page\", corner = \"bottom_right\" }\n\n# frame comment\n\
                 [layouts.map_frame]\nmargin_mm = 12.0\n",
            );
        let (_d, path) = temp(&body);
        let mut layout = a4(&load(&path).unwrap());
        assert_eq!(layout.map_frame.margin_mm, 12.0);
        layout.map_frame.margin_mm = 10.0;
        set_layout(&path, &layout).unwrap();
        assert_eq!(
            text(&path),
            PROJECT.replace(
                "map_frame = { margin_mm = 12.0 }",
                "map_frame = { margin_mm = 10.0, border_mm = 0.25 }",
            )
        );
    }

    #[test]
    fn a_file_without_a_last_line_break_gets_one_before_new_text() {
        let body = PROJECT.trim_end();
        let (_d, path) = temp(body);
        let a = Assignment {
            course: "A".into(),
            layout: "a4".into(),
            mask: Some("whiteout-1".into()),
        };
        set_assignment(&path, &a).unwrap();
        assert_eq!(text(&path), format!("{body}\nmask = \"whiteout-1\"\n"));
        // A new layout after the last table, which ends without a line break.
        let cut = PROJECT.find("\n# assignments comment").unwrap();
        let body = PROJECT[..cut].trim_end();
        let (_d, path) = temp(body);
        let mut layout = a4(&load(&path).unwrap());
        layout.id = "a3".into();
        layout.elements.clear();
        let (lp, change) = set_layout(&path, &layout).unwrap();
        assert_eq!(change, Change::Added);
        assert_eq!(lp.project.layouts[1].id, "a3");
        assert!(
            text(&path).starts_with(&format!("{body}\n\n[[layouts]]\nid = \"a3\"\n")),
            "{}",
            text(&path)
        );
    }

    #[test]
    fn multi_line_strings_of_a_value_become_basic_strings() {
        assert_eq!(one_line_strings(r#"["a", 'b']"#), r#"["a", 'b']"#);
        assert_eq!(
            one_line_strings("[\"\"\"\na\nb\"\"\", '''\nc\\d''']"),
            r#"["a\nb", "c\\d"]"#
        );
        assert_eq!(one_line_strings("'''a\"\"\"b'''"), r#""a\"\"\"b""#);
        assert_eq!(basic_string("\u{1}\u{7f}é"), r#""\u0001\u007Fé""#);
    }

    #[test]
    fn lines_inside_strings_and_arrays_are_no_headers() {
        let doc = Doc::new(PROJECT);
        let names: Vec<Vec<String>> = doc
            .headers()
            .into_iter()
            .map(|h| doc.name(h).unwrap().to_vec())
            .collect();
        let names: Vec<String> = names.iter().map(|n| n.join(".")).collect();
        assert_eq!(
            names,
            [
                "event",
                "sources",
                "map",
                "print",
                "shared",
                "layouts",
                "layouts.elements",
                "assignments"
            ]
        );
        assert_eq!(
            key_path(r#" a . "b.c" . 'd' "#),
            ["a", "b.c", "d"].map(String::from)
        );
        let doc = Doc::new("x = \"\"\"a\\\n[b]\"\"\"\"\n[c]\ny = 'a'\n");
        let names: Vec<String> = doc
            .headers()
            .into_iter()
            .map(|h| doc.name(h).unwrap().join("."))
            .collect();
        assert_eq!(names, ["c"]);
    }
}
