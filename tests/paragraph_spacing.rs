//! Integration tests for paragraph spacing (space_before / spacing_after).
//!
//! Verifies that cosmic-text's per-BufferLine margin_top/margin_bottom are
//! wired correctly through Content, producing visible vertical gaps between
//! paragraphs.

use markright::widget::rich_editor::{Content, Edit, Format};
use markright_core::Name;
use markright_document::format as mr;

type C = Content<iced::Renderer>;

const BOUNDS: iced::Size = iced::Size::new(800.0, 600.0);

fn content(text: &str) -> C {
    let c = C::with_text(text);
    c.update_layout(BOUNDS);
    c
}

fn from_mr(input: &str) -> C {
    let c = C::from_styled_lines(&mr::parse(input).expect("parse failed"));
    c.update_layout(BOUNDS);
    c
}

#[test]
fn no_spacing_lines_are_contiguous() {
    let c = content("aaa\nbbb\nccc");

    let g0 = c.line_geometry(0).expect("line 0");
    let g1 = c.line_geometry(1).expect("line 1");

    let expected_top = g0.line_top + g0.line_height;
    assert!(
        (g1.line_top - expected_top).abs() < 0.5,
        "without spacing, line 1 should start right after line 0: \
         expected ~{expected_top}, got {}",
        g1.line_top,
    );
}

#[test]
fn spacing_after_shifts_next_line_down() {
    // Line 0 has sa=20, so line 1 should be shifted down by ~20px.
    let c = from_mr(">|sa=20|\naaa\nbbb\nccc");

    let g0 = c.line_geometry(0).expect("line 0");
    let g1 = c.line_geometry(1).expect("line 1");

    let expected_top = g0.line_top + g0.line_height + 20.0;
    assert!(
        (g1.line_top - expected_top).abs() < 0.5,
        "spacing_after=20 on line 0 should push line 1 down: \
         expected ~{expected_top}, got {}",
        g1.line_top,
    );
}

#[test]
fn space_before_shifts_line_down() {
    // Line 1 has sb=20, so it should be shifted down by ~20px.
    let c = from_mr("aaa\n>|sb=20|\nbbb\nccc");

    let g0 = c.line_geometry(0).expect("line 0");
    let g1 = c.line_geometry(1).expect("line 1");

    let expected_top = g0.line_top + g0.line_height + 20.0;
    assert!(
        (g1.line_top - expected_top).abs() < 0.5,
        "space_before=20 on line 1 should push it down: \
         expected ~{expected_top}, got {}",
        g1.line_top,
    );
}

#[test]
fn spacing_after_and_space_before_stack() {
    // Line 0 sa=10, line 1 sb=20 → total gap of 30px.
    let c = from_mr(">|sa=10|\naaa\n>|sb=20|\nbbb\nccc");

    let g0 = c.line_geometry(0).expect("line 0");
    let g1 = c.line_geometry(1).expect("line 1");

    let expected_top = g0.line_top + g0.line_height + 30.0;
    assert!(
        (g1.line_top - expected_top).abs() < 0.5,
        "sa=10 + sb=20 should stack to 30px gap: \
         expected ~{expected_top}, got {}",
        g1.line_top,
    );
}

#[test]
fn set_name_heading_applies_theme_spacing() {
    let c = content("aaa\nbbb\nccc");

    // Baseline: no spacing
    let g1_before = c.line_geometry(1).expect("line 1 before");

    // Set line 0 to HEADING_1 (theme: sa=12)
    c.perform(Format::SetName(Name::HEADING_1));
    c.update_layout(BOUNDS);

    let g0 = c.line_geometry(0).expect("line 0");
    let g1_after = c.line_geometry(1).expect("line 1 after");

    // Line 1 should have moved down due to heading's spacing_after
    assert!(
        g1_after.line_top > g1_before.line_top,
        "heading spacing_after should push line 1 down: \
         before={}, after={}",
        g1_before.line_top,
        g1_after.line_top,
    );

    // Also verify the actual gap includes the theme sa value
    let gap = g1_after.line_top - (g0.line_top + g0.line_height);
    assert!(
        gap > 5.0,
        "heading should produce visible spacing_after gap: got {gap}",
    );
}

#[test]
fn set_space_before_via_format_action() {
    let c = content("aaa\nbbb\nccc");

    // Move to line 1
    c.perform(Edit::Enter { inherit: false });
    c.perform(Format::SetSpaceBefore(Some(25.0)));
    c.update_layout(BOUNDS);

    let g0 = c.line_geometry(0).expect("line 0");
    let g1 = c.line_geometry(1).expect("line 1");

    let gap = g1.line_top - (g0.line_top + g0.line_height);
    assert!(
        (gap - 25.0).abs() < 0.5,
        "SetSpaceBefore(25) should produce 25px gap: got {gap}",
    );
}

#[test]
fn spacing_survives_enter_split() {
    // Start with a heading (has theme spacing), press Enter in middle.
    // Both halves should keep the heading's spacing.
    let c = content("abcdef");
    c.perform(Format::SetName(Name::HEADING_2));
    c.update_layout(BOUNDS);

    let ctx = c.cursor_context();
    let sa = ctx.paragraph.style.spacing_after;

    // Move to middle, split
    c.perform(markright::widget::rich_editor::Action::Move(
        markright::widget::rich_editor::Motion::Right,
    ));
    c.perform(markright::widget::rich_editor::Action::Move(
        markright::widget::rich_editor::Motion::Right,
    ));
    c.perform(markright::widget::rich_editor::Action::Move(
        markright::widget::rich_editor::Motion::Right,
    ));
    c.perform(Edit::Enter { inherit: true });
    c.update_layout(BOUNDS);

    // Line 1 should still be a heading with spacing
    let ctx1 = c.cursor_context();
    assert_eq!(ctx1.paragraph.name, Name::HEADING_2);
    assert_eq!(ctx1.paragraph.style.spacing_after, sa);
}

#[test]
fn spacing_cleared_on_heading_demotion() {
    // Heading at end-of-line → Enter demotes to BODY.
    // The new BODY line should have BODY's default spacing, not heading's.
    let c = content("hello");
    c.perform(Format::SetName(Name::HEADING_1));
    c.update_layout(BOUNDS);

    // Move to end of line, press Enter (no shift → demote)
    c.perform(markright::widget::rich_editor::Action::Move(
        markright::widget::rich_editor::Motion::End,
    ));
    c.perform(Edit::Enter { inherit: false });
    c.update_layout(BOUNDS);

    let ctx = c.cursor_context();
    assert_eq!(ctx.paragraph.name, Name::BODY, "should demote to BODY");

    // BODY's theme spacing_after is 8.0
    let sa = ctx.paragraph.style.spacing_after;
    assert!(
        sa.is_none() || sa == Some(8.0),
        "BODY spacing_after should be theme default (None or 8.0), got {sa:?}",
    );
}

#[test]
fn merge_preserves_surviving_line_spacing() {
    // Two lines with different spacing. Backspace on line 1 merges into line 0.
    // Line 0's spacing should survive.
    let c = from_mr(">|sa=15|\nfirst\n>|sb=25|\nsecond");

    let _g0_before = c.line_geometry(0).expect("line 0 before");

    // Move to start of line 1
    c.perform(markright::widget::rich_editor::Action::Move(
        markright::widget::rich_editor::Motion::Down,
    ));
    c.perform(markright::widget::rich_editor::Action::Move(
        markright::widget::rich_editor::Motion::Home,
    ));
    c.perform(Edit::Backspace);
    c.update_layout(BOUNDS);

    // After merge, line 0 (now "firstsecond") should still have sa=15
    let ctx = c.cursor_context();
    assert_eq!(
        ctx.paragraph.style.spacing_after,
        Some(15.0),
        "surviving line should keep its spacing_after"
    );
}
