//! Integration tests for undo/redo through Content::perform — the same
//! code path used by the GUI.

use markright::widget::rich_editor::{Action, Alignment, Content, Edit, Format, Motion};

const SAMPLE: &str = include_str!("../examples/editor/sample.txt");

type C = Content<iced::Renderer>;

fn content(text: &str) -> C {
    C::with_text(text)
}

#[test]
fn select_all_center_align_then_undo() {
    let c = content(SAMPLE);

    c.perform(Action::SelectAll);
    c.perform(Format::SetAlignment(Alignment::Center));

    assert_eq!(
        c.cursor_context().paragraph.alignment,
        Alignment::Center,
        "line 0 should be centered"
    );

    c.perform(Action::Undo);

    assert_ne!(
        c.cursor_context().paragraph.alignment,
        Alignment::Center,
        "line 0 should not be centered after undo"
    );
}

#[test]
fn select_all_center_then_left_preserves_styles() {
    let c = content(SAMPLE);

    let original = c.cursor_context();

    // Center-align
    c.perform(Action::SelectAll);
    c.perform(Format::SetAlignment(Alignment::Center));

    // Left-align
    c.perform(Action::SelectAll);
    c.perform(Format::SetAlignment(Alignment::Left));

    let after = c.cursor_context();

    assert_eq!(
        after.character, original.character,
        "character styles should not change after alignment round-trip"
    );
    assert_eq!(
        after.paragraph, original.paragraph,
        "paragraph styles should not change after alignment round-trip"
    );
}

#[test]
fn select_all_type_to_replace_then_undo() {
    let c = content(SAMPLE);
    let original_line0 = c.line(0).unwrap().text.to_string();

    c.perform(Action::SelectAll);
    c.perform(Action::Edit(Edit::Insert('a')));

    assert_eq!(
        c.line(0).map(|l| l.text.to_string()),
        Some("a".to_string()),
        "text should be 'a'"
    );

    c.perform(Action::Undo);

    assert_eq!(
        c.line(0).map(|l| l.text.to_string()),
        Some(original_line0),
        "after undo, first line should be restored"
    );
}

#[test]
fn bold_italic_underline_are_additive_and_undo_independently() {
    let c = content("hello");
    let original = c.cursor_context();

    // Bold
    c.perform(Action::SelectAll);
    c.perform(Format::ToggleBold);
    let ctx = c.cursor_context();
    assert!(ctx.character.bold, "should be bold");
    assert!(!ctx.character.italic, "should not be italic yet");

    // Italic — should NOT clear bold
    c.perform(Action::SelectAll);
    c.perform(Format::ToggleItalic);
    let ctx = c.cursor_context();
    assert!(ctx.character.bold, "bold should be preserved");
    assert!(ctx.character.italic, "should be italic");

    // Underline — should NOT clear bold or italic
    c.perform(Action::SelectAll);
    c.perform(Format::ToggleUnderline);
    let ctx = c.cursor_context();
    assert!(ctx.character.bold, "bold should be preserved");
    assert!(ctx.character.italic, "italic should be preserved");
    assert!(ctx.character.underline, "should be underlined");

    // Undo underline
    c.perform(Action::Undo);
    let ctx = c.cursor_context();
    assert!(ctx.character.bold, "bold after undo underline");
    assert!(ctx.character.italic, "italic after undo underline");
    assert!(!ctx.character.underline, "underline should be gone");

    // Undo italic
    c.perform(Action::Undo);
    let ctx = c.cursor_context();
    assert!(ctx.character.bold, "bold after undo italic");
    assert!(!ctx.character.italic, "italic should be gone");

    // Undo bold — back to original
    c.perform(Action::Undo);
    let ctx = c.cursor_context();
    assert_eq!(
        ctx.character, original.character,
        "should be back to original after undoing all formatting"
    );
}

// ── Redo ────────────────────────────────────────────────────────────────

#[test]
fn redo_restores_undone_edit() {
    let c = content("hello");

    c.perform(Action::Move(Motion::End));
    c.perform(Action::Edit(Edit::Insert('!')));
    assert_eq!(c.text(), "hello!");

    c.perform(Action::Undo);
    assert_eq!(c.text(), "hello");

    c.perform(Action::Redo);
    assert_eq!(c.text(), "hello!");
}

#[test]
fn redo_restores_undone_formatting() {
    let c = content("hello");

    c.perform(Action::SelectAll);
    c.perform(Format::ToggleBold);
    assert!(c.cursor_context().character.bold, "should be bold");

    c.perform(Action::Undo);
    assert!(!c.cursor_context().character.bold, "bold undone");

    c.perform(Action::Redo);
    assert!(c.cursor_context().character.bold, "bold restored by redo");
}

#[test]
fn new_edit_clears_redo() {
    let c = content("hello");

    c.perform(Action::SelectAll);
    c.perform(Format::ToggleBold);
    c.perform(Action::Undo);
    assert!(c.can_redo(), "redo should be available after undo");

    // A new edit should clear the redo stack
    c.perform(Action::Move(Motion::End));
    c.perform(Action::Edit(Edit::Insert('x')));
    assert!(!c.can_redo(), "redo should be cleared after new edit");
}

// ── Partial selection formatting ────────────────────────────────────────

#[test]
fn bold_partial_selection() {
    let c = content("hello world");

    // Select "world" (chars 6..11): move to start, right 6, select right 5
    c.perform(Action::Move(Motion::Home));
    for _ in 0..6 {
        c.perform(Action::Move(Motion::Right));
    }
    for _ in 0..5 {
        c.perform(Action::Select(Motion::Right));
    }

    c.perform(Format::ToggleBold);

    let styled = c.styled_line(0).expect("line 0 should exist");
    assert!(
        styled.runs.len() >= 2,
        "should have multiple style runs, got: {:#?}",
        styled.runs
    );

    // The run covering "hello " should NOT be bold
    let hello_run = styled.runs.iter().find(|r| r.range.start == 0).unwrap();
    assert!(
        !hello_run.style.bold.unwrap_or(false),
        "hello should not be bold"
    );

    // The run covering "world" should be bold
    let world_run = styled.runs.iter().find(|r| r.range.contains(&6)).unwrap();
    assert!(
        world_run.style.bold.unwrap_or(false),
        "world should be bold"
    );
}

#[test]
fn format_partial_then_undo_restores_runs() {
    let c = content("hello world");
    let original_styled = c.styled_line(0).expect("line 0");

    // Select "world" and bold it
    c.perform(Action::Move(Motion::Home));
    for _ in 0..6 {
        c.perform(Action::Move(Motion::Right));
    }
    for _ in 0..5 {
        c.perform(Action::Select(Motion::Right));
    }
    c.perform(Format::ToggleBold);

    // Undo should restore the original uniform runs
    c.perform(Action::Undo);

    let restored = c.styled_line(0).expect("line 0 after undo");
    assert_eq!(
        restored.runs.len(),
        original_styled.runs.len(),
        "run count should match original after undo"
    );
    // All runs should be non-bold (matching original)
    for run in &restored.runs {
        assert!(
            !run.style.bold.unwrap_or(false),
            "no run should be bold after undo"
        );
    }
}

// ── Interleaved edit + format ───────────────────────────────────────────

#[test]
fn type_then_format_then_undo_each() {
    let c = content("hello");
    let original_text = c.text();

    // Step 1: bold all
    c.perform(Action::SelectAll);
    c.perform(Format::ToggleBold);

    // Step 2: type " world" at end
    c.perform(Action::Move(Motion::End));
    c.perform(Action::Edit(Edit::Insert(' ')));
    c.perform(Action::Edit(Edit::Insert('w')));
    c.perform(Action::Edit(Edit::Insert('o')));
    c.perform(Action::Edit(Edit::Insert('r')));
    c.perform(Action::Edit(Edit::Insert('l')));
    c.perform(Action::Edit(Edit::Insert('d')));
    assert_eq!(c.text(), "hello world");

    // Undo each character of " world" (6 undos)
    for _ in 0..6 {
        c.perform(Action::Undo);
    }
    assert_eq!(c.text(), "hello", "text after undoing inserts");

    // Undo the bold
    c.perform(Action::Undo);
    assert!(!c.cursor_context().character.bold, "bold should be undone");
    assert_eq!(c.text(), original_text, "text should match original");
}

#[test]
fn enter_then_undo_merges_line() {
    let c = content("hello world");
    let original_lines = c.line_count();

    // Position cursor after "hello" (5 chars from start)
    c.perform(Action::Move(Motion::Home));
    for _ in 0..5 {
        c.perform(Action::Move(Motion::Right));
    }

    c.perform(Action::Edit(Edit::Enter));
    assert_eq!(
        c.line_count(),
        original_lines + 1,
        "enter should add a line"
    );
    assert_eq!(c.line(0).unwrap().text.to_string(), "hello");
    assert_eq!(c.line(1).unwrap().text.to_string(), " world");

    c.perform(Action::Undo);
    assert_eq!(
        c.line_count(),
        original_lines,
        "undo should merge lines back"
    );
    assert_eq!(c.text(), "hello world");
}

// ── Multi-step undo/redo cycle ──────────────────────────────────────────

#[test]
fn full_undo_redo_cycle() {
    let c = content("a");

    // Edit 1: append "b"
    c.perform(Action::Move(Motion::End));
    c.perform(Action::Edit(Edit::Insert('b')));
    assert_eq!(c.text(), "ab");

    // Edit 2: append "c"
    c.perform(Action::Edit(Edit::Insert('c')));
    assert_eq!(c.text(), "abc");

    // Edit 3: append "d"
    c.perform(Action::Edit(Edit::Insert('d')));
    let final_text = c.text();
    assert_eq!(final_text, "abcd");

    // Undo all 3
    c.perform(Action::Undo);
    assert_eq!(c.text(), "abc");
    c.perform(Action::Undo);
    assert_eq!(c.text(), "ab");
    c.perform(Action::Undo);
    assert_eq!(c.text(), "a");

    // Redo all 3
    c.perform(Action::Redo);
    assert_eq!(c.text(), "ab");
    c.perform(Action::Redo);
    assert_eq!(c.text(), "abc");
    c.perform(Action::Redo);
    assert_eq!(
        c.text(),
        final_text,
        "redo all 3 should restore final state"
    );
}

// ── Cursor position ─────────────────────────────────────────────────────

#[test]
fn cursor_position_after_undo() {
    let c = content("abc");

    // Move to end and type "de"
    c.perform(Action::Move(Motion::End));
    let pos_before = c.cursor_context().position;

    c.perform(Action::Edit(Edit::Insert('d')));
    c.perform(Action::Edit(Edit::Insert('e')));
    assert_eq!(c.cursor_context().position.column, pos_before.column + 2);

    // Undo "e"
    c.perform(Action::Undo);
    assert_eq!(
        c.cursor_context().position.column,
        pos_before.column + 1,
        "cursor should be after 'd'"
    );

    // Undo "d"
    c.perform(Action::Undo);
    assert_eq!(
        c.cursor_context().position.column,
        pos_before.column,
        "cursor should be back to original position"
    );
}

// ── Alignment round-trip ────────────────────────────────────────────────

#[test]
fn alignment_undo_redo() {
    let c = content("hello");

    assert_eq!(c.cursor_context().paragraph.alignment, Alignment::Left);

    // Set center
    c.perform(Action::SelectAll);
    c.perform(Format::SetAlignment(Alignment::Center));
    assert_eq!(c.cursor_context().paragraph.alignment, Alignment::Center);

    // Undo → left
    c.perform(Action::Undo);
    assert_eq!(
        c.cursor_context().paragraph.alignment,
        Alignment::Left,
        "undo should restore left"
    );

    // Redo → center
    c.perform(Action::Redo);
    assert_eq!(
        c.cursor_context().paragraph.alignment,
        Alignment::Center,
        "redo should restore center"
    );

    // Undo again → left
    c.perform(Action::Undo);
    assert_eq!(
        c.cursor_context().paragraph.alignment,
        Alignment::Left,
        "second undo should restore left again"
    );
}
