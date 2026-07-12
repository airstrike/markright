use std::collections::HashMap;

use iced_core::text::rich_editor::span;

use crate::paragraph::{self, Border, Borders, Fill, Name, OverrideSet, Paragraph};

/// Theme maps paragraph names to default styles.
///
/// When a user changes a paragraph's name via `SetName`, `Theme::apply`
/// copies theme defaults for the new name while preserving any fields
/// the user has explicitly overridden (tracked by `OverrideSet`).
#[derive(Clone)]
pub struct Theme {
    entries: HashMap<Name, Paragraph>,
    fallback: Paragraph,
    /// Inline code size as a fraction of the containing paragraph's
    /// explicit size (e.g. `0.85`). Paragraphs without an explicit
    /// size (typically body text) keep the CODE_BLOCK entry's
    /// absolute size instead.
    code_scale: Option<f32>,
}

impl Theme {
    /// Scale inline code relative to its containing paragraph's size.
    pub fn with_code_scale(mut self, scale: f32) -> Self {
        self.code_scale = Some(scale);
        self
    }

    pub fn code_scale(&self) -> Option<f32> {
        self.code_scale
    }

    pub fn get(&self, name: Name) -> &Paragraph {
        self.entries.get(&name).unwrap_or(&self.fallback)
    }

    pub fn with_entry(mut self, p: Paragraph) -> Self {
        self.entries.insert(p.name, p);
        self
    }

    /// Re-theme a paragraph. Copies theme defaults for `name` onto `p`,
    /// preserving any fields the user has explicitly overridden.
    pub fn apply(&self, p: &mut Paragraph, name: Name) {
        let base = self.get(name);

        // Paragraph-level fields
        if !p.overrides().contains(OverrideSet::ALIGNMENT) {
            p.style.alignment = base.style.alignment;
        }
        if !p.overrides().contains(OverrideSet::SPACE_BEFORE) {
            p.style.space_before = base.style.space_before;
        }
        if !p.overrides().contains(OverrideSet::SPACING_AFTER) {
            p.style.spacing_after = base.style.spacing_after;
        }
        if !p.overrides().contains(OverrideSet::CONTIGUOUS) {
            p.style.contiguous = base.style.contiguous;
        }
        if !p.overrides().contains(OverrideSet::LINE_HEIGHT) {
            p.style.line_height = base.style.line_height;
        }
        if !p.overrides().contains(OverrideSet::LINE_SPACING) {
            p.style.line_spacing = base.style.line_spacing;
        }
        if !p.overrides().contains(OverrideSet::LEVEL) {
            p.style.level = base.style.level;
        }
        if !p.overrides().contains(OverrideSet::LIST) {
            p.style.list = base.style.list.clone();
        }
        if !p.overrides().contains(OverrideSet::INDENT) {
            p.style.indent = base.style.indent;
        }
        if !p.overrides().contains(OverrideSet::FILL) {
            p.style.fill = base.style.fill;
        }
        if !p.overrides().contains(OverrideSet::BORDERS) {
            p.style.borders = base.style.borders.clone();
        }

        // Character-level defaults
        if !p.overrides().contains(OverrideSet::BOLD) {
            p.style.style.bold = base.style.style.bold;
        }
        if !p.overrides().contains(OverrideSet::ITALIC) {
            p.style.style.italic = base.style.style.italic;
        }
        if !p.overrides().contains(OverrideSet::UNDERLINE) {
            p.style.style.underline = base.style.style.underline;
        }
        if !p.overrides().contains(OverrideSet::STRIKETHROUGH) {
            p.style.style.strikethrough = base.style.style.strikethrough;
        }
        if !p.overrides().contains(OverrideSet::FONT) {
            p.style.style.font = base.style.style.font;
        }
        if !p.overrides().contains(OverrideSet::SIZE) {
            p.style.style.size = base.style.style.size;
        }
        if !p.overrides().contains(OverrideSet::COLOR) {
            p.style.style.color = base.style.style.color;
        }
        if !p.overrides().contains(OverrideSet::LETTER_SPACING) {
            p.style.style.letter_spacing = base.style.style.letter_spacing;
        }

        p.name = name;
    }
}

/// Build a heading paragraph with conventional sizing.
fn heading(name: Name, size: f32, space_before: f32, spacing_after: f32) -> Paragraph {
    Paragraph::new(
        name,
        paragraph::Style {
            style: span::Style {
                bold: Some(true),
                size: Some(size),
                ..span::Style::default()
            },
            space_before: Some(space_before),
            spacing_after: Some(spacing_after),
            ..paragraph::Style::default()
        },
    )
}

impl Default for Theme {
    fn default() -> Self {
        let mut entries = HashMap::new();

        // BODY: sb=0 sa=8
        entries.insert(
            Name::BODY,
            Paragraph::new(
                Name::BODY,
                paragraph::Style {
                    spacing_after: Some(8.0),
                    ..paragraph::Style::default()
                },
            ),
        );

        // Headings per MAP_PLAN table
        entries.insert(Name::HEADING_1, heading(Name::HEADING_1, 32.0, 24.0, 12.0));
        entries.insert(Name::HEADING_2, heading(Name::HEADING_2, 28.0, 20.0, 10.0));
        entries.insert(Name::HEADING_3, heading(Name::HEADING_3, 24.0, 16.0, 8.0));
        entries.insert(Name::HEADING_4, heading(Name::HEADING_4, 20.0, 12.0, 6.0));
        entries.insert(Name::HEADING_5, heading(Name::HEADING_5, 18.0, 10.0, 4.0));
        entries.insert(Name::HEADING_6, heading(Name::HEADING_6, 16.0, 8.0, 4.0));

        // CODE_BLOCK: sb=12 sa=12, size=14, monospace, fill (color from widget style)
        entries.insert(
            Name::CODE_BLOCK,
            Paragraph::new(
                Name::CODE_BLOCK,
                paragraph::Style {
                    style: span::Style {
                        size: Some(14.0),
                        font: Some(iced_core::Font::MONOSPACE),
                        ..span::Style::default()
                    },
                    space_before: Some(12.0),
                    spacing_after: Some(12.0),
                    contiguous: true,
                    fill: Some(Fill {
                        color: None,
                        height: None,
                        radius: 4.0,
                        padding: iced_core::Padding::new(8.0),
                    }),
                    indent: paragraph::Indent {
                        left: 8.0,
                        ..paragraph::Indent::default()
                    },
                    ..paragraph::Style::default()
                },
            ),
        );

        // BLOCK_QUOTE: sb=8 sa=8, left border 3px (color from widget style)
        entries.insert(
            Name::BLOCK_QUOTE,
            Paragraph::new(
                Name::BLOCK_QUOTE,
                paragraph::Style {
                    space_before: Some(8.0),
                    spacing_after: Some(8.0),
                    contiguous: true,
                    indent: paragraph::Indent {
                        left: 12.0,
                        ..paragraph::Indent::default()
                    },
                    borders: Some(Box::new(Borders {
                        left: Some(Border {
                            color: None,
                            width: 3.0,
                        }),
                        ..Borders::default()
                    })),
                    ..paragraph::Style::default()
                },
            ),
        );

        // RULE: sb=12 sa=12, hairline 1px (color from widget style)
        entries.insert(
            Name::RULE,
            Paragraph::new(
                Name::RULE,
                paragraph::Style {
                    space_before: Some(12.0),
                    spacing_after: Some(12.0),
                    fill: Some(Fill {
                        color: None,
                        height: Some(1.0),
                        radius: 0.0,
                        padding: iced_core::Padding::ZERO,
                    }),
                    ..paragraph::Style::default()
                },
            ),
        );

        Self {
            entries,
            fallback: Paragraph::default(),
            code_scale: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_all_entries() {
        let theme = Theme::default();
        let names = [
            Name::BODY,
            Name::HEADING_1,
            Name::HEADING_2,
            Name::HEADING_3,
            Name::HEADING_4,
            Name::HEADING_5,
            Name::HEADING_6,
            Name::CODE_BLOCK,
            Name::BLOCK_QUOTE,
            Name::RULE,
        ];
        for name in &names {
            assert!(
                theme.entries.contains_key(name),
                "missing theme entry for {name}"
            );
        }
    }

    #[test]
    fn heading_1_defaults() {
        let theme = Theme::default();
        let h1 = theme.get(Name::HEADING_1);
        assert_eq!(h1.style.style.bold, Some(true));
        assert_eq!(h1.style.style.size, Some(32.0));
        assert_eq!(h1.style.space_before, Some(24.0));
        assert_eq!(h1.style.spacing_after, Some(12.0));
    }

    #[test]
    fn code_block_has_fill() {
        let theme = Theme::default();
        let cb = theme.get(Name::CODE_BLOCK);
        assert!(cb.style.fill.is_some());
        assert!(cb.style.fill.as_ref().unwrap().height.is_none()); // background, not hairline
        assert_eq!(cb.style.style.font, Some(iced_core::Font::MONOSPACE));
    }

    #[test]
    fn block_quote_has_left_border() {
        let theme = Theme::default();
        let bq = theme.get(Name::BLOCK_QUOTE);
        let borders = bq.style.borders.as_ref().expect("borders should be Some");
        assert!(borders.left.is_some());
        assert!(borders.top.is_none());
    }

    #[test]
    fn rule_has_hairline_fill() {
        let theme = Theme::default();
        let rule = theme.get(Name::RULE);
        let fill = rule.style.fill.as_ref().expect("fill should be Some");
        assert_eq!(fill.height, Some(1.0));
    }

    #[test]
    fn apply_preserves_overridden_size() {
        let theme = Theme::default();
        let mut p = Paragraph::body();
        p.style.style.size = Some(40.0);
        p.set_override(OverrideSet::SIZE);

        theme.apply(&mut p, Name::HEADING_1);

        assert_eq!(p.name, Name::HEADING_1);
        // Size preserved (user override)
        assert_eq!(p.style.style.size, Some(40.0));
        // Bold applied from theme (not overridden)
        assert_eq!(p.style.style.bold, Some(true));
    }

    #[test]
    fn apply_clobbers_non_overridden_fields() {
        let theme = Theme::default();
        let mut p = Paragraph::new(
            Name::HEADING_1,
            paragraph::Style {
                style: span::Style {
                    bold: Some(true),
                    size: Some(32.0),
                    ..span::Style::default()
                },
                space_before: Some(24.0),
                spacing_after: Some(12.0),
                ..paragraph::Style::default()
            },
        );
        // No overrides set — everything should be clobbered

        theme.apply(&mut p, Name::BODY);

        assert_eq!(p.name, Name::BODY);
        assert_eq!(p.style.style.bold, None);
        assert_eq!(p.style.style.size, None);
        assert_eq!(p.style.spacing_after, Some(8.0));
    }

    #[test]
    fn unknown_name_gets_fallback() {
        let theme = Theme::default();
        let p = theme.get(Name::from_slug("unknown-thing"));
        assert_eq!(p.name, Name::BODY);
    }
}
