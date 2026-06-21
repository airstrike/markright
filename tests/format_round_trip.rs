//! Integration tests for .mr format round-tripping through Content.
//!
//! These tests go through the full pipeline: parse .mr → Content → serialize
//! back to .mr, verifying that all styling survives.

use markright::widget::rich_editor::{Alignment, Content, Format};
use markright_document::format as mr;

type C = Content<iced::Renderer>;

#[test]
fn paragraph_character_defaults_are_preserved_and_spans_stay_sparse() {
    // Paragraph character defaults (d:b, d:sz=28) survive parsing and
    // the spans remain sparse — every field that matches defaults is
    // `None`, not duplicated onto each run.
    let input = ">|d:b d:sz=28|\nHello world";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse failed"));

    let lines = content.styled_lines();
    let line = &lines[0];

    // Paragraph defaults round-trip intact.
    assert_eq!(line.paragraph.style.style.bold, Some(true));
    assert_eq!(line.paragraph.style.style.size, Some(28.0));

    // Spans carry no redundant override for fields that match defaults.
    assert!(
        line.runs.iter().all(|r| r.style.bold.is_none()),
        "spans should be sparse — bold matches defaults.\nRuns: {:?}",
        line.runs,
    );
    assert!(
        line.runs.iter().all(|r| r.style.size.is_none()),
        "spans should be sparse — size matches defaults.\nRuns: {:?}",
        line.runs,
    );
}

#[test]
fn paragraph_defaults_with_mixed_spans_stay_sparse() {
    // Paragraph defaults italic, with one span explicitly overriding
    // to bold. The unstyled text has no italic override (it's the
    // default), and the bold span has bold=Some(true) with italic=None.
    let input = ">|d:i|\nnormal {{b} bold part} normal";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse failed"));

    let lines = content.styled_lines();
    let line = &lines[0];

    assert_eq!(line.paragraph.style.style.italic, Some(true));

    // "normal" at the start: no overrides (italic is the default).
    let first_run = &line.runs[0];
    assert!(
        first_run.style.italic.is_none(),
        "unstyled span italic should be None (matches paragraph default).\nRuns: {:?}",
        line.runs,
    );

    // "bold part": bold=Some(true), italic still None (inherits default).
    let bold_run = &line.runs[1];
    assert_eq!(bold_run.style.bold, Some(true));
    assert!(
        bold_run.style.italic.is_none(),
        "bold span italic should be None (inherits paragraph default).\nRuns: {:?}",
        line.runs,
    );
}

#[test]
fn sample_file_round_trips_through_content() {
    let input = include_str!("../examples/editor/sample.mr");
    let content = C::from_styled_lines(&mr::parse(input).expect("parse failed"));
    let output = mr::serialize(&content.styled_lines());

    // Re-parse and verify same number of lines and same text
    let original = mr::parse(input).expect("original parse failed");
    let reparsed = mr::parse(&output).expect("reparse failed");

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
    let content = C::from_styled_lines(&mr::parse(input).expect("parse failed"));

    // Change alignment to left
    content.perform(Format::SetAlignment(Alignment::Left));

    let lines = content.styled_lines();
    let line = &lines[0];

    // Paragraph italic default survives the alignment change.
    assert_eq!(
        line.paragraph.style.style.italic,
        Some(true),
        "paragraph default italic lost after alignment change.\nParagraph: {:?}",
        line.paragraph,
    );

    // Spans remain sparse — italic matches defaults, so None.
    assert!(
        line.runs.iter().all(|r| r.style.italic.is_none()),
        "spans should be sparse after alignment change.\nRuns: {:?}",
        line.runs,
    );
}
