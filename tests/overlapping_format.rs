//! Integration tests for overlapping formatting operations, cursor font/size
//! inspection, and undo/redo of overlapping format ranges.

use markright::widget::rich_editor::{Action, Content, Format, Motion};

use iced::Font;

type C = Content<iced::Renderer>;

fn content(text: &str) -> C {
    C::with_text(text)
}

/// Select a byte range on a single line: move Home, right `start` times,
/// then select right `len` times.
fn select_range(c: &C, start: usize, len: usize) {
    c.perform(Action::Move(Motion::Home));
    for _ in 0..start {
        c.perform(Action::Move(Motion::Right));
    }
    for _ in 0..len {
        c.perform(Action::Select(Motion::Right));
    }
}

/// Read the character style at byte index `idx` on line 0 via cursor bias-left.
///
/// Moves the cursor to column `idx + 1` so that bias-left reads the style of
/// the character at index `idx`.
fn char_style_at(c: &C, idx: usize) -> markright::widget::rich_editor::cursor::Character {
    c.perform(Action::Move(Motion::Home));
    for _ in 0..=idx {
        c.perform(Action::Move(Motion::Right));
    }
    c.cursor_context().character
}

// ── Cursor font/size inspection ─────────────────────────────────────────

#[test]
fn cursor_reports_font_after_set_font() {
    let c = content("hello");

    c.perform(Action::SelectAll);
    c.perform(Format::SetFont(Font::new("Fira Code")));

    // Move cursor to middle of text (Home + 3 rights)
    c.perform(Action::Move(Motion::Home));
    for _ in 0..3 {
        c.perform(Action::Move(Motion::Right));
    }

    assert_eq!(
        c.cursor_context().character.font,
        Some(Font::new("Fira Code")),
        "cursor should report Fira Code after SetFont on entire text"
    );
}

#[test]
fn cursor_reports_size_after_set_size() {
    let c = content("hello");

    c.perform(Action::SelectAll);
    c.perform(Format::SetFontSize(24.0));

    // Move cursor to middle of text
    c.perform(Action::Move(Motion::Home));
    for _ in 0..3 {
        c.perform(Action::Move(Motion::Right));
    }

    assert_eq!(
        c.cursor_context().character.size,
        Some(24.0),
        "cursor should report size 24.0 after SetFontSize on entire text"
    );
}

#[test]
fn cursor_font_at_format_boundary() {
    let c = content("hello world");

    // Select "hello" (chars 0..5) and set font
    select_range(&c, 0, 5);
    c.perform(Format::SetFont(Font::new("Fira Code")));

    // Position 4 (inside "hello"): char_style_at reads char at index 4 ('o')
    let style_inside = char_style_at(&c, 4);
    assert_eq!(
        style_inside.font,
        Some(Font::new("Fira Code")),
        "char inside formatted range should have Fira Code"
    );

    // Position 5 (the space after "hello"): NOT in the formatted range
    let style_outside = char_style_at(&c, 5);
    assert_eq!(
        style_outside.font, None,
        "char outside formatted range should have no font (None)"
    );
}

// ── Overlapping bold + italic ───────────────────────────────────────────

#[test]
fn overlapping_bold_then_italic_creates_correct_runs() {
    let c = content("hello world");

    // Bold "hello" (chars 0..5)
    select_range(&c, 0, 5);
    c.perform(Format::ToggleBold);

    // Italic "lo wo" (chars 3..8)
    select_range(&c, 3, 5);
    c.perform(Format::ToggleItalic);

    // Check individual character styles at representative positions:
    // pos 1 ("e"): bold, not italic
    let s = char_style_at(&c, 1);
    assert!(s.bold, "pos 1 should be bold");
    assert!(!s.italic, "pos 1 should not be italic");

    // pos 4 ("o"): bold AND italic (overlap region)
    let s = char_style_at(&c, 4);
    assert!(s.bold, "pos 4 should be bold");
    assert!(s.italic, "pos 4 should be italic");

    // pos 6 ("w"): not bold, italic
    let s = char_style_at(&c, 6);
    assert!(!s.bold, "pos 6 should not be bold");
    assert!(s.italic, "pos 6 should be italic");

    // pos 9 ("l"): not bold, not italic
    let s = char_style_at(&c, 9);
    assert!(!s.bold, "pos 9 should not be bold");
    assert!(!s.italic, "pos 9 should not be italic");
}

#[test]
fn undo_overlapping_italic_preserves_bold() {
    let c = content("hello world");

    // Bold "hello" (chars 0..5)
    select_range(&c, 0, 5);
    c.perform(Format::ToggleBold);

    // Italic "lo wo" (chars 3..8)
    select_range(&c, 3, 5);
    c.perform(Format::ToggleItalic);

    // Undo the italic
    c.perform(Action::Undo);

    // pos 1 should still be bold
    let s = char_style_at(&c, 1);
    assert!(s.bold, "pos 1 should still be bold after undoing italic");

    // pos 4 should be bold, not italic
    let s = char_style_at(&c, 4);
    assert!(s.bold, "pos 4 should still be bold after undoing italic");
    assert!(!s.italic, "pos 4 should not be italic after undo");

    // pos 6 should be neither bold nor italic
    let s = char_style_at(&c, 6);
    assert!(!s.bold, "pos 6 should not be bold");
    assert!(!s.italic, "pos 6 should not be italic after undo");

    // Undo the bold
    c.perform(Action::Undo);

    // All positions should be default (no bold, no italic)
    for pos in [1, 4, 6, 9] {
        let s = char_style_at(&c, pos);
        assert!(!s.bold, "pos {} should not be bold after full undo", pos);
        assert!(
            !s.italic,
            "pos {} should not be italic after full undo",
            pos
        );
    }
}

#[test]
fn overlapping_bold_italic_full_undo_redo_cycle() {
    let c = content("abcdefgh");

    // Bold "abcd" (chars 0..4)
    select_range(&c, 0, 4);
    c.perform(Format::ToggleBold);

    // Italic "cdef" (chars 2..6)
    select_range(&c, 2, 4);
    c.perform(Format::ToggleItalic);

    // Verify initial state
    let s = char_style_at(&c, 1);
    assert!(s.bold, "pos 1 bold");
    assert!(!s.italic, "pos 1 not italic");

    let s = char_style_at(&c, 3);
    assert!(s.bold, "pos 3 bold");
    assert!(s.italic, "pos 3 italic");

    let s = char_style_at(&c, 5);
    assert!(!s.bold, "pos 5 not bold");
    assert!(s.italic, "pos 5 italic");

    let s = char_style_at(&c, 7);
    assert!(!s.bold, "pos 7 not bold");
    assert!(!s.italic, "pos 7 not italic");

    // Undo italic -> bold intact for 0..4
    c.perform(Action::Undo);
    let s = char_style_at(&c, 1);
    assert!(s.bold, "pos 1 bold after undo italic");
    assert!(!s.italic, "pos 1 not italic after undo italic");

    let s = char_style_at(&c, 3);
    assert!(s.bold, "pos 3 bold after undo italic");
    assert!(!s.italic, "pos 3 not italic after undo italic");

    let s = char_style_at(&c, 5);
    assert!(!s.bold, "pos 5 not bold after undo italic");
    assert!(!s.italic, "pos 5 not italic after undo italic");

    // Redo italic -> italic back
    c.perform(Action::Redo);
    let s = char_style_at(&c, 3);
    assert!(s.bold, "pos 3 bold after redo italic");
    assert!(s.italic, "pos 3 italic after redo italic");

    let s = char_style_at(&c, 5);
    assert!(s.italic, "pos 5 italic after redo italic");

    // Undo twice (italic then bold) -> everything default
    c.perform(Action::Undo);
    c.perform(Action::Undo);
    for pos in [1, 3, 5, 7] {
        let s = char_style_at(&c, pos);
        assert!(!s.bold, "pos {} not bold after full undo", pos);
        assert!(!s.italic, "pos {} not italic after full undo", pos);
    }

    // Redo twice -> both back
    c.perform(Action::Redo);
    c.perform(Action::Redo);

    let s = char_style_at(&c, 1);
    assert!(s.bold, "pos 1 bold after full redo");
    assert!(!s.italic, "pos 1 not italic after full redo");

    let s = char_style_at(&c, 3);
    assert!(s.bold, "pos 3 bold after full redo");
    assert!(s.italic, "pos 3 italic after full redo");

    let s = char_style_at(&c, 5);
    assert!(!s.bold, "pos 5 not bold after full redo");
    assert!(s.italic, "pos 5 italic after full redo");

    let s = char_style_at(&c, 7);
    assert!(!s.bold, "pos 7 not bold after full redo");
    assert!(!s.italic, "pos 7 not italic after full redo");
}

// ── Font + bold preservation ────────────────────────────────────────────

#[test]
fn set_font_preserves_existing_bold() {
    let c = content("hello");

    // Bold everything
    c.perform(Action::SelectAll);
    c.perform(Format::ToggleBold);

    // Set font on everything
    c.perform(Action::SelectAll);
    c.perform(Format::SetFont(Font::new("Fira Code")));

    // Check styled runs: should have both bold and font
    let styled = c.styled_line(0).expect("line 0 should exist");
    for run in &styled.runs {
        assert!(
            run.style.bold.unwrap_or(false),
            "run {:?} should be bold after SetFont",
            run.range
        );
        assert_eq!(
            run.style.font,
            Some(Font::new("Fira Code")),
            "run {:?} should have Fira Code",
            run.range
        );
    }

    // Cursor context should also report both
    let ctx = c.cursor_context();
    assert!(ctx.character.bold, "cursor should report bold");
    assert_eq!(
        ctx.character.font,
        Some(Font::new("Fira Code")),
        "cursor should report Fira Code"
    );
}

#[test]
fn set_bold_preserves_existing_font() {
    let c = content("hello");

    // Set font on everything
    c.perform(Action::SelectAll);
    c.perform(Format::SetFont(Font::new("Fira Code")));

    // Bold everything
    c.perform(Action::SelectAll);
    c.perform(Format::ToggleBold);

    // Check styled runs: should have both font and bold
    let styled = c.styled_line(0).expect("line 0 should exist");
    for run in &styled.runs {
        assert_eq!(
            run.style.font,
            Some(Font::new("Fira Code")),
            "run {:?} should have Fira Code after ToggleBold",
            run.range
        );
        assert!(
            run.style.bold.unwrap_or(false),
            "run {:?} should be bold",
            run.range
        );
    }
}

// ── Three overlapping formats ───────────────────────────────────────────

#[test]
fn three_overlapping_formats_undo_all() {
    let c = content("abcdefghij");

    // Bold "abcd" (chars 0..4)
    select_range(&c, 0, 4);
    c.perform(Format::ToggleBold);

    // Italic "defg" (chars 3..7)
    select_range(&c, 3, 4);
    c.perform(Format::ToggleItalic);

    // Underline "ghij" (chars 6..10)
    select_range(&c, 6, 4);
    c.perform(Format::ToggleUnderline);

    // Verify initial state at various positions
    // pos 1 ("b"): bold only
    let s = char_style_at(&c, 1);
    assert!(s.bold, "pos 1 should be bold");
    assert!(!s.italic, "pos 1 should not be italic");
    assert!(!s.underline, "pos 1 should not be underlined");

    // pos 3 ("d"): bold + italic (overlap of bold and italic)
    let s = char_style_at(&c, 3);
    assert!(s.bold, "pos 3 should be bold");
    assert!(s.italic, "pos 3 should be italic");
    assert!(!s.underline, "pos 3 should not be underlined");

    // pos 5 ("f"): italic only
    let s = char_style_at(&c, 5);
    assert!(!s.bold, "pos 5 should not be bold");
    assert!(s.italic, "pos 5 should be italic");
    assert!(!s.underline, "pos 5 should not be underlined");

    // pos 6 ("g"): italic + underline (overlap of italic and underline)
    let s = char_style_at(&c, 6);
    assert!(!s.bold, "pos 6 should not be bold");
    assert!(s.italic, "pos 6 should be italic");
    assert!(s.underline, "pos 6 should be underlined");

    // pos 8 ("i"): underline only
    let s = char_style_at(&c, 8);
    assert!(!s.bold, "pos 8 should not be bold");
    assert!(!s.italic, "pos 8 should not be italic");
    assert!(s.underline, "pos 8 should be underlined");

    // Undo underline
    c.perform(Action::Undo);
    let s = char_style_at(&c, 8);
    assert!(!s.underline, "pos 8 should lose underline after undo");
    // Rest should be unchanged
    let s = char_style_at(&c, 1);
    assert!(s.bold, "pos 1 still bold after undo underline");
    let s = char_style_at(&c, 5);
    assert!(s.italic, "pos 5 still italic after undo underline");

    // Undo italic
    c.perform(Action::Undo);
    let s = char_style_at(&c, 5);
    assert!(!s.italic, "pos 5 should lose italic after undo");
    let s = char_style_at(&c, 3);
    assert!(s.bold, "pos 3 still bold after undo italic");
    assert!(!s.italic, "pos 3 should lose italic after undo");

    // Undo bold
    c.perform(Action::Undo);
    for pos in [1, 3, 5, 8] {
        let s = char_style_at(&c, pos);
        assert!(!s.bold, "pos {} should not be bold after full undo", pos);
        assert!(
            !s.italic,
            "pos {} should not be italic after full undo",
            pos
        );
        assert!(
            !s.underline,
            "pos {} should not be underlined after full undo",
            pos
        );
    }
}

// ── Overlapping font + bold ─────────────────────────────────────────────

#[test]
fn overlapping_font_and_bold_undo() {
    let c = content("hello world");

    // SetFont on "hello" (chars 0..5)
    select_range(&c, 0, 5);
    c.perform(Format::SetFont(Font::new("Fira Code")));

    // Bold "lo wo" (chars 3..8)
    select_range(&c, 3, 5);
    c.perform(Format::ToggleBold);

    // pos 1 ("e"): Fira Code, not bold
    let s = char_style_at(&c, 1);
    assert_eq!(
        s.font,
        Some(Font::new("Fira Code")),
        "pos 1 should have Fira Code"
    );
    assert!(!s.bold, "pos 1 should not be bold");

    // pos 4 ("o"): Fira Code AND bold
    let s = char_style_at(&c, 4);
    assert_eq!(
        s.font,
        Some(Font::new("Fira Code")),
        "pos 4 should have Fira Code"
    );
    assert!(s.bold, "pos 4 should be bold");

    // pos 6 ("w"): no Fira Code, bold
    let s = char_style_at(&c, 6);
    assert_eq!(s.font, None, "pos 6 should not have Fira Code");
    assert!(s.bold, "pos 6 should be bold");

    // Undo bold
    c.perform(Action::Undo);

    // pos 4 should have Fira Code, no bold
    let s = char_style_at(&c, 4);
    assert_eq!(
        s.font,
        Some(Font::new("Fira Code")),
        "pos 4 should still have Fira Code after undo bold"
    );
    assert!(!s.bold, "pos 4 should not be bold after undo");

    // pos 6 should have neither
    let s = char_style_at(&c, 6);
    assert!(!s.bold, "pos 6 should not be bold after undo");

    // Undo font
    c.perform(Action::Undo);

    // Everything should be default
    for pos in [1, 4, 6] {
        let s = char_style_at(&c, pos);
        assert_eq!(
            s.font, None,
            "pos {} should have no font after full undo",
            pos
        );
        assert!(!s.bold, "pos {} should not be bold after full undo", pos);
    }
}
