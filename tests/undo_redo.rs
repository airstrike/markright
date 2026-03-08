//! Integration tests for undo/redo through Content::perform — the same
//! code path used by the GUI.

use markright::widget::rich_editor::{Action, Alignment, Content, Edit, FormatAction};

const SAMPLE: &str = include_str!("../examples/editor/sample.txt");

type C = Content<iced::Renderer>;

fn content(text: &str) -> C {
    C::with_text(text)
}

fn fmt(f: FormatAction) -> Action {
    Action::Edit(Edit::Format(f))
}

#[test]
fn select_all_center_align_then_undo() {
    let c = content(SAMPLE);

    c.perform(Action::SelectAll);
    c.perform(fmt(FormatAction::SetAlignment(Alignment::Center)));

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
    c.perform(fmt(FormatAction::SetAlignment(Alignment::Center)));

    // Left-align
    c.perform(Action::SelectAll);
    c.perform(fmt(FormatAction::SetAlignment(Alignment::Left)));

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
