//! Tests for Content::set_color — setting a document-wide color should
//! strip all per-span colors so that the widget's .color() default takes over.

use iced::{Color, Size};
use markright::widget::rich_editor::{Action, Content, Format, Motion};

type C = Content<iced::Renderer>;

/// Select a byte range on line 0.
fn select_range(c: &C, start: usize, len: usize) {
    c.perform(Action::Move(Motion::Home));
    for _ in 0..start {
        c.perform(Action::Move(Motion::Right));
    }
    for _ in 0..len {
        c.perform(Action::Select(Motion::Right));
    }
}

const RED: Color = Color::from_rgb(1.0, 0.0, 0.0);
const BLUE: Color = Color::from_rgb(0.0, 0.0, 1.0);

/// Reproduce the exact user scenario:
/// 1. Start with "A second textbox.\n\nDrag me around!"
/// 2. Bold "second" (chars 2..8 on line 0)
/// 3. Color "second" red
/// 4. Call set_color(blue)
/// 5. All per-span colors should be None (stripped).
/// 6. default_style.color should be blue.
///
/// The widget passes default_style through .color(), so on screen everything is blue.
#[test]
fn set_color_strips_all_span_colors() {
    let c = C::with_text("A second textbox.\n\nDrag me around!");

    // Bold "second" (chars 2..8)
    select_range(&c, 2, 6);
    c.perform(Format::ToggleBold);

    // Color "second" red
    select_range(&c, 2, 6);
    c.perform(Format::SetColor(Some(RED)));

    // Verify red is present before set_color
    let lines_before = c.styled_lines();
    let has_red = lines_before[0]
        .runs
        .iter()
        .any(|r| r.style.color == Some(RED));
    assert!(has_red, "red should be present before set_color");

    // Now set_color(blue) on the whole document
    c.set_color(BLUE);

    // Every span on every line should have color: None (stripped).
    let lines = c.styled_lines();
    for (i, line) in lines.iter().enumerate() {
        for (j, run) in line.runs.iter().enumerate() {
            assert_eq!(
                run.style.color, None,
                "line {i} run {j} (range {:?}) should have color stripped (None) but has {:?}",
                run.range, run.style.color,
            );
        }
    }
}

/// The serialized .mr should contain NO color attributes after set_color —
/// the color lives on the widget, not in the document.
#[test]
fn set_color_serialization_has_no_colors() {
    let c = C::with_text("A second textbox.\n\nDrag me around!");

    // Bold + red on "second"
    select_range(&c, 2, 6);
    c.perform(Format::ToggleBold);
    select_range(&c, 2, 6);
    c.perform(Format::SetColor(Some(RED)));

    // Set whole doc to blue
    c.set_color(BLUE);

    let mr = c.serialize();
    assert!(
        !mr.contains("ff0000"),
        "red color should not appear in serialization.\n.mr:\n{mr}"
    );
    assert!(
        !mr.contains("0000ff"),
        "blue color should not appear in serialization (it's on the widget, not spans).\n.mr:\n{mr}"
    );
    // Bold should survive
    assert!(
        mr.contains("{{b} second}"),
        "bold on 'second' should survive.\n.mr:\n{mr}"
    );
}

/// set_color on plain text should strip any paragraph default colors too.
#[test]
fn set_color_strips_paragraph_default_colors() {
    let input = ">|d:c=ff0000|\nRed paragraph";
    let c = C::parse(input).expect("parse failed");

    c.set_color(BLUE);

    let lines = c.styled_lines();
    // Paragraph default color should be None
    assert_eq!(
        lines[0].paragraph_style.style.color, None,
        "paragraph default color should be stripped.\nParagraph style: {:?}",
        lines[0].paragraph_style,
    );
}

/// THE BUG: after set_color + update_layout, all lines should render with
/// the same color. Currently, lines where strip_attr created explicit spans
/// (line 0) show black, while untouched lines (line 2) show blue.
///
/// This test reproduces the exact user scenario:
/// 1. "A second textbox.\n\nDrag me around!"
/// 2. Bold "second", color it red
/// 3. set_color(BLUE)
/// 4. update_layout() — propagates default_style to editor
/// 5. Read styled_lines() — all non-empty lines should have consistent color.
#[test]
fn set_color_renders_uniformly_after_layout() {
    let c = C::with_text("A second textbox.\n\nDrag me around!");

    // Bold "second" and color it red
    select_range(&c, 2, 6);
    c.perform(Format::ToggleBold);
    select_range(&c, 2, 6);
    c.perform(Format::SetColor(Some(RED)));

    // Set whole doc to blue
    c.set_color(BLUE);

    // Trigger layout so default_style propagates to editor
    c.update_layout(Size::new(400.0, 400.0));

    // Read what the editor actually has after layout.
    let lines = c.styled_lines();

    // Collect the color of the first run on each non-empty line.
    let line_colors: Vec<_> = lines
        .iter()
        .filter(|l| !l.text.is_empty())
        .map(|l| l.runs[0].style.color)
        .collect();

    // All non-empty lines must show the same color. If line 0 has None
    // but line 2 has Some(blue), the rendering is inconsistent.
    assert!(
        line_colors.windows(2).all(|w| w[0] == w[1]),
        "all lines should render with the same color after set_color.\n\
         Line colors: {line_colors:?}\n\
         Expected uniform color across all lines.",
    );
}
