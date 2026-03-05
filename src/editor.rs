// WYSIWYG block-based markdown editor widget for iced.

use crate::document::{self, BlockKind, SpanStyle};

use iced_core::alignment;
use iced_core::keyboard;
use iced_core::keyboard::key::Named;
use iced_core::mouse;
use iced_core::renderer;
use iced_core::text;
use iced_core::text::Paragraph as _;
use iced_core::widget::tree;
use iced_core::{
    Background, Border, Color, Element, Event, Font, Layout, Length, Padding, Pixels, Point,
    Rectangle, Shadow, Shell, Size, Widget,
};

/// Action that can be performed on the document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Action {
    Insert(char),
    Delete,
    Backspace,
    Enter,
    Move(Motion),
    Click { block: usize, offset: usize },
}

/// Cursor motion direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

/// A WYSIWYG block-based markdown editor widget.
///
/// The active block (where the cursor sits) shows raw markdown text.
/// All other blocks show formatted display text with markers hidden.
pub struct Editor<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer,
{
    document: &'a document::Document,
    on_action: Option<Box<dyn Fn(Action) -> Message + 'a>>,
    font: Option<Renderer::Font>,
    monospace_font: Option<Renderer::Font>,
    size: Option<Pixels>,
    line_height: text::LineHeight,
    width: Length,
    height: Length,
    padding: Padding,
    _theme: std::marker::PhantomData<Theme>,
}

/// Internal widget state, stored in the iced widget tree.
struct State<P: text::Paragraph> {
    paragraphs: Vec<BlockLayout<P>>,
    focus: bool,
    scroll_offset: f32,
}

/// Layout information for a single block.
struct BlockLayout<P: text::Paragraph> {
    paragraph: P,
    kind: BlockKind,
    y_offset: f32,
    height: f32,
}

impl<P: text::Paragraph> Default for State<P> {
    fn default() -> Self {
        Self {
            paragraphs: Vec::new(),
            focus: false,
            scroll_offset: 0.0,
        }
    }
}

/// Create an [`Editor`] widget for the given document.
pub fn editor<'a, Message, Theme, Renderer>(
    document: &'a document::Document,
) -> Editor<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer<Font = Font>,
{
    Editor::new(document)
}

impl<'a, Message, Theme, Renderer> Editor<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer<Font = Font>,
{
    /// Create a new editor widget referencing the given document.
    pub fn new(document: &'a document::Document) -> Self {
        Self {
            document,
            on_action: None,
            font: None,
            monospace_font: None,
            size: None,
            line_height: text::LineHeight::default(),
            width: Length::Fill,
            height: Length::Fill,
            padding: Padding::new(5.0),
            _theme: std::marker::PhantomData,
        }
    }

    /// Set the callback for editor actions.
    pub fn on_action(mut self, f: impl Fn(Action) -> Message + 'a) -> Self {
        self.on_action = Some(Box::new(f));
        self
    }

    /// Set the font used for text rendering.
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Set the monospace font used for code blocks and inline code.
    pub fn monospace_font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.monospace_font = Some(font.into());
        self
    }

    /// Set the text size in pixels.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Set the line height.
    pub fn line_height(mut self, line_height: impl Into<text::LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    /// Set the widget width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Set the widget height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Set the padding around the text area.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Resolve the effective font for this widget.
    fn effective_font(&self, renderer: &Renderer) -> Font {
        self.font.unwrap_or_else(|| renderer.default_font())
    }

    /// Resolve the effective monospace font.
    fn effective_monospace_font(&self, renderer: &Renderer) -> Font {
        self.monospace_font.unwrap_or_else(|| {
            let mut font = renderer.default_font();
            font.family = iced_core::font::Family::Monospace;
            font
        })
    }

    /// Resolve the effective text size for this widget.
    fn effective_size(&self, renderer: &Renderer) -> Pixels {
        self.size.unwrap_or_else(|| renderer.default_size())
    }

    /// Build a text::Text template for creating paragraphs.
    fn text_template(&self, renderer: &Renderer, bounds_width: f32) -> text::Text<(), Font> {
        text::Text {
            content: (),
            bounds: Size::new(bounds_width, f32::INFINITY),
            size: self.effective_size(renderer),
            line_height: self.line_height,
            font: self.effective_font(renderer),
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::Word,
            ellipsis: text::Ellipsis::None,
            hint_factor: None,
        }
    }

    /// Build paragraphs for all blocks in the document.
    ///
    /// The active block is rendered as raw text (plain, no formatting).
    /// Inactive blocks use their display cache for WYSIWYG formatting.
    fn build_paragraphs(
        &self,
        renderer: &Renderer,
        bounds_width: f32,
    ) -> Vec<BlockLayout<Renderer::Paragraph>> {
        let template = self.text_template(renderer, bounds_width);
        let base_font = self.effective_font(renderer);
        let mono_font = self.effective_monospace_font(renderer);
        let effective_size = self.effective_size(renderer);
        let active_block = self.document.active_block();

        let mut layouts = Vec::with_capacity(self.document.block_count());
        let mut y_offset = 0.0_f32;

        for i in 0..self.document.block_count() {
            let Some(block) = self.document.block(i) else {
                continue;
            };

            if i == active_block {
                // Active block: show raw markdown text, no formatting.
                let para =
                    Renderer::Paragraph::with_text(template.with_content(block.raw.as_str()));
                let height = para.min_bounds().height;

                layouts.push(BlockLayout {
                    paragraph: para,
                    kind: block.kind.clone(),
                    y_offset,
                    height,
                });

                y_offset += height;
            } else {
                match &block.kind {
                    BlockKind::ThematicBreak => {
                        // ThematicBreak: render an empty paragraph, draw the line in draw().
                        let para = Renderer::Paragraph::with_text(template.with_content(""));
                        let height = 20.0; // Fixed height for thematic breaks.

                        layouts.push(BlockLayout {
                            paragraph: para,
                            kind: block.kind.clone(),
                            y_offset,
                            height,
                        });

                        y_offset += height;
                    }
                    BlockKind::CodeBlock(_) => {
                        // CodeBlock: use display_cache content with monospace font.
                        let display_text = block
                            .display_cache
                            .as_ref()
                            .map(|c| c.display_text.as_str())
                            .unwrap_or(block.raw.as_str());

                        let code_size = Pixels(effective_size.0 * 0.9);
                        let code_template = text::Text {
                            font: mono_font,
                            size: code_size,
                            ..template
                        };

                        let para = Renderer::Paragraph::with_text(
                            code_template.with_content(display_text),
                        );
                        // Add padding around the code block content.
                        let content_height = para.min_bounds().height;
                        let height = content_height + 16.0; // 8px top + 8px bottom padding

                        layouts.push(BlockLayout {
                            paragraph: para,
                            kind: block.kind.clone(),
                            y_offset,
                            height,
                        });

                        y_offset += height;
                    }
                    BlockKind::Heading(level) => {
                        let scale = heading_scale(*level);
                        let heading_size = Pixels(effective_size.0 * scale);

                        if let Some(cache) = &block.display_cache {
                            if cache.spans.is_empty() {
                                // No inline formatting, just bold heading text.
                                let heading_font = Font {
                                    weight: iced_core::font::Weight::Bold,
                                    ..base_font
                                };
                                let heading_template = text::Text {
                                    font: heading_font,
                                    size: heading_size,
                                    ..template
                                };
                                let para = Renderer::Paragraph::with_text(
                                    heading_template.with_content(cache.display_text.as_str()),
                                );
                                let mut height = para.min_bounds().height;
                                // H1 gets a bottom border, add space for it.
                                if *level == 1 {
                                    height += 4.0;
                                }

                                layouts.push(BlockLayout {
                                    paragraph: para,
                                    kind: block.kind.clone(),
                                    y_offset,
                                    height,
                                });
                                y_offset += height;
                            } else {
                                let iced_spans = build_heading_spans(
                                    cache,
                                    base_font,
                                    mono_font,
                                    heading_size,
                                    *level,
                                );
                                let heading_template = text::Text {
                                    size: heading_size,
                                    ..template
                                };
                                let para = Renderer::Paragraph::with_spans(
                                    heading_template.with_content(iced_spans.as_slice()),
                                );
                                let mut height = para.min_bounds().height;
                                if *level == 1 {
                                    height += 4.0;
                                }

                                layouts.push(BlockLayout {
                                    paragraph: para,
                                    kind: block.kind.clone(),
                                    y_offset,
                                    height,
                                });
                                y_offset += height;
                            }
                        } else {
                            // No display cache, fall back to raw text.
                            let heading_font = Font {
                                weight: iced_core::font::Weight::Bold,
                                ..base_font
                            };
                            let heading_template = text::Text {
                                font: heading_font,
                                size: heading_size,
                                ..template
                            };
                            let para = Renderer::Paragraph::with_text(
                                heading_template.with_content(block.raw.as_str()),
                            );
                            let mut height = para.min_bounds().height;
                            if *level == 1 {
                                height += 4.0;
                            }

                            layouts.push(BlockLayout {
                                paragraph: para,
                                kind: block.kind.clone(),
                                y_offset,
                                height,
                            });
                            y_offset += height;
                        }
                    }
                    BlockKind::Paragraph => {
                        if let Some(cache) = &block.display_cache {
                            if cache.spans.is_empty() {
                                let para = Renderer::Paragraph::with_text(
                                    template.with_content(cache.display_text.as_str()),
                                );
                                let height = para.min_bounds().height;

                                layouts.push(BlockLayout {
                                    paragraph: para,
                                    kind: block.kind.clone(),
                                    y_offset,
                                    height,
                                });
                                y_offset += height;
                            } else {
                                let iced_spans = build_paragraph_spans(
                                    cache,
                                    base_font,
                                    mono_font,
                                    effective_size,
                                );
                                let para = Renderer::Paragraph::with_spans(
                                    template.with_content(iced_spans.as_slice()),
                                );
                                let height = para.min_bounds().height;

                                layouts.push(BlockLayout {
                                    paragraph: para,
                                    kind: block.kind.clone(),
                                    y_offset,
                                    height,
                                });
                                y_offset += height;
                            }
                        } else {
                            // No display cache, fall back to raw text.
                            let para = Renderer::Paragraph::with_text(
                                template.with_content(block.raw.as_str()),
                            );
                            let height = para.min_bounds().height;

                            layouts.push(BlockLayout {
                                paragraph: para,
                                kind: block.kind.clone(),
                                y_offset,
                                height,
                            });
                            y_offset += height;
                        }
                    }
                }
            }
        }

        layouts
    }
}

/// Return the font size scale factor for a heading level.
fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 2.0,
        2 => 1.5,
        3 => 1.25,
        4..=6 => 1.1,
        _ => 1.0,
    }
}

/// Build iced text spans for a paragraph block's display cache.
fn build_paragraph_spans<'a>(
    cache: &'a document::DisplayCache,
    base_font: Font,
    mono_font: Font,
    effective_size: Pixels,
) -> Vec<text::Span<'a, (), Font>> {
    build_inline_spans(cache, base_font, mono_font, effective_size, false)
}

/// Build iced text spans for a heading block's display cache.
///
/// All text in a heading gets bold weight. The size is set at the paragraph
/// level via the template, so we only need to apply additional inline styles.
fn build_heading_spans<'a>(
    cache: &'a document::DisplayCache,
    base_font: Font,
    mono_font: Font,
    _heading_size: Pixels,
    _level: u8,
) -> Vec<text::Span<'a, (), Font>> {
    // For headings, the base font is always bold.
    let heading_base = Font {
        weight: iced_core::font::Weight::Bold,
        ..base_font
    };
    let display = &cache.display_text;
    let mut spans = Vec::new();
    let mut pos = 0;

    for inline_span in &cache.spans {
        // Gap before this span: heading-styled (bold) text.
        if inline_span.range.start > pos {
            spans.push(text::Span::new(&display[pos..inline_span.range.start]).font(heading_base));
        }

        let style = &inline_span.style;
        let text_slice = &display[inline_span.range.clone()];

        let mut iced_span = text::Span::new(text_slice);

        // Build font: heading is always bold, inline styles add italic/monospace.
        let font = font_for_heading_style(style, heading_base, mono_font);
        iced_span = iced_span.font(font);

        // Set color for code spans.
        if style.code {
            iced_span = iced_span.color(Color::from_rgb(0.6, 0.3, 0.3));
        }

        if style.strikethrough {
            iced_span = iced_span.strikethrough(true);
        }

        spans.push(iced_span);
        pos = inline_span.range.end;
    }

    // Trailing text.
    if pos < display.len() {
        spans.push(text::Span::new(&display[pos..]).font(heading_base));
    }

    // Ensure at least one span for paragraph height.
    if display.is_empty() && spans.is_empty() {
        spans.push(text::Span::new("").font(heading_base));
    }

    spans
}

/// Build iced text spans from a display cache's inline spans.
///
/// Used for paragraphs (and headings when `is_heading` is false).
fn build_inline_spans<'a>(
    cache: &'a document::DisplayCache,
    base_font: Font,
    mono_font: Font,
    _effective_size: Pixels,
    _is_heading: bool,
) -> Vec<text::Span<'a, (), Font>> {
    let display = &cache.display_text;
    let mut spans = Vec::new();
    let mut pos = 0;

    for inline_span in &cache.spans {
        // Gap before this span: unstyled text.
        if inline_span.range.start > pos {
            spans.push(text::Span::new(&display[pos..inline_span.range.start]));
        }

        let style = &inline_span.style;
        let text_slice = &display[inline_span.range.clone()];

        let mut iced_span = text::Span::new(text_slice);

        // Build font based on style.
        let font = font_for_style(style, base_font, mono_font);
        if font != base_font {
            iced_span = iced_span.font(font);
        }

        // Set color for code spans.
        if style.code {
            iced_span = iced_span.color(Color::from_rgb(0.6, 0.3, 0.3));
        }

        if style.strikethrough {
            iced_span = iced_span.strikethrough(true);
        }

        spans.push(iced_span);
        pos = inline_span.range.end;
    }

    // Trailing unstyled text.
    if pos < display.len() {
        spans.push(text::Span::new(&display[pos..]));
    }

    // Ensure at least one span for paragraph height.
    if display.is_empty() && spans.is_empty() {
        spans.push(text::Span::new(""));
    }

    spans
}

/// Determine the font for a given inline style in a paragraph.
fn font_for_style(style: &SpanStyle, base: Font, mono: Font) -> Font {
    if style.code {
        return mono;
    }

    let mut font = base;
    if style.bold {
        font.weight = iced_core::font::Weight::Bold;
    }
    if style.italic {
        font.style = iced_core::font::Style::Italic;
    }
    font
}

/// Determine the font for a given inline style within a heading.
///
/// Headings are always bold, so we start from a bold base. Inline bold is
/// redundant but harmless; inline italic adds italic on top.
fn font_for_heading_style(style: &SpanStyle, heading_base: Font, mono: Font) -> Font {
    if style.code {
        // Code in a heading: monospace but keep bold weight.
        return Font {
            weight: iced_core::font::Weight::Bold,
            ..mono
        };
    }

    let mut font = heading_base;
    if style.italic {
        font.style = iced_core::font::Style::Italic;
    }
    font
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Editor<'_, Message, Theme, Renderer>
where
    Renderer: text::Renderer<Font = Font>,
{
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph>::default())
    }

    fn layout(
        &mut self,
        tree: &mut tree::Tree,
        renderer: &Renderer,
        limits: &iced_core::layout::Limits,
    ) -> iced_core::layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        iced_core::layout::sized(limits, self.width, self.height, |limits| {
            let max_width = limits.max().width;
            let content_width = max_width - self.padding.left - self.padding.right;

            // Build paragraphs for all blocks.
            state.paragraphs = self.build_paragraphs(renderer, content_width);

            // Sum up paragraph heights for total content height.
            let total_height: f32 = state.paragraphs.iter().map(|bl| bl.height).sum();

            // Use the height allocated by the layout system.
            let allocated_height = limits
                .max()
                .height
                .min(total_height + self.padding.top + self.padding.bottom);

            // Clamp scroll offset to valid range.
            let visible_height = allocated_height - self.padding.top - self.padding.bottom;
            let max_scroll = (total_height - visible_height).max(0.0);
            state.scroll_offset = state.scroll_offset.clamp(0.0, max_scroll);

            // Auto-scroll to keep the cursor block visible.
            let cursor_block = self.document.active_block();
            let layout_bounds = Rectangle {
                x: 0.0,
                y: 0.0,
                width: max_width,
                height: allocated_height,
            };
            scroll_cursor_into_view(state, cursor_block, layout_bounds, self.padding);

            Size::new(max_width, allocated_height)
        })
    }

    fn draw(
        &self,
        tree: &tree::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();

        // Clip drawing to widget bounds.
        renderer.start_layer(bounds);

        let text_color = style.text_color;
        let (cursor_block_idx, cursor_offset) = self.document.cursor();
        let text_size = self.effective_size(renderer);
        let line_height_px = self.line_height.to_absolute(text_size).0;

        let visible_top = bounds.y + self.padding.top;
        let visible_bottom = bounds.y + bounds.height - self.padding.bottom;

        for (i, bl) in state.paragraphs.iter().enumerate() {
            // Y position of this block in widget coordinates.
            let y = bounds.y + self.padding.top - state.scroll_offset + bl.y_offset;

            // Skip blocks entirely above the visible area.
            if y + bl.height < visible_top {
                continue;
            }

            // Stop once we're entirely below the visible area.
            if y > visible_bottom {
                break;
            }

            let is_active = i == cursor_block_idx && state.focus;

            match &bl.kind {
                BlockKind::ThematicBreak if i != self.document.active_block() => {
                    // Draw a horizontal rule line.
                    let line_y = y + bl.height / 2.0;
                    let line_rect = Rectangle {
                        x: bounds.x + self.padding.left,
                        y: line_y,
                        width: bounds.width - self.padding.left - self.padding.right,
                        height: 1.0,
                    };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: line_rect,
                            border: Border::default(),
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        Background::Color(Color::from_rgb(0.8, 0.8, 0.8)),
                    );
                }
                BlockKind::CodeBlock(_) if i != self.document.active_block() => {
                    // Draw code block background rectangle.
                    let bg_rect = Rectangle {
                        x: bounds.x + self.padding.left,
                        y,
                        width: bounds.width - self.padding.left - self.padding.right,
                        height: bl.height,
                    };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: bg_rect,
                            border: Border {
                                radius: 4.0.into(),
                                ..Border::default()
                            },
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        Background::Color(Color::from_rgb(
                            0xF5 as f32 / 255.0,
                            0xF5 as f32 / 255.0,
                            0xF5 as f32 / 255.0,
                        )),
                    );

                    // Draw the code text with 8px internal padding.
                    let text_position = Point::new(bounds.x + self.padding.left + 8.0, y + 8.0);
                    renderer.fill_paragraph(&bl.paragraph, text_position, text_color, *viewport);
                }
                BlockKind::Heading(1) if i != self.document.active_block() => {
                    // Draw heading text.
                    let position = Point::new(bounds.x + self.padding.left, y);
                    renderer.fill_paragraph(&bl.paragraph, position, text_color, *viewport);

                    // Draw H1 bottom border line.
                    let border_y = y + bl.height - 2.0;
                    let border_rect = Rectangle {
                        x: bounds.x + self.padding.left,
                        y: border_y,
                        width: bounds.width - self.padding.left - self.padding.right,
                        height: 1.0,
                    };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: border_rect,
                            border: Border::default(),
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        Background::Color(Color::from_rgb(0.85, 0.85, 0.85)),
                    );
                }
                _ => {
                    // Standard text rendering for paragraphs, headings, and active blocks.
                    let position = Point::new(bounds.x + self.padding.left, y);
                    renderer.fill_paragraph(&bl.paragraph, position, text_color, *viewport);
                }
            }

            // Draw cursor on the active block when focused.
            if is_active
                && let Some(grapheme_point) = bl.paragraph.grapheme_position(0, cursor_offset)
            {
                let position = Point::new(bounds.x + self.padding.left, y);
                let cursor_x = position.x + grapheme_point.x;
                let cursor_y = position.y + grapheme_point.y;

                let cursor_height = line_height_px.max(text_size.0);

                let cursor_rect = Rectangle {
                    x: cursor_x,
                    y: cursor_y,
                    width: 2.0,
                    height: cursor_height,
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: cursor_rect,
                        border: Border::default(),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(text_color),
                );
            }
        }

        renderer.end_layer();
    }

    fn update(
        &mut self,
        tree: &mut tree::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();

        let Some(on_action) = &self.on_action else {
            return;
        };

        match event {
            // Mouse wheel: scroll content.
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                let scroll_amount = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * 20.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };

                let total_height: f32 = state.paragraphs.iter().map(|bl| bl.height).sum();
                let visible_height = bounds.height - self.padding.top - self.padding.bottom;
                let max_scroll = (total_height - visible_height).max(0.0);

                state.scroll_offset = (state.scroll_offset + scroll_amount).clamp(0.0, max_scroll);

                shell.capture_event();
                shell.request_redraw();
            }

            // Mouse click: focus + position cursor.
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(click_pos) = cursor.position_in(bounds) {
                    state.focus = true;

                    // Account for scroll offset when determining which block was clicked.
                    let scrolled_y = click_pos.y - self.padding.top + state.scroll_offset;

                    // Find which block was clicked by checking y_offset ranges.
                    let mut target_block = state.paragraphs.len().saturating_sub(1);

                    for (i, bl) in state.paragraphs.iter().enumerate() {
                        if scrolled_y < bl.y_offset + bl.height {
                            target_block = i;
                            break;
                        }
                    }

                    // Determine character offset via hit_test.
                    let bl = &state.paragraphs[target_block];

                    // Compute the local point relative to where the paragraph is drawn.
                    let local_x = click_pos.x - self.padding.left;
                    let local_y = scrolled_y - bl.y_offset;

                    // Adjust for code block padding.
                    let (adj_x, adj_y) = if matches!(bl.kind, BlockKind::CodeBlock(_))
                        && target_block != self.document.active_block()
                    {
                        (local_x - 8.0, local_y - 8.0)
                    } else {
                        (local_x, local_y)
                    };

                    let local_point = Point::new(adj_x.max(0.0), adj_y.max(0.0));

                    let offset = if let Some(hit) = bl.paragraph.hit_test(local_point) {
                        hit.cursor()
                    } else {
                        // Click outside text; for active block use raw length,
                        // for inactive we approximate with display text length.
                        if target_block == self.document.active_block() {
                            self.document
                                .block(target_block)
                                .map(|b| b.raw.len())
                                .unwrap_or(0)
                        } else {
                            // When clicking an inactive block, offset will be within raw text
                            // after the block becomes active. Place at end.
                            self.document
                                .block(target_block)
                                .map(|b| b.raw.len())
                                .unwrap_or(0)
                        }
                    };

                    shell.publish(on_action(Action::Click {
                        block: target_block,
                        offset,
                    }));
                    shell.capture_event();
                    shell.request_redraw();
                } else {
                    // Click outside widget: lose focus.
                    state.focus = false;
                }
            }

            // Keyboard events when focused.
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                text: key_text,
                modifiers,
                ..
            }) if state.focus => {
                let action = match key {
                    keyboard::Key::Named(Named::Backspace) => Some(Action::Backspace),
                    keyboard::Key::Named(Named::Delete) => Some(Action::Delete),
                    keyboard::Key::Named(Named::Enter) => Some(Action::Enter),
                    keyboard::Key::Named(Named::ArrowLeft) => Some(Action::Move(Motion::Left)),
                    keyboard::Key::Named(Named::ArrowRight) => Some(Action::Move(Motion::Right)),
                    keyboard::Key::Named(Named::ArrowUp) => Some(Action::Move(Motion::Up)),
                    keyboard::Key::Named(Named::ArrowDown) => Some(Action::Move(Motion::Down)),
                    keyboard::Key::Named(Named::Home) => Some(Action::Move(Motion::Home)),
                    keyboard::Key::Named(Named::End) => Some(Action::Move(Motion::End)),
                    _ => {
                        // Text input: insert each character from the text field.
                        if !modifiers.command() {
                            if let Some(txt) = key_text {
                                let mut first_action = None;
                                for ch in txt.chars() {
                                    if !ch.is_control() || ch == '\n' {
                                        let a = Action::Insert(ch);
                                        if first_action.is_none() {
                                            first_action = Some(a);
                                        } else {
                                            shell.publish(on_action(a));
                                        }
                                    }
                                }
                                first_action
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                };

                if let Some(action) = action {
                    shell.publish(on_action(action));
                    shell.capture_event();
                    shell.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &tree::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::None
        }
    }
}

/// Adjust scroll offset so the cursor's block is visible within the viewport.
fn scroll_cursor_into_view<P: text::Paragraph>(
    state: &mut State<P>,
    cursor_block: usize,
    bounds: Rectangle,
    padding: Padding,
) {
    let visible_height = bounds.height - padding.top - padding.bottom;
    if visible_height <= 0.0 {
        return;
    }

    // Find the cursor block's layout.
    let Some(bl) = state.paragraphs.get(cursor_block) else {
        return;
    };

    let cursor_y = bl.y_offset;
    let cursor_bottom = cursor_y + bl.height;

    // If cursor block is above the visible area, scroll up.
    if cursor_y < state.scroll_offset {
        state.scroll_offset = cursor_y;
    }

    // If cursor block is below the visible area, scroll down.
    if cursor_bottom > state.scroll_offset + visible_height {
        state.scroll_offset = cursor_bottom - visible_height;
    }

    // Clamp to valid range.
    let total_height: f32 = state.paragraphs.iter().map(|bl| bl.height).sum();
    let max_scroll = (total_height - visible_height).max(0.0);
    state.scroll_offset = state.scroll_offset.clamp(0.0, max_scroll);
}

impl<'a, Message, Theme, Renderer> From<Editor<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: text::Renderer<Font = Font> + 'a,
{
    fn from(editor: Editor<'a, Message, Theme, Renderer>) -> Self {
        Self::new(editor)
    }
}
