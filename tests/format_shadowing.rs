//! End-to-end invariant: a span attribute set to a value it already has
//! must never end up shadowing later changes to the line's default style
//! (e.g. from a paragraph name change). Two layers defend this — markright
//! skips no-op span writes, and iced migrates span attrs that match the
//! old defaults when the paragraph style changes.

use markright::widget::rich_editor::{Action, Content, Format};
use markright_core::Name;

type C = Content<iced::Renderer>;

#[test]
fn noop_attr_write_does_not_shadow_paragraph_defaults() {
    let c = C::with_text("hello world");

    // A color reset that changes nothing — no character has a color.
    c.perform(Action::SelectAll);
    c.perform(Format::SetColor(None));

    // Switch the paragraph to a heading, whose character default is bold.
    c.perform(Format::SetName(Name::HEADING_1));

    // Bold must come from the paragraph defaults; a no-op write would have
    // frozen the pre-heading defaults into explicit spans that now read
    // back as bold overrides.
    let styled = c.styled_line(0).expect("line 0");
    assert!(
        styled.runs.iter().all(|r| r.style.bold.is_none()),
        "no-op attr write created explicit spans that shadow the heading's \
         bold default: {:?}",
        styled.runs,
    );
}
