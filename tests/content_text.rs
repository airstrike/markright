use markright::widget::rich_editor::{Action, Content, Format};

type C = Content<iced::Renderer>;

#[test]
fn text_returns_raw_content() {
    let c = C::with_text("{=1+2}");
    assert_eq!(c.text(), "{=1+2}");
}

#[test]
fn text_unaffected_by_bold() {
    let c = C::with_text("{=1+2}");
    c.perform(Action::SelectAll);
    c.perform(Format::ToggleBold);
    assert_eq!(c.text(), "{=1+2}");
}

#[test]
fn text_with_braces_and_backslashes() {
    let c = C::with_text("{hello} \\world");
    assert_eq!(c.text(), "{hello} \\world");
}

#[test]
fn text_multiline() {
    let c = C::with_text("line one\nline two\nline three");
    assert_eq!(c.text(), "line one\nline two\nline three");
}
