//! Export: the layouts of a project as PDFs, one file `<layout id>.pdf` per layout (design §5), or, with a
//! course file, the courses: one PDF `<course>.pdf` per course, or all of them as the pages of one PDF.
//!
//! The sheets are drawn and written one after the other. Each page is dropped once its PDF bytes
//! exist, and the bytes once they are on disk, so memory does not grow with the number of sheets; a PDF of
//! several pages keeps only its bytes. The CLI, the MCP server and the editor share these loops.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::course::Course;
use crate::courses::{LoadedCourses, layout_for};
use crate::omap::{MapCoord, MapDocument};
use crate::output::{OutputError, write_pdf_file};
use crate::project::{LayoutSpec, LoadedProject, SheetLang};
use crate::render::page::RenderedPage;
use crate::render::pdf::{MultiPagePdf, PdfError, PdfMeta, write_pdf};
use crate::sheet::{
    LogoCache, SheetError, SheetReport, course_file_stem, render_course_sheet, render_sheet,
};

/// A layout id asked for that the project does not have.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the project has no layout `{id}`; its layouts are {}", known.join(", "))]
pub struct UnknownLayout {
    pub id: String,
    /// The ids of the project's layouts, in file order.
    pub known: Vec<String>,
}

/// A course asked for that the course file does not have.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the course file has no course `{name}`; its courses are {}", known.join(", "))]
pub struct UnknownCourse {
    pub name: String,
    /// The names of the courses, in file order.
    pub known: Vec<String>,
}

/// Why a layout or course could not be exported. Sheets before it are written already. The message names
/// the layout or course; the cause is the `source`, so that a chain of messages names it once.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("could not draw layout `{layout}`")]
    Sheet {
        layout: String,
        #[source]
        error: SheetError,
    },
    #[error("could not write the PDF of layout `{layout}`")]
    Pdf {
        layout: String,
        #[source]
        error: PdfError,
    },
    #[error("could not write the PDF of layout `{layout}`")]
    Write {
        layout: String,
        #[source]
        error: OutputError,
    },
    #[error("could not draw course `{course}`")]
    CourseSheet {
        course: String,
        #[source]
        error: SheetError,
    },
    #[error("could not write the PDF page of course `{course}`")]
    CoursePdf {
        course: String,
        #[source]
        error: PdfError,
    },
    /// The course PDF `file` (one course's, or the single file of all) could not be written.
    #[error("could not write {}", file.display())]
    CourseWrite {
        file: PathBuf,
        #[source]
        error: OutputError,
    },
}

/// One course drawn by [`export_courses`] or [`export_courses_single_file`].
#[derive(Debug, Clone, PartialEq)]
pub struct ExportedCourse<'a> {
    pub course: &'a Course<MapCoord>,
    /// The layout the course is printed on.
    pub layout: &'a LayoutSpec,
    /// The PDF file of the course; for the single file, the file the page goes into.
    pub path: PathBuf,
    /// The page number in that file, from 1.
    pub page: usize,
    pub report: SheetReport,
}

/// One layout written by [`export_layouts`].
#[derive(Debug, Clone, PartialEq)]
pub struct Exported<'a> {
    pub layout: &'a LayoutSpec,
    /// The PDF file written.
    pub path: PathBuf,
    pub report: SheetReport,
}

/// The layouts named in `ids`, in that order and each once; every layout in file order when `ids`
/// is empty.
pub fn select_layouts<'a>(
    project: &'a LoadedProject,
    ids: &[String],
) -> Result<Vec<&'a LayoutSpec>, UnknownLayout> {
    let layouts = &project.project.layouts;
    if ids.is_empty() {
        return Ok(layouts.iter().collect());
    }
    let mut selected: Vec<&LayoutSpec> = Vec::new();
    for id in ids {
        let layout = layouts
            .iter()
            .find(|l| &l.id == id)
            .ok_or_else(|| UnknownLayout {
                id: id.clone(),
                known: layouts.iter().map(|l| l.id.clone()).collect(),
            })?;
        if !selected.iter().any(|l| l.id == layout.id) {
            selected.push(layout);
        }
    }
    Ok(selected)
}

/// The PDF file of `layout` in `out_dir`: `<layout id>.pdf`. The project loader makes sure the id
/// is a plain file name.
pub fn pdf_path(out_dir: &Path, layout: &LayoutSpec) -> PathBuf {
    out_dir.join(format!("{}.pdf", layout.id))
}

/// The PDF of one sheet, in memory, and its report with the characters the font lacks. The page
/// is dropped before this returns. `check` uses it without writing anything. `logos` holds the
/// logos of the run, shared by its sheets.
pub fn sheet_pdf(
    project: &LoadedProject,
    map: &MapDocument,
    layout: &LayoutSpec,
    logos: &LogoCache,
) -> Result<(Vec<u8>, SheetReport), ExportError> {
    let (page, mut report) =
        render_sheet(project, map, layout, logos).map_err(|error| ExportError::Sheet {
            layout: layout.id.clone(),
            error,
        })?;
    let page = TrackedPage::new(page);
    let title = format!("{} ({})", project.project.event.title, layout.id);
    let (pdf, info) =
        write_pdf(&page.page, &PdfMeta::new(&title)).map_err(|error| ExportError::Pdf {
            layout: layout.id.clone(),
            error,
        })?;
    drop(page);
    report.print.missing_glyphs = info.missing_glyphs.into_iter().collect();
    Ok((pdf, report))
}

/// Draws and writes `layouts` one after the other into `out_dir` (which must exist), each through
/// [`write_pdf_file`], and hands each written layout to `on_layout` before the next is drawn.
/// Stops at the first layout that fails; the ones before it stay written. Each logo file is
/// decoded once for all layouts ([`LogoCache`]).
pub fn export_layouts<'a>(
    project: &LoadedProject,
    map: &MapDocument,
    layouts: &[&'a LayoutSpec],
    out_dir: &Path,
    mut on_layout: impl FnMut(Exported<'a>),
) -> Result<(), ExportError> {
    let logos = LogoCache::new();
    for &layout in layouts {
        let (pdf, report) = sheet_pdf(project, map, layout, &logos)?;
        let path = pdf_path(out_dir, layout);
        write_pdf_file(&path, &pdf).map_err(|error| ExportError::Write {
            layout: layout.id.clone(),
            error,
        })?;
        drop(pdf);
        on_layout(Exported {
            layout,
            path,
            report,
        });
    }
    Ok(())
}

/// The courses named in `names`, in that order and each once; every course in file order when `names` is
/// empty. Names match exactly.
pub fn select_courses<'a>(
    courses: &'a LoadedCourses,
    names: &[String],
) -> Result<Vec<&'a Course<MapCoord>>, UnknownCourse> {
    let all = &courses.event.courses;
    if names.is_empty() {
        return Ok(all.iter().collect());
    }
    let mut selected: Vec<&Course<MapCoord>> = Vec::new();
    for name in names {
        let course = all
            .iter()
            .find(|c| &c.name == name)
            .ok_or_else(|| UnknownCourse {
                name: name.clone(),
                known: all.iter().map(|c| c.name.clone()).collect(),
            })?;
        if !selected.iter().any(|c| std::ptr::eq(*c, course)) {
            selected.push(course);
        }
    }
    Ok(selected)
}

/// The file stem of each course of the course file, in file order ([`course_file_stem`]), and the lower-case
/// stems taken. Stems are made unique over all courses, so a course keeps its file name when `--course` picks
/// only some.
fn course_stems(courses: &LoadedCourses) -> (Vec<String>, HashSet<String>) {
    let mut taken = HashSet::new();
    let stems = courses
        .event
        .courses
        .iter()
        .map(|c| course_file_stem(&c.name, &mut taken))
        .collect();
    (stems, taken)
}

/// The PDF file of each course of the course file, in file order: `<out_dir>/<stem>.pdf`.
pub fn course_pdf_paths(courses: &LoadedCourses, out_dir: &Path) -> Vec<PathBuf> {
    course_stems(courses)
        .0
        .iter()
        .map(|stem| out_dir.join(format!("{stem}.pdf")))
        .collect()
}

/// The single PDF of all courses: `<out_dir>/<event stem> - alle Bahnen.pdf` on German sheets, `… - all
/// courses.pdf` on English ones, the event stem made from the event title as course stems are, and the whole
/// unique against the course stems.
pub fn single_file_path(
    project: &LoadedProject,
    courses: &LoadedCourses,
    out_dir: &Path,
) -> PathBuf {
    let event = &project.project.event;
    let suffix = match event.lang {
        SheetLang::De => " - alle Bahnen",
        SheetLang::En => " - all courses",
    };
    let event_stem = course_file_stem(&event.title, &mut HashSet::new());
    let mut taken = course_stems(courses).1;
    let stem = course_file_stem(&format!("{event_stem}{suffix}"), &mut taken);
    out_dir.join(format!("{stem}.pdf"))
}

/// Draws `course` on its layout ([`layout_for`]), counted by [`live_pages`] while it exists.
fn course_page<'a>(
    project: &'a LoadedProject,
    map: &MapDocument,
    courses: &LoadedCourses,
    course: &Course<MapCoord>,
    logos: &LogoCache,
) -> Result<(TrackedPage, &'a LayoutSpec, SheetReport), ExportError> {
    let (layout, _) = layout_for(&project.project, &course.name);
    let (page, report) = render_course_sheet(project, map, courses, course, layout, logos)
        .map_err(|error| ExportError::CourseSheet {
            course: course.name.clone(),
            error,
        })?;
    Ok((TrackedPage::new(page), layout, report))
}

/// The PDF of one course sheet, in memory, with its layout and its report with the characters the font lacks.
/// The page is dropped before this returns. `check` uses it without writing anything.
pub fn course_sheet_pdf<'a>(
    project: &'a LoadedProject,
    map: &MapDocument,
    courses: &LoadedCourses,
    course: &Course<MapCoord>,
    logos: &LogoCache,
) -> Result<(Vec<u8>, &'a LayoutSpec, SheetReport), ExportError> {
    let (page, layout, mut report) = course_page(project, map, courses, course, logos)?;
    let title = format!("{} ({})", project.project.event.title, course.name);
    let (pdf, info) =
        write_pdf(&page.page, &PdfMeta::new(&title)).map_err(|error| ExportError::CoursePdf {
            course: course.name.clone(),
            error,
        })?;
    drop(page);
    report.print.missing_glyphs = info.missing_glyphs.into_iter().collect();
    Ok((pdf, layout, report))
}

/// Draws and writes `selected` (courses of `courses`) one after the other into `out_dir` (which must exist),
/// each to its file of [`course_pdf_paths`], and hands each written course to `on_course` before the next is
/// drawn. Stops at the first course that fails; the ones before it stay written.
pub fn export_courses<'a>(
    project: &'a LoadedProject,
    map: &MapDocument,
    courses: &'a LoadedCourses,
    selected: &[&'a Course<MapCoord>],
    out_dir: &Path,
    mut on_course: impl FnMut(ExportedCourse<'a>),
) -> Result<(), ExportError> {
    let logos = LogoCache::new();
    let paths = course_pdf_paths(courses, out_dir);
    for &course in selected {
        let (pdf, layout, report) = course_sheet_pdf(project, map, courses, course, &logos)?;
        let index = courses
            .event
            .courses
            .iter()
            .position(|c| std::ptr::eq(c, course))
            .expect("a selected course is a course of the course file");
        let path = paths[index].clone();
        write_pdf_file(&path, &pdf).map_err(|error| ExportError::CourseWrite {
            file: path.clone(),
            error,
        })?;
        drop(pdf);
        on_course(ExportedCourse {
            course,
            layout,
            path,
            page: 1,
            report,
        });
    }
    Ok(())
}

/// Draws `selected` one after the other as the pages of one PDF, [`single_file_path`] in `out_dir` (which must
/// exist), each page on its course's layout and paper. Each page is dropped once it is in the PDF, so only one
/// is held at a time; the PDF bytes grow until the file is written at the end. `on_course` gets each course
/// once its page is in the PDF, before the file is written. Returns the file written, or `None` without a
/// course: a PDF has at least one page. Nothing is written when a course fails.
pub fn export_courses_single_file<'a>(
    project: &'a LoadedProject,
    map: &MapDocument,
    courses: &'a LoadedCourses,
    selected: &[&'a Course<MapCoord>],
    out_dir: &Path,
    mut on_course: impl FnMut(ExportedCourse<'a>),
) -> Result<Option<PathBuf>, ExportError> {
    if selected.is_empty() {
        return Ok(None);
    }
    let logos = LogoCache::new();
    let path = single_file_path(project, courses, out_dir);
    let mut pdf = MultiPagePdf::new();
    for &course in selected {
        let (page, layout, mut report) = course_page(project, map, courses, course, &logos)?;
        let info = pdf
            .add_page(&page.page)
            .map_err(|error| ExportError::CoursePdf {
                course: course.name.clone(),
                error,
            })?;
        drop(page);
        report.print.missing_glyphs = info.missing_glyphs.into_iter().collect();
        on_course(ExportedCourse {
            course,
            layout,
            path: path.clone(),
            page: pdf.len(),
            report,
        });
    }
    let bytes = pdf.finish(&PdfMeta::new(&project.project.event.title));
    write_pdf_file(&path, &bytes).map_err(|error| ExportError::CourseWrite {
        file: path.clone(),
        error,
    })?;
    Ok(Some(path))
}

/// A rendered page counted by [`live_pages`] while it exists.
struct TrackedPage {
    page: RenderedPage,
    items: usize,
}

impl TrackedPage {
    fn new(page: RenderedPage) -> Self {
        let items = page.primitives.len() + page.sheet.len();
        #[cfg(any(test, feature = "test-hooks"))]
        live_pages::add(items);
        TrackedPage { page, items }
    }
}

impl Drop for TrackedPage {
    fn drop(&mut self) {
        #[cfg(any(test, feature = "test-hooks"))]
        live_pages::remove(self.items);
        #[cfg(not(any(test, feature = "test-hooks")))]
        let _ = self.items;
    }
}

/// Test hook (feature `test-hooks`): how many rendered pages [`sheet_pdf`] holds on this thread,
/// and the most at once. The counts are per thread, so tests running side by side do not mix.
#[cfg(any(test, feature = "test-hooks"))]
#[doc(hidden)]
pub mod live_pages {
    use std::cell::Cell;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Stats {
        /// Pages alive now.
        pub live: usize,
        /// Primitives and sheet items of the pages alive now.
        pub live_items: usize,
        /// Most pages alive at once since the last [`reset`].
        pub peak_live: usize,
        /// Most primitives and sheet items held at once since the last [`reset`].
        pub peak_items: usize,
    }

    const ZERO: Stats = Stats {
        live: 0,
        live_items: 0,
        peak_live: 0,
        peak_items: 0,
    };

    thread_local! {
        static STATS: Cell<Stats> = const { Cell::new(ZERO) };
    }

    /// Sets the peaks to the current counts.
    pub fn reset() {
        STATS.with(|s| {
            let now = s.get();
            s.set(Stats {
                peak_live: now.live,
                peak_items: now.live_items,
                ..now
            });
        });
    }

    pub fn stats() -> Stats {
        STATS.with(Cell::get)
    }

    pub(super) fn add(items: usize) {
        STATS.with(|s| {
            let mut st = s.get();
            st.live += 1;
            st.live_items += items;
            st.peak_live = st.peak_live.max(st.live);
            st.peak_items = st.peak_items.max(st.live_items);
            s.set(st);
        });
    }

    pub(super) fn remove(items: usize) {
        STATS.with(|s| {
            let mut st = s.get();
            st.live = st.live.saturating_sub(1);
            st.live_items = st.live_items.saturating_sub(items);
            s.set(st);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal() -> LoadedProject {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/projects/minimal")
            .join(crate::project::FILE_NAME);
        let mut p = crate::project::load(&path).unwrap();
        let mut b = p.project.layouts[0].clone();
        b.id = "b".into();
        p.project.layouts.push(b);
        p
    }

    #[test]
    fn selects_in_the_given_order_once() {
        let p = minimal();
        let ids = |sel: Vec<&LayoutSpec>| sel.iter().map(|l| l.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(select_layouts(&p, &[]).unwrap()), ["a4", "b"]);
        let ask = ["b".to_string(), "a4".to_string(), "b".to_string()];
        assert_eq!(ids(select_layouts(&p, &ask).unwrap()), ["b", "a4"]);
        let err = select_layouts(&p, &["zz".to_string()]).unwrap_err();
        assert_eq!(err.known, ["a4", "b"]);
        assert!(err.to_string().contains("`zz`") && err.to_string().contains("a4, b"));
    }

    #[test]
    fn pdf_named_after_the_layout() {
        let p = minimal();
        assert_eq!(
            pdf_path(Path::new("/out"), &p.project.layouts[1]),
            Path::new("/out/b.pdf")
        );
    }

    #[test]
    fn exports_one_page_at_a_time() {
        let p = minimal();
        let map = crate::omap::read::read_map_file(&p.map_path()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let layouts = select_layouts(&p, &[]).unwrap();
        live_pages::reset();
        let mut written = Vec::new();
        export_layouts(&p, &map, &layouts, dir.path(), |done| {
            assert_eq!(live_pages::stats().live, 0);
            assert!(std::fs::read(&done.path).unwrap().starts_with(b"%PDF"));
            written.push(done.layout.id.clone());
        })
        .unwrap();
        assert_eq!(written, ["a4", "b"]);
        assert_eq!(live_pages::stats().peak_live, 1);
    }

    const TWO_PAPERS: &str = "[[layouts]]\nid = \"a4\"\norientation = \"portrait\"\nnorth_lines = \"never\"\n\
        [[layouts]]\nid = \"a3\"\npaper = \"A3\"\norientation = \"landscape\"\nnorth_lines = \"never\"\n\
        [[assignments]]\ncourse = \"D/H 12\"\nlayout = \"a3\"\n";

    /// The MediaBoxes of a PDF in page order (as written: one per page object).
    fn media_boxes(pdf: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(pdf);
        text.match_indices("/MediaBox [")
            .map(|(i, _)| {
                let rest = &text[i + 11..];
                rest[..rest.find(']').unwrap()].to_string()
            })
            .collect()
    }

    fn page_count(pdf: &[u8]) -> usize {
        let text = String::from_utf8_lossy(pdf);
        let i = text.find("/Count ").unwrap() + 7;
        text[i..]
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .unwrap()
            .parse()
            .unwrap()
    }

    const A4_BOX: &str = "0 0 595.2756 841.8898";
    const A3_LANDSCAPE_BOX: &str = "0 0 1190.5511 841.8898";

    #[test]
    fn selects_courses_in_the_given_order_once() {
        let (_d, _p, _m, courses) = crate::sheet::course_sheet::course_project(TWO_PAPERS);
        let names =
            |sel: Vec<&Course<MapCoord>>| sel.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(select_courses(&courses, &[]).unwrap()),
            ["Kurz", "D/H 12"]
        );
        let ask = [
            "D/H 12".to_string(),
            "Kurz".to_string(),
            "D/H 12".to_string(),
        ];
        assert_eq!(
            names(select_courses(&courses, &ask).unwrap()),
            ["D/H 12", "Kurz"]
        );
        let err = select_courses(&courses, &["kurz".to_string()]).unwrap_err();
        assert_eq!(err.known, ["Kurz", "D/H 12"]);
        assert!(err.to_string().contains("`kurz`"), "{err}");
    }

    #[test]
    fn export_per_course() {
        let (dir, p, map, courses) = crate::sheet::course_sheet::course_project(TWO_PAPERS);
        let out = dir.path().join("out");
        std::fs::create_dir(&out).unwrap();
        let selected = select_courses(&courses, &[]).unwrap();
        let mut done = Vec::new();
        export_courses(&p, &map, &courses, &selected, &out, |c| {
            assert_eq!(c.page, 1);
            done.push((c.course.name.clone(), c.layout.id.clone(), c.path.clone()));
        })
        .unwrap();
        assert_eq!(
            done,
            [
                ("Kurz".to_string(), "a4".to_string(), out.join("Kurz.pdf")),
                (
                    "D/H 12".to_string(),
                    "a3".to_string(),
                    out.join("D-H 12.pdf")
                ),
            ]
        );
        let kurz = std::fs::read(out.join("Kurz.pdf")).unwrap();
        assert_eq!(page_count(&kurz), 1);
        assert_eq!(media_boxes(&kurz), [A4_BOX]);
        assert!(
            kurz.windows(4).any(|w| w == b"/Mul"),
            "overprint with Multiply"
        );
        let dh = std::fs::read(out.join("D-H 12.pdf")).unwrap();
        assert_eq!(media_boxes(&dh), [A3_LANDSCAPE_BOX]);
        // A filtered export keeps the file names.
        let only = select_courses(&courses, &["D/H 12".to_string()]).unwrap();
        let mut paths = Vec::new();
        export_courses(&p, &map, &courses, &only, &out, |c| paths.push(c.path)).unwrap();
        assert_eq!(paths, [out.join("D-H 12.pdf")]);
    }

    #[test]
    fn export_single_file_has_one_page_per_course() {
        let (dir, p, map, courses) = crate::sheet::course_sheet::course_project(TWO_PAPERS);
        let selected = select_courses(&courses, &[]).unwrap();
        let mut pages = Vec::new();
        let path = export_courses_single_file(&p, &map, &courses, &selected, dir.path(), |c| {
            pages.push((c.page, c.course.name.clone(), c.path.clone()));
        })
        .unwrap()
        .unwrap();
        assert_eq!(path, dir.path().join("Lübeck Training - alle Bahnen.pdf"));
        assert_eq!(
            pages,
            [
                (1, "Kurz".to_string(), path.clone()),
                (2, "D/H 12".to_string(), path.clone())
            ]
        );
        let pdf = std::fs::read(&path).unwrap();
        assert_eq!(page_count(&pdf), 2);
        assert_eq!(media_boxes(&pdf), [A4_BOX, A3_LANDSCAPE_BOX]);
        assert!(!dir.path().join("Kurz.pdf").exists());
    }

    #[test]
    fn single_file_without_courses_writes_nothing() {
        let (dir, p, map, mut courses) = crate::sheet::course_sheet::course_project(TWO_PAPERS);
        courses.event.courses.clear();
        let none = export_courses_single_file(&p, &map, &courses, &[], dir.path(), |_| {}).unwrap();
        assert_eq!(none, None);
        assert!(
            !dir.path()
                .join("Lübeck Training - alle Bahnen.pdf")
                .exists()
        );
    }

    #[test]
    fn single_file_name_differs_from_every_course_file() {
        let (_dir, mut p, _map, mut courses) =
            crate::sheet::course_sheet::course_project(TWO_PAPERS);
        let out = Path::new("/out");
        // An event titled like one of its courses.
        p.project.event.title = "Kurz".into();
        assert_eq!(
            single_file_path(&p, &courses, out),
            out.join("Kurz - alle Bahnen.pdf")
        );
        p.project.event.lang = SheetLang::En;
        assert_eq!(
            single_file_path(&p, &courses, out),
            out.join("Kurz - all courses.pdf")
        );
        // A course that already has the single file's name keeps it; the single file moves aside.
        courses.event.courses[1].name = "kurz - ALL courses".into();
        assert_eq!(
            single_file_path(&p, &courses, out),
            out.join("Kurz - all courses (2).pdf")
        );
        assert_eq!(
            course_pdf_paths(&courses, out)[1],
            out.join("kurz - ALL courses.pdf")
        );
    }

    #[test]
    fn single_file_holds_one_page_at_a_time() {
        let (dir, p, map, courses) = crate::sheet::course_sheet::course_project(TWO_PAPERS);
        let selected = select_courses(&courses, &[]).unwrap();
        live_pages::reset();
        let mut seen = 0;
        export_courses_single_file(&p, &map, &courses, &selected, dir.path(), |_| {
            assert_eq!(live_pages::stats().live, 0);
            seen += 1;
        })
        .unwrap();
        assert_eq!(seen, 2);
        assert_eq!(live_pages::stats().peak_live, 1);
        live_pages::reset();
        export_courses(&p, &map, &courses, &selected, dir.path(), |_| {
            assert_eq!(live_pages::stats().live, 0);
        })
        .unwrap();
        assert_eq!(live_pages::stats().peak_live, 1);
    }

    #[test]
    fn no_course_file_exports_layouts_as_before() {
        // The minimal project has no course file: the layouts export as in M2, byte for byte as `sheet_pdf`.
        let p = minimal();
        let map = crate::omap::read::read_map_file(&p.map_path()).unwrap();
        assert!(crate::courses::load_courses(&p, &map).unwrap().is_none());
        let dir = tempfile::tempdir().unwrap();
        let layouts = select_layouts(&p, &[]).unwrap();
        export_layouts(&p, &map, &layouts, dir.path(), |_| {}).unwrap();
        let logos = LogoCache::new();
        for layout in &layouts {
            let written = std::fs::read(pdf_path(dir.path(), layout)).unwrap();
            let (pdf, report) = sheet_pdf(&p, &map, layout, &logos).unwrap();
            assert!(written == pdf, "{}", layout.id);
            assert!(
                !report
                    .notes
                    .iter()
                    .any(|n| matches!(n, crate::sheet::SheetNote::NoDescriptionBox { .. }))
            );
        }
    }

    #[test]
    fn write_error_names_the_layout() {
        let p = minimal();
        let map = crate::omap::read::read_map_file(&p.map_path()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let layouts = select_layouts(&p, &[]).unwrap();
        let err = export_layouts(&p, &map, &layouts, &missing, |_| {}).unwrap_err();
        assert!(
            matches!(&err, ExportError::Write { layout, .. } if layout == "a4"),
            "{err}"
        );
    }
}
