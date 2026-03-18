//! Tests for Content::set_font — setting a document-wide font should
//! strip all per-span fonts so that the widget's .font() default takes over.

use iced::{Color, Font, Size, font};
use markright::widget::rich_editor::{Action, Content, Format, Motion};

type C = Content<iced::Renderer>;

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

const SERIF: Font = Font {
    family: font::Family::Serif,
    ..Font::DEFAULT
};

const MONO: Font = Font {
    family: font::Family::Monospace,
    ..Font::DEFAULT
};

/// Reproduce the same pattern as the set_color bug:
/// 1. "A second textbox.\n\nDrag me around!"
/// 2. Bold "second", set its font to Mono
/// 3. set_font(Serif) on the whole document
/// 4. After update_layout, all lines should render with the same font.
#[test]
fn set_font_renders_uniformly_after_layout() {
    let c = C::with_text("A second textbox.\n\nDrag me around!");

    // Bold "second" and set it to monospace
    select_range(&c, 2, 6);
    c.perform(Format::ToggleBold);
    select_range(&c, 2, 6);
    c.perform(Format::SetFont(MONO));

    // Set whole doc to serif
    c.set_font(SERIF);

    // Trigger layout so default_style propagates
    c.update_layout(Size::new(400.0, 400.0));

    let lines = c.styled_lines();

    // Collect the font of the first run on each non-empty line.
    let line_fonts: Vec<_> = lines
        .iter()
        .filter(|l| !l.text.is_empty())
        .map(|l| l.runs[0].style.font)
        .collect();

    // All non-empty lines must show the same font.
    assert!(
        line_fonts.windows(2).all(|w| w[0] == w[1]),
        "all lines should render with the same font after set_font.\n\
         Line fonts: {line_fonts:?}\n\
         Expected uniform font across all lines.",
    );
}

/// After set_font, no span should retain an explicit font override.
#[test]
fn set_font_strips_all_span_fonts() {
    let c = C::with_text("A second textbox.\n\nDrag me around!");

    // Set "second" to monospace
    select_range(&c, 2, 6);
    c.perform(Format::SetFont(MONO));

    // Verify mono is present before set_font
    let lines_before = c.styled_lines();
    let has_mono = lines_before[0]
        .runs
        .iter()
        .any(|r| r.style.font == Some(MONO));
    assert!(has_mono, "mono should be present before set_font");

    // Set whole doc to serif
    c.set_font(SERIF);

    // Every span should have font: None (stripped).
    let lines = c.styled_lines();
    for (i, line) in lines.iter().enumerate() {
        for (j, run) in line.runs.iter().enumerate() {
            assert_eq!(
                run.style.font, None,
                "line {i} run {j} (range {:?}) should have font stripped (None) but has {:?}",
                run.range, run.style.font,
            );
        }
    }
}

/// Serialized .mr should contain no font attributes after set_font.
#[test]
fn set_font_serialization_has_no_fonts() {
    let c = C::with_text("A second textbox.\n\nDrag me around!");

    // Bold + mono on "second"
    select_range(&c, 2, 6);
    c.perform(Format::ToggleBold);
    select_range(&c, 2, 6);
    c.perform(Format::SetFont(MONO));

    // Set whole doc to serif
    c.set_font(SERIF);

    let mr = c.serialize();
    assert!(
        !mr.contains("f="),
        "font attributes should not appear in serialization.\n.mr:\n{mr}"
    );
    // Bold should survive
    assert!(
        mr.contains("{{b} second}"),
        "bold on 'second' should survive.\n.mr:\n{mr}"
    );
}

/// Combined: set_font + set_color, then verify both propagate uniformly.
#[test]
fn set_font_and_color_both_uniform_after_layout() {
    let c = C::with_text("styled text\n\nplain text");

    // Color first line red, set font to mono
    select_range(&c, 0, 11);
    c.perform(Format::SetColor(Some(RED)));
    select_range(&c, 0, 11);
    c.perform(Format::SetFont(MONO));

    // Set whole doc to serif + blue (simulated by setting font then color)
    let blue = Color::from_rgb(0.0, 0.0, 1.0);
    c.set_font(SERIF);
    c.set_color(blue);

    c.update_layout(Size::new(400.0, 400.0));

    let lines = c.styled_lines();
    let non_empty: Vec<_> = lines.iter().filter(|l| !l.text.is_empty()).collect();

    // Both lines should have identical color and font on their first run
    let colors: Vec<_> = non_empty.iter().map(|l| l.runs[0].style.color).collect();
    let fonts: Vec<_> = non_empty.iter().map(|l| l.runs[0].style.font).collect();

    assert!(
        colors.windows(2).all(|w| w[0] == w[1]),
        "colors should be uniform.\nLine colors: {colors:?}",
    );
    assert!(
        fonts.windows(2).all(|w| w[0] == w[1]),
        "fonts should be uniform.\nLine fonts: {fonts:?}",
    );
}
