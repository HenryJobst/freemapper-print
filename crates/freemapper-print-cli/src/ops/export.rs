//! `export`: writes the PDFs of a project, per layout or per course (ADR 0041 for where they may go).

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use freemapper_print::export::{
    ExportedCourse, export_courses, export_courses_single_file, export_layouts, select_courses,
    select_layouts,
};
use freemapper_print::output;
use freemapper_print::project::LoadedProject;
use serde::Serialize;

use super::check::course_file_notes;
use super::{Notice, OpError, Sheet, load_project, read_courses, read_map};

/// What to export.
#[derive(Debug, Clone, Default)]
pub struct ExportArgs {
    /// The project file or its folder.
    pub project: PathBuf,
    /// Layout ids: these layouts as such, without courses. Empty: courses if the project has a course file,
    /// else every layout.
    pub layouts: Vec<String>,
    /// Course names; empty for every course. Needs a course file.
    pub courses: Vec<String>,
    /// All courses as the pages of one PDF. Needs a course file.
    pub single_file: bool,
    /// The output folder, which must exist; `None` for `out` next to the project file, created if missing.
    pub out_dir: Option<PathBuf>,
}

/// What an export writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportMode {
    /// One PDF per layout, without courses.
    Layouts,
    /// One PDF per course.
    Courses,
    /// The courses as the pages of one PDF.
    SingleFile,
}

/// One page drawn by `export`.
#[derive(Debug, Serialize)]
pub struct ExportedPage {
    /// The PDF file the page is in.
    pub path: PathBuf,
    /// The page number in that file, from 1.
    pub page: usize,
    pub sheet: Sheet,
}

/// A PDF file written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WrittenFile {
    pub path: PathBuf,
    /// The size of the file; `None` when it could not be read back.
    pub bytes: Option<u64>,
}

/// What `export` reports while it works, so that a caller can show each page before the next is drawn. A
/// course or layout that fails stops the export; the files before it stay written. The caller answers each report:
/// `ControlFlow::Break(())` stops the export before the next sheet is drawn ([`OpError::ExportStopped`]).
#[derive(Debug)]
pub enum ExportProgress<'a> {
    /// The notes of the course file as a whole, before the first course is drawn.
    CourseFileNotes(&'a [Notice]),
    /// A page is drawn and, except for the single file, written.
    Page {
        mode: ExportMode,
        page: &'a ExportedPage,
    },
}

/// What `export` wrote.
#[derive(Debug, Serialize)]
pub struct ExportResult {
    pub mode: ExportMode,
    /// The output folder.
    pub out_dir: PathBuf,
    /// The notes of the course file as a whole; `None` when the export is not per course.
    pub course_file: Option<Vec<Notice>>,
    /// The pages, in the order they were drawn. Empty when the reader left out every course: then nothing is
    /// written, not even an empty PDF.
    pub pages: Vec<ExportedPage>,
    /// The files written, each once, in the order they were written.
    pub files: Vec<WrittenFile>,
}

/// Exports the project as `args` says and hands each page to `progress` once it is drawn. When `progress`
/// answers stop, the export ends with [`OpError::ExportStopped`]: the PDFs per course or layout written so far stay
/// written, the single file is not written, and no partial file is left.
pub fn export(
    args: &ExportArgs,
    progress: &mut dyn FnMut(ExportProgress<'_>) -> ControlFlow<()>,
) -> Result<ExportResult, OpError> {
    let loaded = load_project(&args.project)?;
    let layouts =
        select_layouts(&loaded, &args.layouts).map_err(|unknown| OpError::UnknownLayout {
            id: unknown.id,
            known: unknown.known,
        })?;
    let map = read_map(&loaded.map_path())?;
    // Layouts asked for by id are exported as such, without courses, also when the project has a course file.
    let courses = if args.layouts.is_empty() {
        read_courses(&loaded, &map)?
    } else {
        None
    };
    if courses.is_none() && (!args.courses.is_empty() || args.single_file) {
        return Err(OpError::ExportNeedsCourseFile);
    }
    let selected = match &courses {
        Some(courses) => Some(select_courses(courses, &args.courses).map_err(|unknown| {
            OpError::UnknownCourse {
                name: unknown.name,
                known: unknown.known,
            }
        })?),
        None => None,
    };
    let out_dir = out_dir(args.out_dir.as_deref(), &loaded)?;
    let map_scale = map.georef.scale_denominator;
    let kind = loaded.project.map.kind;
    let mode = match (&courses, args.single_file) {
        (None, _) => ExportMode::Layouts,
        (Some(_), false) => ExportMode::Courses,
        (Some(_), true) => ExportMode::SingleFile,
    };
    let mut pages: Vec<ExportedPage> = Vec::new();
    let mut course_file = None;
    let mut stopped = false;
    match (&courses, selected) {
        (Some(courses), Some(selected)) => {
            let notes = course_file_notes(courses);
            stopped = progress(ExportProgress::CourseFileNotes(&notes)).is_break();
            course_file = Some(notes);
            if !stopped && !selected.is_empty() {
                let mut on_course = |done: ExportedCourse<'_>| {
                    let page = ExportedPage {
                        path: done.path,
                        page: done.page,
                        sheet: Sheet::new(
                            Some(&done.course.name),
                            done.layout,
                            done.report,
                            kind,
                            map_scale,
                        ),
                    };
                    let flow = progress(ExportProgress::Page { mode, page: &page });
                    stopped = flow.is_break();
                    pages.push(page);
                    flow
                };
                if args.single_file {
                    export_courses_single_file(
                        &loaded,
                        &map,
                        courses,
                        &selected,
                        &out_dir,
                        &mut on_course,
                    )
                    .map_err(OpError::Export)?;
                } else {
                    export_courses(&loaded, &map, courses, &selected, &out_dir, &mut on_course)
                        .map_err(OpError::Export)?;
                }
            }
        }
        _ => {
            export_layouts(&loaded, &map, &layouts, &out_dir, |done| {
                let page = ExportedPage {
                    path: done.path,
                    page: 1,
                    sheet: Sheet::new(None, done.layout, done.report, kind, map_scale),
                };
                let flow = progress(ExportProgress::Page { mode, page: &page });
                stopped = flow.is_break();
                pages.push(page);
                flow
            })
            .map_err(OpError::Export)?;
        }
    }
    if stopped {
        // The single file is not written when its export stops; the files per sheet are.
        let written = if mode == ExportMode::SingleFile {
            Vec::new()
        } else {
            pages.iter().map(|p| p.path.clone()).collect()
        };
        return Err(OpError::ExportStopped { written });
    }
    let mut files: Vec<WrittenFile> = Vec::new();
    for page in &pages {
        if !files.iter().any(|f| f.path == page.path) {
            files.push(WrittenFile {
                path: page.path.clone(),
                bytes: std::fs::metadata(&page.path).ok().map(|m| m.len()),
            });
        }
    }
    Ok(ExportResult {
        mode,
        out_dir,
        course_file,
        pages,
        files,
    })
}

/// The output folder: `given`, which must exist, else `out` next to the project file, created if missing. A
/// folder inside the project by name that a link leads out of the project stops the export before anything is
/// written (ADR 0041). Partial files older than an hour that interrupted runs left there are removed.
fn out_dir(given: Option<&Path>, loaded: &LoadedProject) -> Result<PathBuf, OpError> {
    let dir = match given {
        Some(dir) => {
            if !dir.is_dir() {
                return Err(OpError::OutDirMissing {
                    path: dir.to_path_buf(),
                });
            }
            output::check_out_folder(&loaded.dir, dir).map_err(OpError::Output)?;
            dir.to_path_buf()
        }
        None => {
            let dir = loaded.dir.join("out");
            output::check_out_folder(&loaded.dir, &dir).map_err(OpError::Output)?;
            // The library's writes create no folder (ADR 0041), so `out` is created here, once checked.
            std::fs::create_dir_all(&dir).map_err(|err| OpError::CreateFolder {
                path: dir.clone(),
                source: anyhow::Error::new(err),
            })?;
            dir
        }
    };
    output::remove_stale_parts(&loaded.dir, &dir);
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::viktoriapark_copy;

    #[test]
    fn viktoriapark_exports_its_course_and_reports_each_page() {
        let dir = viktoriapark_copy();
        let mut seen = Vec::new();
        let result = export(
            &ExportArgs {
                project: dir.path().to_path_buf(),
                ..ExportArgs::default()
            },
            &mut |progress| {
                match progress {
                    ExportProgress::CourseFileNotes(notes) => {
                        seen.push(format!("notes {}", notes.len()));
                    }
                    ExportProgress::Page { mode, page } => {
                        seen.push(format!("{mode:?} {}", page.sheet.layout));
                    }
                }
                ControlFlow::Continue(())
            },
        )
        .unwrap();
        assert_eq!(seen, ["notes 1", "Courses a4-4000"]);
        assert_eq!(result.mode, ExportMode::Courses);
        assert_eq!(result.out_dir, dir.path().join("out"));
        let file_notes: Vec<_> = result
            .course_file
            .as_ref()
            .unwrap()
            .iter()
            .map(Notice::kind)
            .collect();
        assert_eq!(file_notes, ["fit_georef_skipped"]);
        assert_eq!(result.pages.len(), 1);
        let sheet = &result.pages[0].sheet;
        assert_eq!(sheet.course.as_deref(), Some("Training"));
        assert_eq!(sheet.layout, "a4-4000");
        assert_eq!(sheet.map_scale, 4000);
        assert_eq!(sheet.print.print_scale, 4000);
        let kinds: Vec<_> = sheet.notes.iter().map(Notice::kind).collect();
        assert_eq!(kinds, ["map_cropped"]);
        assert_eq!(result.files.len(), 1);
        let file = &result.files[0];
        assert_eq!(file.path, result.pages[0].path);
        assert!(file.path.starts_with(dir.path().join("out")));
        assert_eq!(
            file.bytes,
            Some(std::fs::metadata(&file.path).unwrap().len())
        );
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["mode"], "courses");
        assert_eq!(json["pages"][0]["sheet"]["course"], "Training");
    }

    #[test]
    fn a_progress_that_stops_ends_the_export_with_what_was_written() {
        let dir = viktoriapark_copy();
        let stopped = |single_file: bool, at_notes: bool| {
            export(
                &ExportArgs {
                    project: dir.path().to_path_buf(),
                    single_file,
                    ..ExportArgs::default()
                },
                &mut |progress| match progress {
                    ExportProgress::CourseFileNotes(_) if !at_notes => ControlFlow::Continue(()),
                    _ => ControlFlow::Break(()),
                },
            )
            .unwrap_err()
        };
        let out = dir.path().join("out");
        let err = stopped(false, false);
        assert!(
            matches!(&err, OpError::ExportStopped { written } if written == &[out.join("Training.pdf")]),
            "{err:?}"
        );
        std::fs::remove_file(out.join("Training.pdf")).unwrap();
        // The single file is written at the end, so it is not written at all.
        let err = stopped(true, false);
        assert!(
            matches!(&err, OpError::ExportStopped { written } if written.is_empty()),
            "{err:?}"
        );
        // Stopped before the first sheet.
        let err = stopped(false, true);
        assert!(
            matches!(&err, OpError::ExportStopped { written } if written.is_empty()),
            "{err:?}"
        );
        assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    }

    #[test]
    fn layouts_by_id_leave_the_courses_out() {
        let dir = viktoriapark_copy();
        let result = export(
            &ExportArgs {
                project: dir.path().to_path_buf(),
                layouts: vec!["a4-4000".into()],
                ..ExportArgs::default()
            },
            &mut |_| ControlFlow::Continue(()),
        )
        .unwrap();
        assert_eq!(result.mode, ExportMode::Layouts);
        assert!(result.course_file.is_none());
        assert_eq!(result.pages[0].sheet.course, None);
        let unknown = export(
            &ExportArgs {
                project: dir.path().to_path_buf(),
                courses: vec!["Nope".into()],
                ..ExportArgs::default()
            },
            &mut |_| ControlFlow::Continue(()),
        )
        .unwrap_err();
        assert!(
            matches!(&unknown, OpError::UnknownCourse { name, known } if name == "Nope" && known == &["Training"]),
            "{unknown:?}"
        );
    }
}
