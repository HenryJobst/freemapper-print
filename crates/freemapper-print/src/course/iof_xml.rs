//! Reader for IOF XML 3.0 `CourseData` files (Purple Pen, Condes, OCAD and others write them).
//!
//! Streaming with quick-xml. A DOCTYPE is refused and entities other than the five predefined ones are
//! errors (ADR 0019). Positions stay as the file gives them: the reader does not flip y, because
//! writers differ and the fitting step detects the axis direction.
//!
//! What it reads: `Event/Name`, `Map/Scale`, `Control` (`Id`, `Position`, `MapPosition`), `Course` (`Name`,
//! `Length`, `Climb`, `CourseControl` with `Control`, `Score`, `MapTextPosition`). IOF XML carries no
//! descriptions, bends, gaps or special objects. A course with a fork (a `CourseControl` with several
//! `Control` children, or a `CourseVariation` element) is left out and noted.

use std::collections::HashSet;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use super::CourseError;
use super::model::*;

/// Parses the bytes of an IOF XML 3.0 `CourseData` file.
pub fn read_iof_xml(bytes: &[u8]) -> Result<RawEvent, CourseError> {
    if bytes.len() > MAX_COURSE_FILE_BYTES {
        return Err(CourseError::Limit(format!(
            "file larger than {} MiB",
            MAX_COURSE_FILE_BYTES / (1024 * 1024)
        )));
    }
    let mut state = State::default();
    let mut reader = Reader::from_reader(bytes);
    loop {
        match reader.read_event().map_err(xml_err)? {
            Event::Start(e) => state.begin(&e)?,
            Event::Empty(e) => {
                state.begin(&e)?;
                state.end()?;
            }
            Event::End(_) => state.end()?,
            Event::Text(t) => state.text.push_str(&t.xml_content(XmlVersion::Explicit1_0)),
            Event::CData(t) => state.text.push_str(&t.xml_content(XmlVersion::Explicit1_0)),
            Event::GeneralRef(r) => {
                if let Some(c) = r.resolve_char_ref().map_err(xml_err)? {
                    state.text.push(c);
                } else {
                    match r.xml_content(XmlVersion::Explicit1_0).as_ref() {
                        "lt" => state.text.push('<'),
                        "gt" => state.text.push('>'),
                        "amp" => state.text.push('&'),
                        "apos" => state.text.push('\''),
                        "quot" => state.text.push('"'),
                        other => {
                            return Err(CourseError::Xml(format!("entity &{other}; is refused")));
                        }
                    }
                }
            }
            Event::DocType(_) => return Err(CourseError::Xml("a DOCTYPE is refused".into())),
            Event::Eof => break,
            _ => {}
        }
    }
    state.finish()
}

fn xml_err(e: impl std::fmt::Display) -> CourseError {
    CourseError::Xml(e.to_string())
}

fn local(e: &BytesStart<'_>) -> String {
    e.local_name().as_ref().to_string()
}

fn attr(e: &BytesStart<'_>, name: &str) -> Result<Option<String>, CourseError> {
    let a = e.try_get_attribute(name).map_err(xml_err)?;
    a.map(|a| {
        a.normalized_value(XmlVersion::Explicit1_0)
            .map(|v| v.into_owned())
    })
    .transpose()
    .map_err(xml_err)
}

fn number(s: &str, what: &str) -> Result<f64, CourseError> {
    match s.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err(CourseError::Xml(format!("bad number in {what}: {s:?}"))),
    }
}

/// `MapPosition` / `MapTextPosition`: x and y in mm.
fn point(e: &BytesStart<'_>, what: &str) -> Result<RawPoint, CourseError> {
    match attr(e, "unit")?.as_deref() {
        None | Some("mm") => {}
        Some(u) => {
            return Err(CourseError::Unsupported(format!(
                "{what} uses unit {u:?}; only mm is supported"
            )));
        }
    }
    let get = |n: &str| -> Result<f64, CourseError> {
        let v = attr(e, n)?.ok_or_else(|| CourseError::Xml(format!("{what} lacks {n}")))?;
        number(&v, what)
    };
    Ok(RawPoint {
        x_mm: get("x")?,
        y_mm: get("y")?,
    })
}

struct ControlB {
    code: String,
    kind: ControlKind,
    position: Option<RawPoint>,
    geo: Option<(f64, f64)>,
}

struct CcB {
    codes: Vec<String>,
    text_at: Option<RawPoint>,
    score: bool,
}

struct CourseB {
    course: Course<RawPoint>,
    forked: bool,
}

#[derive(Default)]
struct State {
    stack: Vec<String>,
    text: String,
    controls: Vec<Control<RawPoint>>,
    courses: Vec<Course<RawPoint>>,
    notes: Vec<CourseNote>,
    event_name: Option<String>,
    map_scale: Option<u32>,
    control: Option<ControlB>,
    course: Option<CourseB>,
    cc: Option<CcB>,
}

impl State {
    fn note(&mut self, n: CourseNote) {
        if !self.notes.contains(&n) {
            self.notes.push(n);
        }
    }

    fn parent(&self, up: usize) -> &str {
        let n = self.stack.len();
        if n > up { &self.stack[n - 1 - up] } else { "" }
    }

    fn begin(&mut self, e: &BytesStart<'_>) -> Result<(), CourseError> {
        let name = local(e);
        if self.stack.len() >= MAX_XML_DEPTH {
            return Err(CourseError::Limit(format!(
                "nesting too deep: more than {MAX_XML_DEPTH} levels of elements"
            )));
        }
        self.text.clear();
        let parent = self.parent(0).to_string();
        match (parent.as_str(), name.as_str()) {
            ("CourseData", "Event" | "RaceCourseData") => {}
            ("CourseData", other) | ("RaceCourseData", other)
                if !matches!(other, "Map" | "Control" | "Course") =>
            {
                self.note(CourseNote::UnknownElement(other.to_string()));
            }
            _ => {}
        }
        match (parent.as_str(), name.as_str()) {
            ("RaceCourseData", "Control") => {
                if self.controls.len() >= MAX_CONTROLS {
                    return Err(CourseError::Limit(format!(
                        "more than {MAX_CONTROLS} controls"
                    )));
                }
                let kind = match attr(e, "type")?.as_deref() {
                    None | Some("Control") => ControlKind::Control,
                    Some("Start") => ControlKind::Start,
                    Some("Finish") => ControlKind::Finish,
                    Some("CrossingPoint") => ControlKind::CrossingPoint,
                    Some(other) => {
                        self.note(CourseNote::UnknownElement(format!("Control type {other}")));
                        ControlKind::Control
                    }
                };
                self.control = Some(ControlB {
                    code: String::new(),
                    kind,
                    position: None,
                    geo: None,
                });
            }
            ("Control", "Position") => {
                if let Some(c) = self.control.as_mut() {
                    let lat = attr(e, "lat")?
                        .map(|v| number(&v, "Position"))
                        .transpose()?;
                    let lng = attr(e, "lng")?
                        .map(|v| number(&v, "Position"))
                        .transpose()?;
                    if let (Some(lat), Some(lng)) = (lat, lng) {
                        c.geo = Some((lat, lng));
                    }
                }
            }
            ("Control", "MapPosition") => {
                if let Some(c) = self.control.as_mut() {
                    c.position = Some(point(e, "MapPosition")?);
                }
            }
            ("RaceCourseData", "Course") => {
                if self.courses.len() >= MAX_COURSES {
                    return Err(CourseError::Limit(format!(
                        "more than {MAX_COURSES} courses"
                    )));
                }
                self.course = Some(CourseB {
                    course: Course {
                        name: String::new(),
                        kind: CourseKind::Normal,
                        legs: Vec::new(),
                        stops: Vec::new(),
                        climb_m: None,
                        length_m: None,
                        print_scale_hint: None,
                    },
                    forked: false,
                });
            }
            ("Course", "CourseControl") => {
                if let Some(c) = self.course.as_ref()
                    && c.course.stops.len() >= MAX_STOPS_PER_COURSE
                {
                    return Err(CourseError::Limit(format!(
                        "more than {MAX_STOPS_PER_COURSE} stops in a course"
                    )));
                }
                self.cc = Some(CcB {
                    codes: Vec::new(),
                    text_at: None,
                    score: false,
                });
            }
            ("Course", "CourseVariation") => {
                if let Some(c) = self.course.as_mut() {
                    c.forked = true;
                }
            }
            ("CourseControl", "MapTextPosition") => {
                if let Some(c) = self.cc.as_mut() {
                    c.text_at = Some(point(e, "MapTextPosition")?);
                }
            }
            _ => {}
        }
        self.stack.push(name);
        Ok(())
    }

    fn end(&mut self) -> Result<(), CourseError> {
        let Some(name) = self.stack.pop() else {
            return Err(CourseError::Xml("unbalanced end tag".into()));
        };
        let parent = self.parent(0).to_string();
        let text = std::mem::take(&mut self.text);
        let text = text.trim();
        match (parent.as_str(), name.as_str()) {
            ("Event", "Name") => self.event_name = Some(text.to_string()),
            ("Map", "Scale") => {
                let v = number(text, "Scale")?;
                if v >= 1.0 && v <= f64::from(u32::MAX) {
                    self.map_scale = Some(v.round() as u32);
                }
            }
            ("Control", "Id") => {
                if let Some(c) = self.control.as_mut() {
                    c.code = text.to_string();
                }
            }
            ("RaceCourseData", "Control") => {
                let c = self.control.take().expect("control is open");
                if c.code.is_empty() {
                    return Err(CourseError::Xml("a Control has no Id".into()));
                }
                let position = c.position.ok_or_else(|| {
                    CourseError::Xml(format!("control {} has no MapPosition", c.code))
                })?;
                self.controls.push(Control {
                    code: c.code,
                    kind: c.kind,
                    position,
                    geo: c.geo,
                    description: None,
                });
            }
            ("Course", "Name") => {
                if let Some(c) = self.course.as_mut() {
                    c.course.name = text.to_string();
                }
            }
            ("Course", "Length") => {
                if let Some(c) = self.course.as_mut() {
                    c.course.length_m = Some(number(text, "Length")?);
                }
            }
            ("Course", "Climb") => {
                if let Some(c) = self.course.as_mut() {
                    c.course.climb_m = Some(number(text, "Climb")?);
                }
            }
            ("CourseControl", "Control") => {
                if let Some(c) = self.cc.as_mut() {
                    c.codes.push(text.to_string());
                }
            }
            ("CourseControl", "Score") => {
                if let Some(c) = self.cc.as_mut() {
                    c.score = true;
                }
            }
            ("Course", "CourseControl") => {
                let cc = self.cc.take().expect("course control is open");
                if let Some(c) = self.course.as_mut() {
                    if cc.codes.len() > 1 {
                        c.forked = true;
                    }
                    if cc.score {
                        c.course.kind = CourseKind::Score;
                    }
                    if !c.forked {
                        let Some(code) = cc.codes.into_iter().next().filter(|s| !s.is_empty())
                        else {
                            return Err(CourseError::Xml(
                                "a CourseControl names no Control".into(),
                            ));
                        };
                        if !c.course.stops.is_empty() {
                            c.course.legs.push(Leg {
                                bends: Vec::new(),
                                gaps: Vec::new(),
                                marked_route: false,
                            });
                        }
                        c.course.stops.push(Stop {
                            control: code,
                            number: None,
                            number_text_at: cc.text_at,
                            circle_gaps: Vec::new(),
                        });
                    }
                }
            }
            ("RaceCourseData", "Course") => {
                let c = self.course.take().expect("course is open");
                if c.forked {
                    self.note(CourseNote::Forked(c.course.name));
                } else {
                    self.courses.push(c.course);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn finish(mut self) -> Result<RawEvent, CourseError> {
        if !self.stack.is_empty() {
            return Err(CourseError::Xml("unexpected end of file".into()));
        }
        let codes: HashSet<&str> = self.controls.iter().map(|c| c.code.as_str()).collect();
        for course in &self.courses {
            for stop in &course.stops {
                if !codes.contains(stop.control.as_str()) {
                    return Err(CourseError::Xml(format!(
                        "course {} uses control {}, which is not defined",
                        course.name, stop.control
                    )));
                }
            }
        }
        self.note(CourseNote::NoDescriptions);
        Ok(CourseEvent {
            controls: self.controls,
            courses: self.courses,
            specials: Vec::new(),
            source: CourseSource {
                format: SourceFormat::IofXml,
                event_name: self.event_name,
                map_scale: self.map_scale,
            },
            notes: self.notes,
        })
    }
}
