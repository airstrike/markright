use super::op::Op;

/// A group of operations forming a single undoable action.
pub type UndoGroup = Vec<Op>;

/// Undo/redo history for document operations.
///
/// Records operations in groups. Each group is a single undoable unit.
/// History never touches the editor — it returns ops for the caller to apply.
pub struct History {
    undo_stack: Vec<UndoGroup>,
    redo_stack: Vec<UndoGroup>,
    current_group: Option<Vec<Op>>,
}

impl History {
    /// Create a new, empty history.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            current_group: None,
        }
    }

    /// Start recording a new undo group.
    ///
    /// # Panics
    ///
    /// Panics if a group is already open (no nested groups).
    pub fn begin_group(&mut self) {
        assert!(
            self.current_group.is_none(),
            "cannot nest undo groups — a group is already open"
        );
        self.current_group = Some(Vec::new());
    }

    /// Record an operation into the current group.
    ///
    /// # Panics
    ///
    /// Panics if no group is currently open.
    pub fn record(&mut self, op: Op) {
        self.current_group
            .as_mut()
            .expect("record called with no open group — call begin_group() first")
            .push(op);
    }

    /// Finalize the current group and push it to the undo stack.
    ///
    /// Empty groups are silently discarded. Recording a new group clears the
    /// redo stack (the redo branch is invalidated by new edits).
    pub fn end_group(&mut self) {
        let group = self
            .current_group
            .take()
            .expect("end_group called with no open group");
        if group.is_empty() {
            return;
        }
        self.undo_stack.push(group);
        self.redo_stack.clear();
    }

    /// Pop the most recent undo group, if any.
    ///
    /// The caller is responsible for computing inverses and applying them, then
    /// calling [`push_redo`](Self::push_redo) with the inverted group.
    pub fn undo(&mut self) -> Option<UndoGroup> {
        self.undo_stack.pop()
    }

    /// Pop the most recent redo group, if any.
    ///
    /// The caller is responsible for applying the ops and then calling
    /// [`push_undo`](Self::push_undo) with the inverted group.
    pub fn redo(&mut self) -> Option<UndoGroup> {
        self.redo_stack.pop()
    }

    /// Push a group onto the redo stack (used by Content after applying undo inverses).
    pub fn push_redo(&mut self, group: UndoGroup) {
        if !group.is_empty() {
            self.redo_stack.push(group);
        }
    }

    /// Push a group onto the undo stack (used by Content after applying redo inverses).
    pub fn push_undo(&mut self, group: UndoGroup) {
        if !group.is_empty() {
            self.undo_stack.push(group);
        }
    }

    /// Returns whether an undo operation is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns whether a redo operation is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{StyleRun, StyledText};
    use iced_core::text::rich_editor::Style;

    fn make_insert(line: usize, col: usize, text: &str) -> Op {
        Op::InsertText {
            line,
            col,
            content: StyledText {
                text: text.to_string(),
                runs: vec![StyleRun {
                    range: 0..text.len(),
                    style: Style::default(),
                }],
            },
        }
    }

    #[test]
    fn record_and_undo() {
        let mut history = History::new();
        history.begin_group();
        history.record(make_insert(0, 0, "a"));
        history.record(make_insert(0, 1, "b"));
        history.end_group();

        assert!(history.can_undo());
        let group = history.undo().unwrap();
        assert_eq!(group.len(), 2);
        assert!(!history.can_undo());
    }

    #[test]
    fn undo_then_redo() {
        let mut history = History::new();
        history.begin_group();
        history.record(make_insert(0, 0, "hello"));
        history.end_group();

        let group = history.undo().unwrap();
        assert!(!history.can_redo());

        history.push_redo(group);
        assert!(history.can_redo());

        let redo_group = history.redo().unwrap();
        assert_eq!(redo_group.len(), 1);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut history = History::new();

        // First edit
        history.begin_group();
        history.record(make_insert(0, 0, "a"));
        history.end_group();

        // Undo and push to redo
        let group = history.undo().unwrap();
        history.push_redo(group);
        assert!(history.can_redo());

        // New edit should clear redo
        history.begin_group();
        history.record(make_insert(0, 0, "b"));
        history.end_group();

        assert!(!history.can_redo());
    }

    #[test]
    fn empty_group_discarded() {
        let mut history = History::new();
        history.begin_group();
        history.end_group();

        assert!(!history.can_undo());
        assert!(history.undo().is_none());
    }

    #[test]
    fn undo_empty_returns_none() {
        let history = History::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }
}
