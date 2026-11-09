//! Export: the layouts of a project as PDFs, one file `<layout id>.pdf` per layout (design §5).
//!
//! The layouts are drawn and written one after the other. Each page is dropped once its PDF bytes
//! exist, and the bytes once they are on disk, so memory does not grow with the number of layouts.
//! The CLI, the MCP server and the editor share this loop.

use std::path::{Path, PathBuf};

use crate::omap::MapDocument;
use crate::output::{OutputError, write_pdf_file};
use crate::project::{LayoutSpec, LoadedProject};
use crate::render::page::RenderedPage;
use crate::render::pdf::{PdfError, PdfMeta, write_pdf};
use crate::sheet::{SheetError, SheetReport, render_sheet};

/// A layout id asked for that the project does not have.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the project has no layout `{id}`; its layouts are {}", known.join(", "))]
pub struct UnknownLayout {
    pub id: String,
    /// The ids of the project's layouts, in file order.
    pub known: Vec<String>,
}

/// Why a layout could not be exported. Layouts before it are written already.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("layout `{layout}`: {error}")]
    Sheet { layout: String, error: SheetError },
    #[error("layout `{layout}`: {error}")]
    Pdf { layout: String, error: PdfError },
    #[error("layout `{layout}`: {error}")]
    Write { layout: String, error: OutputError },
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
/// is dropped before this returns. `check` uses it without writing anything.
pub fn sheet_pdf(
    project: &LoadedProject,
    map: &MapDocument,
    layout: &LayoutSpec,
) -> Result<(Vec<u8>, SheetReport), ExportError> {
    let (page, mut report) =
        render_sheet(project, map, layout).map_err(|error| ExportError::Sheet {
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
/// Stops at the first layout that fails; the ones before it stay written.
pub fn export_layouts<'a>(
    project: &LoadedProject,
    map: &MapDocument,
    layouts: &[&'a LayoutSpec],
    out_dir: &Path,
    mut on_layout: impl FnMut(Exported<'a>),
) -> Result<(), ExportError> {
    for &layout in layouts {
        let (pdf, report) = sheet_pdf(project, map, layout)?;
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
