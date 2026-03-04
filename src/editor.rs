// WYSIWYG markdown editor widget for iced.

use crate::content;
use crate::parse;

use iced_core::alignment;
use iced_core::keyboard;
use iced_core::keyboard::key::Named;
use iced_core::mouse;
use iced_core::renderer;
use iced_core::text;
use iced_core::text::Paragraph as _;
use iced_core::widget::tree;
use iced_core::{
    Background, Color, Element, Event, Font, Layout, Length, Padding, Pixels, Point, Rectangle,
    Shadow, Shell, Size, Widget,
};

/// A WYSIWYG markdown editor widget.
///
/// The active line (where the cursor sits) shows raw markdown text.
/// All other lines show formatted display text with markers hidden.
pub struct Editor<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer,
{
    content: &'a content::Content,
    on_action: Option<Box<dyn Fn(content::Action) -> Message + 'a>>,
    font: Option<Renderer::Font>,
    size: Option<Pixels>,
    line_height: text::LineHeight,
    width: Length,
    height: Length,
    padding: Padding,
    _theme: std::marker::PhantomData<Theme>,
}

/// Internal widget state, stored in the iced widget tree.
struct State<P: text::Paragraph> {
    paragraphs: Vec<P>,
    focus: bool,
    last_active_line: Option<usize>,
    scroll_offset: f32,
}

impl<P: text::Paragraph> Default for State<P> {
    fn default() -> Self {
        Self {
            paragraphs: Vec::new(),
            focus: false,
            last_active_line: None,
            scroll_offset: 0.0,
        }
    }
}

/// Create an [`Editor`] widget for the given content.
pub fn editor<'a, Message, Theme, Renderer>(
    content: &'a content::Content,
) -> Editor<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer<Font = Font>,
{
    Editor::new(content)
}

impl<'a, Message, Theme, Renderer> Editor<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer<Font = Font>,
{
    /// Create a new editor widget referencing the given content.
    pub fn new(content: &'a content::Content) -> Self {
        Self {
            content,
            on_action: None,
            font: None,
            size: None,
            line_height: text::LineHeight::default(),
            width: Length::Fill,
            height: Length::Fill,
            padding: Padding::new(5.0),
            _theme: std::marker::PhantomData,
        }
    }

    /// Set the callback for editor actions.
    pub fn on_action(mut self, f: impl Fn(content::Action) -> Message + 'a) -> Self {
        self.on_action = Some(Box::new(f));
        self
    }

    /// Set the font used for text rendering.
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
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

    /// Build paragraphs for all lines in the content.
    ///
    /// The active line (cursor line) is rendered as raw text.
    /// Other lines are rendered as formatted spans with markers hidden.
    fn build_paragraphs(&self, renderer: &Renderer, bounds_width: f32) -> Vec<Renderer::Paragraph> {
        let template = self.text_template(renderer, bounds_width);
        let base_font = self.effective_font(renderer);
        let active_line = self.content.cursor().0;

        let mut in_code_block = false;
        let mut paragraphs = Vec::with_capacity(self.content.line_count());

        for i in 0..self.content.line_count() {
            let raw = self.content.line(i).unwrap_or("");

            if i == active_line {
                // Active line: show raw markdown text.
                // Still track code block state for subsequent lines.
                parse::parse_line(raw, &mut in_code_block);

                let para = Renderer::Paragraph::with_text(template.with_content(raw));
                paragraphs.push(para);
            } else {
                // Non-active line: show formatted display text.
                let parsed = parse::parse_line(raw, &mut in_code_block);

                if parsed.spans.is_empty() {
                    // Plain text, no formatting
                    let para = Renderer::Paragraph::with_text(
                        template.with_content(parsed.display.as_str()),
                    );
                    paragraphs.push(para);
                } else {
                    let effective_size = self.effective_size(renderer);
                    let iced_spans = build_iced_spans(&parsed, base_font, effective_size);
                    let para = Renderer::Paragraph::with_spans(
                        template.with_content(iced_spans.as_slice()),
                    );
                    paragraphs.push(para);
                }
            }
        }

        paragraphs
    }
}

/// Convert a parsed line into a vec of iced text spans.
///
/// Gaps between parse spans get default styling. Each parse span
/// gets font/color adjusted based on its style. `effective_size` is
/// the widget's resolved text size, used to scale heading spans.
fn build_iced_spans<'a>(
    parsed: &'a parse::Line,
    base_font: Font,
    effective_size: Pixels,
) -> Vec<text::Span<'a, (), Font>> {
    let display = &parsed.display;
    let mut spans = Vec::new();
    let mut pos = 0;

    for parse_span in &parsed.spans {
        // Gap before this span: unstyled text
        if parse_span.range.start > pos {
            spans.push(text::Span::new(&display[pos..parse_span.range.start]));
        }

        let style = &parse_span.style;
        let text_slice = &display[parse_span.range.clone()];

        let mut iced_span = text::Span::new(text_slice);

        // Build font based on style
        let font = font_for_style(style, base_font);
        if font != base_font {
            iced_span = iced_span.font(font);
        }

        // Set color based on style
        if style.code {
            iced_span = iced_span.color(Color::from_rgb(0.6, 0.3, 0.3));
        } else if style.heading.is_some() {
            iced_span = iced_span.color(Color::from_rgb(0.1, 0.1, 0.5));
        }

        // Headings get larger size
        if let Some(level) = style.heading {
            let scale = match level {
                1 => 2.0_f32,
                2 => 1.5,
                3 => 1.25,
                4 => 1.1,
                _ => 1.0,
            };
            if scale > 1.0 {
                iced_span = iced_span.size(Pixels(effective_size.0 * scale));
            }
        }

        spans.push(iced_span);
        pos = parse_span.range.end;
    }

    // Trailing unstyled text
    if pos < display.len() {
        spans.push(text::Span::new(&display[pos..]));
    }

    // If display is empty, push at least one empty span so the paragraph has height
    if display.is_empty() && spans.is_empty() {
        spans.push(text::Span::new(""));
    }

    spans
}

/// Determine the font for a given parse style.
fn font_for_style(style: &parse::Style, base: Font) -> Font {
    let mut font = base;

    if style.bold {
        font.weight = iced_core::font::Weight::Bold;
    }
    if style.italic {
        font.style = iced_core::font::Style::Italic;
    }
    if style.code {
        font.family = iced_core::font::Family::Monospace;
    }
    if style.heading.is_some() {
        font.weight = iced_core::font::Weight::Bold;
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

            // Build paragraphs for all lines
            state.paragraphs = self.build_paragraphs(renderer, content_width);
            state.last_active_line = Some(self.content.cursor().0);

            // Sum up paragraph heights for total content height
            let total_height: f32 = state.paragraphs.iter().map(|p| p.min_bounds().height).sum();

            // Use the height allocated by the layout system. When height is
            // Length::Fill the max height is the available space; when
            // Length::Shrink the max is unconstrained and we use content height.
            let allocated_height = limits
                .max()
                .height
                .min(total_height + self.padding.top + self.padding.bottom);

            // Clamp scroll offset to valid range
            let visible_height = allocated_height - self.padding.top - self.padding.bottom;
            let max_scroll = (total_height - visible_height).max(0.0);
            state.scroll_offset = state.scroll_offset.clamp(0.0, max_scroll);

            // Auto-scroll to keep the cursor line visible
            let cursor_line = self.content.cursor().0;
            let layout_bounds = Rectangle {
                x: 0.0,
                y: 0.0,
                width: max_width,
                height: allocated_height,
            };
            scroll_cursor_into_view(state, cursor_line, layout_bounds, self.padding);

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

        // Clip drawing to widget bounds
        renderer.start_layer(bounds);

        let text_color = style.text_color;
        let (cursor_line, cursor_offset) = self.content.cursor();
        let text_size = self.effective_size(renderer);
        let line_height_px = self.line_height.to_absolute(text_size).0;

        let visible_top = bounds.y + self.padding.top;
        let visible_bottom = bounds.y + bounds.height - self.padding.bottom;

        // Start Y at the content origin, offset by scroll position
        let mut y = bounds.y + self.padding.top - state.scroll_offset;

        for (i, paragraph) in state.paragraphs.iter().enumerate() {
            let para_height = paragraph.min_bounds().height;

            // Skip paragraphs entirely above the visible area
            if y + para_height < visible_top {
                y += para_height;
                continue;
            }

            // Stop once we're entirely below the visible area
            if y > visible_bottom {
                break;
            }

            let position = Point::new(bounds.x + self.padding.left, y);

            // Draw the paragraph text
            renderer.fill_paragraph(paragraph, position, text_color, *viewport);

            // Draw cursor on active line when focused
            if state.focus && i == cursor_line {
                // Active line shows raw text, so cursor_offset is a raw byte offset.
                if let Some(grapheme_point) = paragraph.grapheme_position(0, cursor_offset) {
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
                            border: iced_core::Border::default(),
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        Background::Color(text_color),
                    );
                }
            }

            y += para_height;
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
            // --- Mouse wheel: scroll content ---
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                let scroll_amount = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * 20.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };

                let total_height: f32 =
                    state.paragraphs.iter().map(|p| p.min_bounds().height).sum();
                let visible_height = bounds.height - self.padding.top - self.padding.bottom;
                let max_scroll = (total_height - visible_height).max(0.0);

                state.scroll_offset = (state.scroll_offset + scroll_amount).clamp(0.0, max_scroll);

                shell.capture_event();
                shell.request_redraw();
            }

            // --- Mouse click: focus + position cursor ---
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(click_pos) = cursor.position_in(bounds) {
                    state.focus = true;

                    // Account for scroll offset when determining which line was clicked
                    let scrolled_y = click_pos.y + state.scroll_offset;

                    // Find which line was clicked by walking paragraph heights
                    let mut y_acc = self.padding.top;
                    let mut target_line = state.paragraphs.len().saturating_sub(1);
                    let cursor_line = self.content.cursor().0;

                    for (i, paragraph) in state.paragraphs.iter().enumerate() {
                        let para_height = paragraph.min_bounds().height;
                        if scrolled_y < y_acc + para_height {
                            target_line = i;
                            break;
                        }
                        y_acc += para_height;
                    }

                    // Determine character offset via hit_test
                    let local_point =
                        Point::new(click_pos.x - self.padding.left, scrolled_y - y_acc);

                    let raw_offset = if let Some(paragraph) = state.paragraphs.get(target_line) {
                        if let Some(hit) = paragraph.hit_test(local_point) {
                            let char_offset = hit.cursor();

                            if target_line == cursor_line {
                                // Active line shows raw text; offset is already raw
                                char_offset
                            } else {
                                // Non-active line shows display text; convert to raw
                                convert_display_to_raw(self.content, target_line, char_offset)
                            }
                        } else {
                            // Click outside text; place at end of line
                            self.content.line(target_line).unwrap_or("").len()
                        }
                    } else {
                        0
                    };

                    shell.publish(on_action(content::Action::Click {
                        line: target_line,
                        offset: raw_offset,
                    }));
                    shell.capture_event();
                    shell.request_redraw();
                } else {
                    // Click outside widget: lose focus
                    state.focus = false;
                }
            }

            // --- Keyboard events when focused ---
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                text: key_text,
                modifiers,
                ..
            }) if state.focus => {
                let action = match key {
                    keyboard::Key::Named(Named::Backspace) => Some(content::Action::Backspace),
                    keyboard::Key::Named(Named::Delete) => Some(content::Action::Delete),
                    keyboard::Key::Named(Named::Enter) => Some(content::Action::Enter),
                    keyboard::Key::Named(Named::ArrowLeft) => {
                        Some(content::Action::Move(content::Motion::Left))
                    }
                    keyboard::Key::Named(Named::ArrowRight) => {
                        Some(content::Action::Move(content::Motion::Right))
                    }
                    keyboard::Key::Named(Named::ArrowUp) => {
                        Some(content::Action::Move(content::Motion::Up))
                    }
                    keyboard::Key::Named(Named::ArrowDown) => {
                        Some(content::Action::Move(content::Motion::Down))
                    }
                    keyboard::Key::Named(Named::Home) => {
                        Some(content::Action::Move(content::Motion::Home))
                    }
                    keyboard::Key::Named(Named::End) => {
                        Some(content::Action::Move(content::Motion::End))
                    }
                    _ => {
                        // Text input: insert each character from the text field
                        if !modifiers.command() {
                            if let Some(txt) = key_text {
                                let mut first_action = None;
                                for ch in txt.chars() {
                                    if !ch.is_control() || ch == '\n' {
                                        let a = content::Action::Insert(ch);
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

/// Adjust scroll offset so the cursor line is visible within the viewport.
///
/// Computes the Y range of the cursor's paragraph and scrolls up or down
/// as needed to keep it fully on screen.
fn scroll_cursor_into_view<P: text::Paragraph>(
    state: &mut State<P>,
    cursor_line: usize,
    bounds: Rectangle,
    padding: Padding,
) {
    let visible_height = bounds.height - padding.top - padding.bottom;
    if visible_height <= 0.0 {
        return;
    }

    // Compute Y position of the cursor line's paragraph
    let mut cursor_y = 0.0_f32;
    let mut cursor_para_height = 0.0_f32;
    for (i, paragraph) in state.paragraphs.iter().enumerate() {
        let h = paragraph.min_bounds().height;
        if i == cursor_line {
            cursor_para_height = h;
            break;
        }
        cursor_y += h;
    }

    let cursor_bottom = cursor_y + cursor_para_height;

    // If cursor is above the visible area, scroll up
    if cursor_y < state.scroll_offset {
        state.scroll_offset = cursor_y;
    }

    // If cursor is below the visible area, scroll down
    if cursor_bottom > state.scroll_offset + visible_height {
        state.scroll_offset = cursor_bottom - visible_height;
    }

    // Clamp to valid range
    let total_height: f32 = state.paragraphs.iter().map(|p| p.min_bounds().height).sum();
    let max_scroll = (total_height - visible_height).max(0.0);
    state.scroll_offset = state.scroll_offset.clamp(0.0, max_scroll);
}

/// Convert a display-text byte offset to a raw-text byte offset for a given line.
///
/// This re-parses all lines up to and including the target line to track
/// code-block state, then uses the resulting offset map for conversion.
fn convert_display_to_raw(
    content: &content::Content,
    target_line: usize,
    display_offset: usize,
) -> usize {
    let mut in_code_block = false;
    for j in 0..target_line {
        let line_raw = content.line(j).unwrap_or("");
        parse::parse_line(line_raw, &mut in_code_block);
    }
    let raw_line = content.line(target_line).unwrap_or("");
    let parsed = parse::parse_line(raw_line, &mut in_code_block);
    parsed.offset_map.display_to_raw(display_offset)
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
