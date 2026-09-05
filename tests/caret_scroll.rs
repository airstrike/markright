//! A caret scrolled out of view must stay out of view — not snap to the
//! buffer origin because its line has no layout run to measure against.

use markright::widget::rich_editor::{Action, Content};

type C = Content<iced::Renderer>;

const BOUNDS: iced::Size = iced::Size::new(400.0, 100.0);

fn numbered_lines(count: usize) -> String {
    (0..count)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn caret_scrolled_out_of_view_stays_off_screen() {
    let c = C::with_text(&numbered_lines(60));
    c.update_layout(BOUNDS);

    let before = c.caret_rect().expect("caret");
    assert!(
        before.y >= 0.0 && before.y < BOUNDS.height,
        "caret should start inside the viewport: {before:?}",
    );

    // The cursor stays on line 0 while the view scrolls far past it.
    c.perform(Action::Scroll { pixels: 2000.0 });
    c.update_layout(BOUNDS);

    let after = c.caret_rect().expect("caret");
    assert!(
        after.y + after.height <= 0.0,
        "caret should sit above the viewport, not at the origin: {after:?}",
    );
}
