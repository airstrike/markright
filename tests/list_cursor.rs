//! Tests for list bullet rendering and cursor position with indented lines.
//!
//! These tests exercise the full Content → Editor → cosmic-text pipeline to
//! verify that toggling a bullet list, pressing Enter, and typing all produce
//! correct paragraph styles, margins, and cursor positions.

use markright::paragraph;
use markright::widget::rich_editor::list;
use markright::widget::rich_editor::{Content, Edit, Format};

type C = Content<iced::Renderer>;

const BOUNDS: iced::Size = iced::Size::new(800.0, 600.0);

fn content(text: &str) -> C {
    let c = C::with_text(text);
    c.update_layout(BOUNDS);
    c
}

fn set_bullet() -> Format {
    Format::SetList(Some(paragraph::List::Bullet(paragraph::Bullet::Disc)))
}

#[test]
fn toggle_bullet_on_empty_line_sets_level_and_list() {
    let c = content("");

    let ctx = c.cursor_context();
    assert!(ctx.paragraph.style.list.is_none(), "initially no list");
    assert_eq!(ctx.paragraph.style.level, 0, "initially level 0");

    c.perform(set_bullet());
    c.update_layout(BOUNDS);

    let ctx = c.cursor_context();
    assert!(
        matches!(ctx.paragraph.style.list, Some(paragraph::List::Bullet(_))),
        "should have bullet list after toggle"
    );
    assert_eq!(
        ctx.paragraph.style.level, 1,
        "should be level 1 after toggle"
    );

    let expected_margin = 2.0 * c.list_indent();
    let actual_margin = list::compute_margin(&ctx.paragraph.style, c.list_indent());
    assert_eq!(
        actual_margin, expected_margin,
        "margin should be 2*list_indent for level-1 bullet"
    );
}

#[test]
fn toggle_bullet_moves_cursor_x() {
    let c = content("");
    c.update_layout(BOUNDS);

    let before = c.caret_rect().expect("should have caret");
    let x_before = before.x;

    c.perform(set_bullet());
    c.update_layout(BOUNDS);

    let after = c.caret_rect().expect("should have caret after toggle");
    let x_after = after.x;

    assert!(
        x_after > x_before,
        "cursor x should move right after toggling bullet (before={x_before}, after={x_after})"
    );
}

#[test]
fn empty_bullet_line_has_geometry() {
    let c = content("");
    c.perform(set_bullet());
    c.update_layout(BOUNDS);

    let geom = c.line_geometry(0);
    assert!(
        geom.is_some(),
        "empty bullet line must have line_geometry for marker to draw"
    );
}

#[test]
fn type_after_bullet_toggle_preserves_list() {
    let c = content("");
    c.perform(set_bullet());
    c.perform(Edit::Insert('a'));
    c.update_layout(BOUNDS);

    let ctx = c.cursor_context();
    assert!(
        matches!(ctx.paragraph.style.list, Some(paragraph::List::Bullet(_))),
        "list should persist after typing"
    );
    assert_eq!(ctx.paragraph.style.level, 1);
    assert_eq!(ctx.position.column, 1, "cursor should be after 'a'");
}

#[test]
fn enter_creates_new_bullet_paragraph() {
    let c = content("");
    c.perform(set_bullet());
    c.perform(Edit::Insert('a'));
    c.perform(Edit::Enter);
    c.update_layout(BOUNDS);

    assert_eq!(c.line_count(), 2, "should have 2 lines after Enter");

    let ctx = c.cursor_context();
    assert_eq!(ctx.position.line, 1, "cursor should be on new line");
    assert_eq!(ctx.position.column, 0, "cursor should be at column 0");

    assert!(
        matches!(ctx.paragraph.style.list, Some(paragraph::List::Bullet(_))),
        "new line should have bullet list"
    );
    assert_eq!(ctx.paragraph.style.level, 1, "new line should be level 1");

    let geom = c.line_geometry(1);
    assert!(
        geom.is_some(),
        "new empty bullet line must have line_geometry for marker to draw"
    );
}

#[test]
fn new_bullet_line_x_offset_matches_original() {
    let c = content("");
    c.perform(set_bullet());
    c.perform(Edit::Insert('a'));
    c.update_layout(BOUNDS);

    let geom0 = c.line_geometry(0).expect("line 0 should have geometry");

    c.perform(Edit::Enter);
    c.update_layout(BOUNDS);

    let geom1 = c.line_geometry(1).expect("line 1 should have geometry");

    assert_eq!(
        geom0.x_offset, geom1.x_offset,
        "new bullet line x_offset ({}) should match original ({})",
        geom1.x_offset, geom0.x_offset
    );
}

#[test]
fn cursor_x_on_new_bullet_line_reflects_margin() {
    let c = content("");
    c.update_layout(BOUNDS);

    let plain_caret = c.caret_rect().expect("should have caret");
    let x_plain = plain_caret.x;

    c.perform(set_bullet());
    c.perform(Edit::Insert('a'));
    c.perform(Edit::Enter);
    c.update_layout(BOUNDS);

    let ctx = c.cursor_context();
    assert_eq!(ctx.position.line, 1);
    assert_eq!(ctx.position.column, 0);

    let caret = c
        .caret_rect()
        .expect("should have caret on new bullet line");
    let x_bullet = caret.x;

    assert!(
        x_bullet > x_plain,
        "cursor x on empty bullet line ({x_bullet}) should be > plain line ({x_plain})"
    );
}

#[test]
fn enter_on_right_aligned_bullet_preserves_alignment() {
    use markright::widget::rich_editor::Alignment;

    let c = content("");
    c.perform(set_bullet());
    c.perform(Format::SetAlignment(Alignment::Right));
    c.perform(Edit::Insert('A'));
    c.perform(Edit::Insert('p'));
    c.perform(Edit::Insert('p'));
    c.perform(Edit::Insert('l'));
    c.perform(Edit::Insert('e'));
    c.update_layout(BOUNDS);

    c.perform(Edit::Enter);
    c.perform(Edit::Insert('B'));
    c.perform(Edit::Insert('a'));
    c.perform(Edit::Insert('n'));
    c.perform(Edit::Insert('a'));
    c.perform(Edit::Insert('n'));
    c.perform(Edit::Insert('a'));
    c.update_layout(BOUNDS);

    let geom1 = c.line_geometry(1).expect("line 1 geometry");

    // Both lines are right-aligned bullets — x_offset should be well past the
    // margin (40px), near the right edge. Without the alignment fix, the new
    // line would have x_offset ≈ 40 (left-aligned).
    let margin = list::compute_margin(&c.cursor_context().paragraph.style, c.list_indent());
    assert!(
        geom1.x_offset > margin * 2.0,
        "new line should be right-aligned (x_offset={} should be >> margin={})",
        geom1.x_offset,
        margin
    );

    let ctx = c.cursor_context();
    assert!(
        matches!(ctx.paragraph.style.list, Some(paragraph::List::Bullet(_))),
        "new line should still have bullet list"
    );
    assert_eq!(
        ctx.paragraph.alignment,
        Alignment::Right,
        "new line should be right-aligned"
    );
}

#[test]
fn toggle_bullet_off_at_any_level() {
    let c = content("");
    c.perform(set_bullet());

    c.perform(Format::IndentList);
    let ctx = c.cursor_context();
    assert_eq!(ctx.paragraph.style.level, 2);
    assert!(matches!(
        ctx.paragraph.style.list,
        Some(paragraph::List::Bullet(paragraph::Bullet::Circle))
    ));

    // Click bullet button again — should toggle OFF, not change variant.
    c.perform(set_bullet());

    let ctx = c.cursor_context();
    assert!(
        ctx.paragraph.style.list.is_none(),
        "bullet should be removed when toggling same kind at any level"
    );
    assert_eq!(
        ctx.paragraph.style.level, 0,
        "level should reset to 0 when bullet is removed"
    );
}
