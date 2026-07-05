//! Contiguous container paragraphs (code blocks, multi-line quotes) form a
//! single visual block: interior vertical margins collapse so the lines pack
//! tightly, while the block stays separated from its neighbours.

use markright::widget::rich_editor::Content;
use markright_document::format;

type C = Content<iced::Renderer>;

const BOUNDS: iced::Size = iced::Size::new(800.0, 600.0);

fn from_mr(input: &str) -> C {
    let c = C::from_styled_lines(&format::parse(input).expect("parse failed"));
    c.update_layout(BOUNDS);
    c
}

/// A four-line code block: `fn`, `let`, `println`, `}` on lines 2..=5,
/// preceded by an empty body line and followed by another body line.
const DOC: &str = concat!(
    "before\n",
    ">|name=code-block|\nfn main() {\n",
    ">|name=code-block|\n    let x = 1;\n",
    ">|name=code-block|\n    dbg!(x);\n",
    ">|name=code-block|\n}\n",
    "after",
);

#[test]
fn interior_code_lines_are_contiguous() {
    let c = from_mr(DOC);

    // Lines 1..=4 are the code block. Interior boundaries (2->3, 3->4) must
    // have zero gap: next line_top equals previous line's bottom.
    for line in 2..=3 {
        let g = c.line_geometry(line).expect("code line");
        let next = c.line_geometry(line + 1).expect("next code line");
        let gap = next.line_top - (g.line_top + g.line_height);
        assert!(
            gap.abs() < 0.5,
            "interior code boundary {line}->{} should collapse to 0, got {gap}",
            line + 1,
        );
    }
}

#[test]
fn code_block_keeps_outer_spacing() {
    let c = from_mr(DOC);

    // Boundary into the block (line 0 body -> line 1 first code line) keeps
    // the code-block's space_before (theme: 12).
    let g0 = c.line_geometry(0).expect("before");
    let g1 = c.line_geometry(1).expect("first code line");
    let top_gap = g1.line_top - (g0.line_top + g0.line_height);
    assert!(
        top_gap > 5.0,
        "block should stay separated from the paragraph above: gap {top_gap}",
    );

    // Boundary out of the block (last code line -> trailing body line) keeps
    // the code-block's spacing_after.
    let g4 = c.line_geometry(4).expect("last code line");
    let g5 = c.line_geometry(5).expect("after");
    let bottom_gap = g5.line_top - (g4.line_top + g4.line_height);
    assert!(
        bottom_gap > 5.0,
        "block should stay separated from the paragraph below: gap {bottom_gap}",
    );
}

#[test]
fn plain_body_paragraphs_do_not_collapse() {
    // Adjacent non-contiguous paragraphs with explicit spacing must keep it —
    // collapsing only applies to styles marked `contiguous`, not body text.
    let c = from_mr(">|sa=15|\naaa\n>|sb=10|\nbbb");

    let g0 = c.line_geometry(0).expect("line 0");
    let g1 = c.line_geometry(1).expect("line 1");
    let gap = g1.line_top - (g0.line_top + g0.line_height);
    assert!(
        (gap - 25.0).abs() < 0.5,
        "body paragraphs should keep sa+sb = 25px gap, got {gap}",
    );
}

#[test]
fn code_lines_scale_line_height_to_font() {
    // The code block's font is 14px in a 16px document. Its line height must
    // scale to the code font (14 * ratio) rather than being floored at the
    // 16px buffer default — while surrounding body text keeps the default.
    let c = from_mr(DOC);

    let body = c.line_geometry(0).expect("body line"); // "before"
    let code = c.line_geometry(2).expect("code line"); // "    let x = 1;"

    assert!(
        code.line_height < body.line_height - 1.0,
        "code line height ({}) should be tighter than body ({})",
        code.line_height,
        body.line_height,
    );
    // Tied to the 14/16 font-size ratio (same line-height multiplier).
    let expected = body.line_height * 14.0 / 16.0;
    assert!(
        (code.line_height - expected).abs() < 0.5,
        "code line height ({}) should scale to the 14px font (~{expected})",
        code.line_height,
    );
}

#[test]
fn contiguous_attribute_not_fill_controls_collapse() {
    // A code-block style carries a fill, but with `contiguous=false` explicitly
    // set the lines must NOT merge — proving the attribute, not the presence of
    // a fill, is what drives grouping.
    let c = from_mr(concat!(
        ">|name=code-block contiguous=false|\nfn main() {\n",
        ">|name=code-block contiguous=false|\n}",
    ));

    let g0 = c.line_geometry(0).expect("line 0");
    let g1 = c.line_geometry(1).expect("line 1");
    let gap = g1.line_top - (g0.line_top + g0.line_height);
    assert!(
        gap > 5.0,
        "contiguous=false must keep interior spacing (sa+sb), got {gap}",
    );
}

#[test]
fn contiguous_round_trips_through_mr() {
    // A code-block with contiguous flipped off from its theme default should
    // serialize the override and parse back to the same value.
    let lines = format::parse(">|name=code-block contiguous=false|\nx").expect("parse");
    assert!(!lines[0].paragraph.style.contiguous);

    let text = format::serialize(&lines);
    assert!(
        text.contains("contiguous=false"),
        "non-default contiguous must be serialized: {text}",
    );

    let reparsed = format::parse(&text).expect("reparse");
    assert!(!reparsed[0].paragraph.style.contiguous);
}
