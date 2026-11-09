//! IOF XML 3.0 `CourseData` reader (M3 Task 2).

use freemapper_print::course::model::{
    ControlKind, CourseKind, CourseNote, MAX_CONTROLS, MAX_COURSE_FILE_BYTES, MAX_COURSES,
    MAX_STOPS_PER_COURSE, MAX_XML_DEPTH,
};
use freemapper_print::course::{CourseError, read_iof_xml};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/courses/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).unwrap()
}

fn wrap(body: &str) -> Vec<u8> {
    format!("<CourseData><RaceCourseData>{body}</RaceCourseData></CourseData>").into_bytes()
}

#[test]
fn reads_controls_courses_and_order() {
    let ev = read_iof_xml(&fixture("y-up.xml")).unwrap();
    let codes: Vec<_> = ev.controls.iter().map(|c| c.code.as_str()).collect();
    assert_eq!(codes, ["S1", "31", "32", "X1", "F1"]);
    let kinds: Vec<_> = ev.controls.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [
            ControlKind::Start,
            ControlKind::Control,
            ControlKind::Control,
            ControlKind::CrossingPoint,
            ControlKind::Finish
        ]
    );
    assert_eq!(ev.courses.len(), 2);
    assert_eq!(ev.courses[0].name, "A");
    let stops: Vec<_> = ev.courses[0]
        .stops
        .iter()
        .map(|s| s.control.as_str())
        .collect();
    assert_eq!(stops, ["S1", "31", "32", "F1"]);
    assert_eq!(ev.courses[0].legs.len(), 3);
    assert_eq!(ev.courses[0].kind, CourseKind::Normal);
    assert_eq!(ev.courses[1].name, "B");
    assert_eq!(ev.source.event_name.as_deref(), Some("Test Event"));
    assert_eq!(ev.source.map_scale, Some(10000));
}

#[test]
fn reads_map_position_mm_and_text_position() {
    let ev = read_iof_xml(&fixture("y-up.xml")).unwrap();
    let c = &ev.controls[0];
    assert_eq!((c.position.x_mm, c.position.y_mm), (50.5, 40.25));
    assert_eq!(c.geo, Some((53.8655, 10.6866)));
    assert_eq!(ev.controls[2].geo, None);

    let ev = read_iof_xml(&fixture("text-position.xml")).unwrap();
    let course = &ev.courses[0];
    assert_eq!(course.kind, CourseKind::Score);
    let p = course.stops[1].number_text_at.unwrap();
    assert_eq!((p.x_mm, p.y_mm), (66.5, 43.0));
    assert!(course.stops[1].number.is_none());
    assert!(course.stops[0].number_text_at.is_none());
}

#[test]
fn length_and_climb_taken_when_present() {
    let ev = read_iof_xml(&fixture("y-up.xml")).unwrap();
    assert_eq!(ev.courses[0].length_m, Some(4500.0));
    assert_eq!(ev.courses[0].climb_m, Some(120.0));
    assert_eq!(ev.courses[1].length_m, None);
    assert_eq!(ev.courses[1].climb_m, None);
}

#[test]
fn forked_course_noted_not_drawn() {
    let ev = read_iof_xml(&fixture("forked.xml")).unwrap();
    assert!(ev.notes.contains(&CourseNote::Forked("Relay".into())));
    assert_eq!(ev.courses.len(), 1);
    assert_eq!(ev.courses[0].name, "Plain");
}

#[test]
fn refuses_dtd_and_entities() {
    let xml = br#"<?xml version="1.0"?><!DOCTYPE CourseData [<!ENTITY x "y">]><CourseData/>"#;
    assert!(matches!(read_iof_xml(xml), Err(CourseError::Xml(_))));
    let xml = br#"<CourseData><Event><Name>&boom;</Name></Event></CourseData>"#;
    assert!(read_iof_xml(xml).is_err());
}

#[test]
fn limits_enforced() {
    // File size.
    let big = vec![b' '; MAX_COURSE_FILE_BYTES + 1];
    assert!(matches!(read_iof_xml(&big), Err(CourseError::Limit(_))));

    // Controls.
    let mut body = String::new();
    for i in 0..=MAX_CONTROLS {
        body.push_str(&format!(
            "<Control type=\"Control\"><Id>c{i}</Id><MapPosition x=\"1\" y=\"1\"/></Control>"
        ));
    }
    assert!(matches!(
        read_iof_xml(&wrap(&body)),
        Err(CourseError::Limit(_))
    ));
    // Exactly the limit passes.
    let ok = body.rsplit_once("<Control type").unwrap().0.to_string();
    assert_eq!(
        read_iof_xml(&wrap(&ok)).unwrap().controls.len(),
        MAX_CONTROLS
    );

    // Courses.
    let ctl = "<Control type=\"Start\"><Id>S</Id><MapPosition x=\"1\" y=\"1\"/></Control>";
    let mut body = ctl.to_string();
    for i in 0..=MAX_COURSES {
        body.push_str(&format!("<Course><Name>c{i}</Name></Course>"));
    }
    assert!(matches!(
        read_iof_xml(&wrap(&body)),
        Err(CourseError::Limit(_))
    ));

    // Stops per course.
    let mut body = ctl.to_string();
    body.push_str("<Course><Name>x</Name>");
    for _ in 0..=MAX_STOPS_PER_COURSE {
        body.push_str("<CourseControl type=\"Control\"><Control>S</Control></CourseControl>");
    }
    body.push_str("</Course>");
    assert!(matches!(
        read_iof_xml(&wrap(&body)),
        Err(CourseError::Limit(_))
    ));

    // Depth.
    let n = MAX_XML_DEPTH + 1;
    let xml = format!(
        "<CourseData>{}{}</CourseData>",
        "<a>".repeat(n),
        "</a>".repeat(n)
    );
    assert!(matches!(
        read_iof_xml(xml.as_bytes()),
        Err(CourseError::Limit(_))
    ));
    let n = MAX_XML_DEPTH - 1;
    let xml = format!(
        "<CourseData>{}{}</CourseData>",
        "<a>".repeat(n),
        "</a>".repeat(n)
    );
    assert!(read_iof_xml(xml.as_bytes()).is_ok());
}

#[test]
fn non_mm_unit_refused() {
    let xml = wrap(
        "<Control type=\"Start\"><Id>S</Id><MapPosition x=\"1\" y=\"1\" unit=\"px\"/></Control>",
    );
    assert!(matches!(
        read_iof_xml(&xml),
        Err(CourseError::Unsupported(_))
    ));
}

#[test]
fn bad_input_is_an_error() {
    let nan = wrap("<Control type=\"Start\"><Id>S</Id><MapPosition x=\"NaN\" y=\"1\"/></Control>");
    assert!(read_iof_xml(&nan).is_err());
    let dangling = wrap(
        "<Course><Name>x</Name><CourseControl type=\"Control\"><Control>nope</Control></CourseControl></Course>",
    );
    assert!(read_iof_xml(&dangling).is_err());
}

#[test]
fn unknown_element_noted() {
    let xml = wrap("<Gadget/>");
    let ev = read_iof_xml(&xml).unwrap();
    assert!(
        ev.notes
            .contains(&CourseNote::UnknownElement("Gadget".into()))
    );
    assert!(ev.notes.contains(&CourseNote::NoDescriptions));
}
