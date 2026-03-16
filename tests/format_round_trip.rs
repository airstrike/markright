//! Integration tests for .mr format round-tripping through Content.
//!
//! These tests go through the full pipeline: parse .mr → Content → serialize
//! back to .mr, verifying that all styling survives.

use markright::widget::rich_editor::{Alignment, Content, Format};

type C = Content<iced::Renderer>;

#[test]
fn paragraph_character_defaults_applied_to_spans() {
    // Paragraph character defaults (d:b, d:sz=28) must be visually applied.
    // The span runs that the renderer sees should reflect the defaults —
    // not just the paragraph_style metadata.
    let input = ">|d:b d:sz=28|\nHello world";
    let content = C::parse(input).expect("parse failed");

    let lines = content.styled_lines();
    let line = &lines[0];

    // The runs are what the renderer draws. They MUST show bold + 28px.
    assert!(
        line.runs.iter().all(|r| r.style.bold == Some(true)),
        "paragraph default bold not applied to spans.\nRuns: {:?}\nParagraph style: {:?}",
        line.runs,
        line.paragraph_style,
    );
    assert!(
        line.runs.iter().all(|r| r.style.size == Some(28.0)),
        "paragraph default size=28 not applied to spans.\nRuns: {:?}\nParagraph style: {:?}",
        line.runs,
        line.paragraph_style,
    );
}

#[test]
fn paragraph_defaults_with_mixed_spans() {
    // Paragraph defaults italic, with one span overriding to bold+italic.
    // The non-bold "normal" text should still be italic from the paragraph default.
    let input = ">|d:i|\nnormal {{b} bold part} normal";
    let content = C::parse(input).expect("parse failed");

    let lines = content.styled_lines();
    let line = &lines[0];

    // "normal" at the start must be italic (from paragraph default)
    let first_run = &line.runs[0];
    assert!(
        first_run.style.italic == Some(true),
        "paragraph default italic not applied to unstyled span.\nRuns: {:?}\nParagraph style: {:?}",
        line.runs,
        line.paragraph_style,
    );
}

#[test]
fn sample_file_round_trips_through_content() {
    let input = include_str!("../examples/editor/sample.mr");
    let content = C::parse(input).expect("parse failed");
    let output = content.serialize();

    // Re-parse and verify same number of lines and same text
    let original = markright_document::format::parse(input).expect("original parse failed");
    let reparsed = markright_document::format::parse(&output).expect("reparse failed");

    assert_eq!(
        original.len(),
        reparsed.len(),
        "line count changed.\nOriginal: {}\nRound-tripped: {}",
        original.len(),
        reparsed.len(),
    );

    for (i, (orig, rt)) in original.iter().zip(reparsed.iter()).enumerate() {
        assert_eq!(
            orig.text, rt.text,
            "text mismatch on line {i}.\nOriginal:      {:?}\nRound-tripped: {:?}",
            orig.text, rt.text,
        );
    }
}

#[test]
fn alignment_change_preserves_paragraph_character_defaults() {
    // Changing alignment must not wipe out paragraph character defaults (d:i).
    let input = ">|align=center d:i|\nTransit complete.";
    let content = C::parse(input).expect("parse failed");

    // Change alignment to left
    content.perform(Format::SetAlignment(Alignment::Left));

    let lines = content.styled_lines();
    let line = &lines[0];

    // The italic paragraph default must survive the alignment change
    assert!(
        line.runs.iter().all(|r| r.style.italic == Some(true)),
        "paragraph default italic lost after alignment change.\nRuns: {:?}\nParagraph style: {:?}",
        line.runs,
        line.paragraph_style,
    );
}
