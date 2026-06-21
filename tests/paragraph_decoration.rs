//! Integration tests for paragraph fill and border round-tripping.
//!
//! These tests verify that fill and border data survives the
//! Content pipeline: .mr parse → Content → styled_lines() → .mr serialize.

use markright::widget::rich_editor::Content;
use markright_document::format as mr;

type C = Content<iced::Renderer>;

#[test]
fn code_block_fill_round_trips() {
    let input = ">|name=code-block fill=eeeeee d:f=Monospace d:sz=14|\nfn main() {}";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse"));
    let lines = content.styled_lines();

    let fill = lines[0]
        .paragraph
        .style
        .fill
        .as_ref()
        .expect("should have fill");
    assert!(
        fill.height.is_none(),
        "code-block fill should be full background"
    );

    let c = fill.color.expect("explicit fill color from .mr format");
    assert!(
        (c.r - 0xee as f32 / 255.0).abs() < 0.01
            && (c.g - 0xee as f32 / 255.0).abs() < 0.01
            && (c.b - 0xee as f32 / 255.0).abs() < 0.01,
        "fill color should be #eeeeee, got r={} g={} b={}",
        c.r,
        c.g,
        c.b,
    );
}

#[test]
fn rule_hairline_fill_round_trips() {
    let input = ">|name=rule fill=cccccc:1|\n";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse"));
    let lines = content.styled_lines();

    let fill = lines[0]
        .paragraph
        .style
        .fill
        .as_ref()
        .expect("should have fill");
    assert_eq!(fill.height, Some(1.0), "rule fill should have height=1");
}

#[test]
fn block_quote_left_border_round_trips() {
    let input = ">|name=block-quote border-left=3:cccccc|\nSome quote";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse"));
    let lines = content.styled_lines();

    let borders = lines[0]
        .paragraph
        .style
        .borders
        .as_ref()
        .expect("should have borders");
    let left = borders.left.as_ref().expect("should have left border");
    assert!(
        (left.width - 3.0).abs() < 0.01,
        "left border width should be 3"
    );
}

#[test]
fn fill_survives_serialize_round_trip() {
    let input = ">|name=code-block fill=eeeeee d:f=Monospace d:sz=14|\nsome code";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse"));
    let lines = content.styled_lines();
    let serialized = mr::serialize(&lines);

    // Re-parse
    let content2 = C::from_styled_lines(&mr::parse(&serialized).expect("re-parse"));
    let lines2 = content2.styled_lines();

    assert_eq!(
        lines[0].paragraph.style.fill, lines2[0].paragraph.style.fill,
        "fill should survive serialize → parse round trip"
    );
}

#[test]
fn borders_survive_serialize_round_trip() {
    let input = ">|name=block-quote border-left=3:cccccc|\nquoted text";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse"));
    let lines = content.styled_lines();
    let serialized = mr::serialize(&lines);

    let content2 = C::from_styled_lines(&mr::parse(&serialized).expect("re-parse"));
    let lines2 = content2.styled_lines();

    assert_eq!(
        lines[0].paragraph.style.borders, lines2[0].paragraph.style.borders,
        "borders should survive serialize → parse round trip"
    );
}

#[test]
fn paragraph_with_no_fill_or_borders_is_default() {
    let input = "plain text";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse"));
    let lines = content.styled_lines();

    assert!(
        lines[0].paragraph.style.fill.is_none(),
        "plain paragraph should have no fill"
    );
    assert!(
        lines[0].paragraph.style.borders.is_none(),
        "plain paragraph should have no borders"
    );
}

#[test]
fn theme_default_fill_applied_via_name() {
    // CODE_BLOCK's theme default includes a fill. Verify it's present
    // when we use name=code-block without explicit fill.
    let input = ">|name=code-block|\ncode";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse"));
    let lines = content.styled_lines();

    // The theme should apply code-block defaults including fill
    let para = &lines[0].paragraph;
    assert_eq!(para.name, markright_core::Name::CODE_BLOCK);
    // Fill comes from theme — check it exists
    assert!(
        para.style.fill.is_some(),
        "code-block should have theme-default fill"
    );
}

#[test]
fn theme_default_borders_applied_via_name() {
    // BLOCK_QUOTE's theme default includes a left border.
    let input = ">|name=block-quote|\nquote";
    let content = C::from_styled_lines(&mr::parse(input).expect("parse"));
    let lines = content.styled_lines();

    let para = &lines[0].paragraph;
    assert_eq!(para.name, markright_core::Name::BLOCK_QUOTE);
    assert!(
        para.style.borders.is_some(),
        "block-quote should have theme-default borders"
    );
    let borders = para.style.borders.as_ref().unwrap();
    assert!(
        borders.left.is_some(),
        "block-quote should have theme-default left border"
    );
}
