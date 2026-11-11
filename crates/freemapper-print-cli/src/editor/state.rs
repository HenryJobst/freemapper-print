//! The editor's undo and redo: whole-file snapshots of the project file (core ADR 0030, freemapper-courseplan
//! ADRs 0043 and 0044).
//!
//! Every successful edit pushes the text the file had before it. Undo puts the last snapshot back and keeps
//! the text it replaces for redo. Nothing here writes: the caller writes, and commits the move only when the
//! write worked. The editor remembers the revision of the file as it left it (`own`); when the file on disk
//! differs, somebody else changed it and no snapshot may be written over that change.

use freemapper_print::web::revision;

/// The most snapshots kept; the 101st edit drops the oldest.
pub(super) const CAP: usize = 100;

/// Why undo or redo does nothing.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    /// The file changed on disk since the editor's last write.
    ChangedOutside,
    /// The stack is empty.
    Nothing,
}

/// The two stacks and the revision of the file as the editor last left it.
#[derive(Debug, Default)]
pub(super) struct History {
    undo: Vec<String>,
    redo: Vec<String>,
    own: Option<String>,
}

impl History {
    pub(super) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(super) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// A new edit changed the file from `before` to `after`: `before` becomes the newest snapshot and redo is
    /// gone.
    ///
    /// When the file was not as the editor left it (`before` differs from `own`), somebody else changed it in
    /// between and the older snapshots would write over that change: they are dropped first.
    pub(super) fn record(&mut self, before: String, after: &str) {
        if self
            .own
            .as_deref()
            .is_some_and(|own| own != revision(before.as_bytes()))
        {
            self.undo.clear();
        }
        self.undo.push(before);
        if self.undo.len() > CAP {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.own = Some(revision(after.as_bytes()));
    }

    /// The text undo would write, when the file on disk (`current`) is the one the editor left.
    pub(super) fn undo_text(&self, current: &str) -> Result<&str, Refusal> {
        self.target(&self.undo, current)
    }

    /// The text redo would write.
    pub(super) fn redo_text(&self, current: &str) -> Result<&str, Refusal> {
        self.target(&self.redo, current)
    }

    fn target<'a>(&self, stack: &'a [String], current: &str) -> Result<&'a str, Refusal> {
        if self.own.as_deref() != Some(revision(current.as_bytes()).as_str()) && !self.is_idle() {
            return Err(Refusal::ChangedOutside);
        }
        stack.last().map(String::as_str).ok_or(Refusal::Nothing)
    }

    /// Nothing was recorded yet, so nothing can be out of date.
    fn is_idle(&self) -> bool {
        self.own.is_none()
    }

    /// Undo was written: the snapshot moves to redo's side and `current` (the text it replaced) goes there.
    pub(super) fn commit_undo(&mut self, current: String, written: &str) {
        self.undo.pop();
        self.redo.push(current);
        self.own = Some(revision(written.as_bytes()));
    }

    /// Redo was written.
    pub(super) fn commit_redo(&mut self, current: String, written: &str) {
        self.redo.pop();
        self.undo.push(current);
        self.own = Some(revision(written.as_bytes()));
    }

    /// The file was changed from outside: no snapshot is safe to write any more.
    pub(super) fn forget(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.own = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_101st_snapshot_drops_the_oldest() {
        let mut history = History::default();
        for n in 0..=CAP {
            history.record(format!("v{n}"), &format!("v{}", n + 1));
        }
        assert_eq!(history.undo.len(), CAP);
        assert_eq!(history.undo.first().unwrap(), "v1");
        assert_eq!(history.undo.last().unwrap(), &format!("v{CAP}"));
    }

    #[test]
    fn an_edit_over_an_outside_change_drops_the_older_snapshots() {
        let mut history = History::default();
        history.record("a".into(), "b");
        // The file became "x" outside the editor; the next edit takes it to "y".
        history.record("x".into(), "y");
        assert_eq!(history.undo, vec!["x".to_owned()]);
        // An edit on top of the editor's own result keeps the stack.
        history.record("y".into(), "z");
        assert_eq!(history.undo, vec!["x".to_owned(), "y".to_owned()]);
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut history = History::default();
        history.record("a".into(), "b");
        let text = history.undo_text("b").unwrap().to_owned();
        history.commit_undo("b".into(), &text);
        assert!(history.can_redo());
        history.record("a".into(), "c");
        assert!(!history.can_redo());
    }

    #[test]
    fn a_file_changed_outside_refuses_both_ways() {
        let mut history = History::default();
        history.record("a".into(), "b");
        assert_eq!(
            history.undo_text("b+").unwrap_err(),
            Refusal::ChangedOutside
        );
        assert_eq!(history.undo_text("b").unwrap(), "a");
        assert_eq!(history.redo_text("b").unwrap_err(), Refusal::Nothing);
        history.forget();
        assert!(!history.can_undo());
    }
}
