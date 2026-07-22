//! Undo of a multi-line selection delete must restore paragraph identity —
//! names, styles, and spacing — not just text and span styles.

use markright::widget::rich_editor::{Action, Content, Edit, Motion};
use markright_core::Name;
use markright_document::format;

type C = Content<iced::Renderer>;

const BOUNDS: iced::Size = iced::Size::new(800.0, 600.0);

fn from_mr(input: &str) -> C {
    let c = C::from_styled_lines(&format::parse(input).expect("parse failed"));
    c.update_layout(BOUNDS);
    c
}

/// Heading, code block, body — three distinct paragraph identities.
const DOC: &str = concat!(
    ">|name=heading-1|\nTitle\n",
    ">|name=code-block|\nfn main() {}\n",
    "body text",
);

fn names(c: &C) -> Vec<Name> {
    (0..c.line_count())
        .map(|i| c.styled_line(i).expect("styled line").paragraph.name)
        .collect()
}

#[test]
fn undo_select_all_delete_restores_paragraph_names() {
    let c = from_mr(DOC);
    assert_eq!(names(&c), [Name::HEADING_1, Name::CODE_BLOCK, Name::BODY]);

    c.perform(Action::SelectAll);
    c.perform(Action::Edit(Edit::Backspace));
    assert_eq!(c.line_count(), 1, "delete should leave a single empty line");

    c.perform(Action::Undo);
    assert_eq!(c.line_count(), 3, "undo should restore all lines");
    assert_eq!(
        c.line(0).map(|l| l.text.to_string()),
        Some("Title".to_string())
    );
    assert_eq!(names(&c), [Name::HEADING_1, Name::CODE_BLOCK, Name::BODY]);
}

#[test]
fn undo_select_all_delete_restores_paragraph_spacing() {
    let c = from_mr(DOC);
    let spacing = |c: &C| -> Vec<Option<f32>> {
        (0..c.line_count())
            .map(|i| {
                c.styled_line(i)
                    .expect("styled line")
                    .paragraph
                    .style
                    .spacing_after
            })
            .collect()
    };
    let before = spacing(&c);
    assert_eq!(before[0], Some(12.0), "H1 spacing_after from theme");

    c.perform(Action::SelectAll);
    c.perform(Action::Edit(Edit::Backspace));
    c.perform(Action::Undo);

    assert_eq!(spacing(&c), before);
}

#[test]
fn undo_partial_multi_line_delete_restores_names() {
    let c = from_mr(DOC);

    // Select from the middle of the heading to the end of the document.
    c.move_to(0, 2);
    c.perform(Action::Select(Motion::DocumentEnd));
    c.perform(Action::Edit(Edit::Backspace));
    assert_eq!(c.line_count(), 1);

    c.perform(Action::Undo);
    assert_eq!(names(&c), [Name::HEADING_1, Name::CODE_BLOCK, Name::BODY]);
    assert_eq!(
        c.line(2).map(|l| l.text.to_string()),
        Some("body text".to_string())
    );
}

#[test]
fn redo_then_undo_again_keeps_paragraph_identity() {
    let c = from_mr(DOC);

    c.perform(Action::SelectAll);
    c.perform(Action::Edit(Edit::Backspace));
    c.perform(Action::Undo);
    c.perform(Action::Redo);
    assert_eq!(c.line_count(), 1, "redo should re-apply the delete");

    c.perform(Action::Undo);
    assert_eq!(names(&c), [Name::HEADING_1, Name::CODE_BLOCK, Name::BODY]);
}
