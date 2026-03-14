//! Tests for list bullet rendering and cursor position with indented lines.
//!
//! These tests exercise the full Content → Editor → cosmic-text pipeline to
//! verify that toggling a bullet list, pressing Enter, and typing all produce
//! correct paragraph styles, margins, and cursor positions.

use markright::paragraph;
use markright::widget::rich_editor::list;
use markright::widget::rich_editor::{Action, Content, Edit, FormatAction};

type C = Content<iced::Renderer>;

const BOUNDS: iced::Size = iced::Size::new(800.0, 600.0);

fn content(text: &str) -> C {
    let c = C::with_text(text);
    c.update_layout(BOUNDS);
    c
}

fn fmt(f: FormatAction) -> Action {
    Action::Edit(Edit::Format(f))
}

fn set_bullet() -> Action {
    fmt(FormatAction::SetList(Some(paragraph::List::Bullet(
        paragraph::Bullet::Disc,
    ))))
}

// -----------------------------------------------------------------------
// Step 1–3: blank doc → toggle bullets → cursor x should have moved
// -----------------------------------------------------------------------

#[test]
fn toggle_bullet_on_empty_line_sets_level_and_list() {
    let c = content("");

    // Step 2: note initial state — no list, level 0.
    let ctx = c.cursor_context();
    assert!(ctx.paragraph.style.list.is_none(), "initially no list");
    assert_eq!(ctx.paragraph.style.level, 0, "initially level 0");

    // Step 3: toggle bullets on.
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

    // The margin should be hanging + 1*indent = 2*DEFAULT_LIST_INDENT = 40.
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

    // Step 2: note initial cursor x position.
    let before = c.caret_rect().expect("should have caret");
    let x_before = before.x;

    // Step 3: toggle bullets on — cursor x should move right.
    c.perform(set_bullet());
    c.update_layout(BOUNDS);

    let after = c.caret_rect().expect("should have caret after toggle");
    let x_after = after.x;

    assert!(
        x_after > x_before,
        "step 3: cursor x should move right after toggling bullet (before={x_before}, after={x_after})"
    );
}

// -----------------------------------------------------------------------
// Step 4: bullet should draw on empty line (line_geometry must return Some)
// -----------------------------------------------------------------------

#[test]
fn empty_bullet_line_has_geometry() {
    let c = content("");
    c.perform(set_bullet());
    c.update_layout(BOUNDS);

    // line_geometry must return Some for the bullet marker to draw.
    let geom = c.line_geometry(0);
    assert!(
        geom.is_some(),
        "step 4: empty bullet line must have line_geometry for marker to draw"
    );
}

// -----------------------------------------------------------------------
// Step 5: type 'a' — cursor moves correctly, bullet draws.
// -----------------------------------------------------------------------

#[test]
fn type_after_bullet_toggle_preserves_list() {
    let c = content("");
    c.perform(set_bullet());
    c.perform(Action::Edit(Edit::Insert('a')));
    c.update_layout(BOUNDS);

    let ctx = c.cursor_context();
    assert!(
        matches!(ctx.paragraph.style.list, Some(paragraph::List::Bullet(_))),
        "list should persist after typing"
    );
    assert_eq!(ctx.paragraph.style.level, 1);
    assert_eq!(ctx.position.column, 1, "cursor should be after 'a'");
}

// -----------------------------------------------------------------------
// Step 6: hit Enter — should create new bullet paragraph.
// -----------------------------------------------------------------------

#[test]
fn enter_creates_new_bullet_paragraph() {
    let c = content("");
    c.perform(set_bullet());
    c.perform(Action::Edit(Edit::Insert('a')));
    c.perform(Action::Edit(Edit::Enter));
    c.update_layout(BOUNDS);

    assert_eq!(c.line_count(), 2, "should have 2 lines after Enter");

    let ctx = c.cursor_context();
    assert_eq!(ctx.position.line, 1, "cursor should be on new line");
    assert_eq!(ctx.position.column, 0, "cursor should be at column 0");

    assert!(
        matches!(ctx.paragraph.style.list, Some(paragraph::List::Bullet(_))),
        "new line should have bullet list (step 6)"
    );
    assert_eq!(ctx.paragraph.style.level, 1, "new line should be level 1");

    // New empty bullet line should also have line geometry for the marker to draw.
    let geom = c.line_geometry(1);
    assert!(
        geom.is_some(),
        "step 6: new empty bullet line must have line_geometry for marker to draw"
    );
}

// -----------------------------------------------------------------------
// Step 7: cursor on new empty bullet line should NOT be at x=0.
// -----------------------------------------------------------------------

#[test]
fn cursor_x_on_new_bullet_line_reflects_margin() {
    let c = content("");
    c.update_layout(BOUNDS);

    // Baseline: cursor x on a plain empty line.
    let plain_caret = c.caret_rect().expect("should have caret");
    let x_plain = plain_caret.x;

    // Now make a bullet line, type, and Enter.
    c.perform(set_bullet());
    c.perform(Action::Edit(Edit::Insert('a')));
    c.perform(Action::Edit(Edit::Enter));
    c.update_layout(BOUNDS);

    // Cursor is now on line 1, col 0 — an empty bulleted line.
    let ctx = c.cursor_context();
    assert_eq!(ctx.position.line, 1);
    assert_eq!(ctx.position.column, 0);

    let caret = c
        .caret_rect()
        .expect("should have caret on new bullet line");
    let x_bullet = caret.x;

    assert!(
        x_bullet > x_plain,
        "step 7: cursor x on empty bullet line ({x_bullet}) should be > plain line ({x_plain})"
    );
}

// -----------------------------------------------------------------------
// Toggle off: clicking bullet button when already in a bullet list
// (regardless of bullet variant or level) should remove the list.
// -----------------------------------------------------------------------

#[test]
fn toggle_bullet_off_at_any_level() {
    let c = content("");
    c.perform(set_bullet());

    // Indent to level 2 (Circle variant).
    c.perform(fmt(FormatAction::IndentList));
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
