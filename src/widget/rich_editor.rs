//! Rich text editor widget with integrated formatting model.
//!
//! This module provides a rich text editor that wraps a `rich_editor::Renderer`
//! backed by cosmic-text. All formatting and text editing go through
//! [`Content::perform`].
//!
//! Key differences from iced's text_editor:
//! - Uses our [`Content`] which wraps the rich editor (cosmic-text Editor +
//!   AttrsList formatting)
//! - No external highlighter -- formatting lives in AttrsList, always up-to-date
//! - Built-in key bindings for Cmd+B/I/U formatting shortcuts
//! - Emits our [`Action`] type instead of iced's `text_editor::Action`
use std::sync::Arc;

use crate::core::Font;
use crate::core::alignment;
use crate::core::clipboard;
use crate::core::input_method;
use crate::core::keyboard;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text::rich_editor::{self, Editor as _};
use crate::core::text::{self, LineHeight, Text, Wrapping};
use crate::core::time::{Duration, Instant};
use crate::core::widget::operation as widget_operation;
use crate::core::widget::{self, Widget};
use crate::core::window;
use crate::core::{
    Element, Event, InputMethod, Length, Padding, Pixels, Point, Rectangle, Shell, Size, Vector,
};

mod action;
mod binding;
mod content;
pub mod cursor;
pub mod list;
pub mod operation;
pub mod style;

use binding::Ime;

pub use action::{
    Action, Alignment, Cursor, Edit, Format, Line, LineEnding, Motion, Position, Selection,
};
pub use binding::{Binding, KeyPress};
pub use content::{Content, StyleRun, StyledLine};
pub use style::{Catalog, Style, StyleFn};

/// Creates a new [`RichEditor`] with the given [`Content`].
pub fn rich_editor<'a, Message, Theme, Renderer>(
    content: &'a Content<Renderer>,
) -> RichEditor<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: rich_editor::Renderer<Font = Font>,
{
    RichEditor::new(content)
}

/// A rich text editor widget with built-in formatting support.
pub struct RichEditor<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: rich_editor::Renderer,
{
    id: Option<widget::Id>,
    content: &'a Content<Renderer>,
    placeholder: Option<text::Fragment<'a>>,
    text_size: Option<Pixels>,
    line_height: LineHeight,
    width: Length,
    height: Length,
    min_height: f32,
    max_height: f32,
    padding: Padding,
    wrapping: Wrapping,
    letter_spacing: crate::core::Em,
    font_features: Vec<crate::core::font::Feature>,
    font_variations: Vec<crate::core::font::Variation>,
    default_style: rich_editor::span::Style,
    scrollable: bool,
    class: Theme::Class<'a>,
    on_action: Option<Box<dyn Fn(Action) -> Message + 'a>>,
    on_blur: Option<Message>,
    align_x: text::Alignment,
    align_y: alignment::Vertical,
    interaction: Option<mouse::Interaction>,
    #[allow(clippy::type_complexity)]
    key_binding: Option<Box<dyn Fn(KeyPress) -> Option<Binding<Message>> + 'a>>,
    last_status: Option<Status>,
}

impl<'a, Message, Theme, Renderer> RichEditor<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: rich_editor::Renderer<Font = Font>,
{
    /// Creates a new [`RichEditor`] with the given [`Content`].
    pub fn new(content: &'a Content<Renderer>) -> Self {
        Self {
            id: None,
            content,
            placeholder: None,
            text_size: None,
            line_height: LineHeight::default(),
            width: Length::Fill,
            height: Length::Shrink,
            min_height: 0.0,
            max_height: f32::INFINITY,
            padding: Padding::new(5.0),
            wrapping: Wrapping::default(),
            letter_spacing: crate::core::Em::default(),
            font_features: Vec::new(),
            font_variations: Vec::new(),
            default_style: rich_editor::span::Style::default(),
            scrollable: true,
            class: <Theme as Catalog>::default(),
            on_action: None,
            on_blur: None,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Top,
            interaction: None,
            key_binding: None,
            last_status: None,
        }
    }

    /// Sets the [`Id`](widget::Id) of the [`RichEditor`].
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the placeholder of the [`RichEditor`].
    pub fn placeholder(mut self, placeholder: impl text::IntoFragment<'a>) -> Self {
        self.placeholder = Some(placeholder.into_fragment());
        self
    }

    /// Sets the width of the [`RichEditor`].
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = Length::from(width.into());
        self
    }

    /// Sets the height of the [`RichEditor`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the minimum height of the [`RichEditor`].
    pub fn min_height(mut self, min_height: impl Into<Pixels>) -> Self {
        self.min_height = min_height.into().0;
        self
    }

    /// Sets the maximum height of the [`RichEditor`].
    pub fn max_height(mut self, max_height: impl Into<Pixels>) -> Self {
        self.max_height = max_height.into().0;
        self
    }

    /// Sets the default horizontal text alignment.
    ///
    /// Used for the placeholder and for content lines that don't have an
    /// explicit paragraph alignment set. Defaults to [`Default`](text::Alignment::Default).
    pub fn align_x(mut self, align_x: impl Into<text::Alignment>) -> Self {
        self.align_x = align_x.into();
        self
    }

    /// Sets the vertical alignment of the content within the editor bounds.
    ///
    /// Only has an effect when the editor has more space than its content
    /// (e.g. with a fixed or `Fill` height). Defaults to [`Top`](alignment::Vertical::Top).
    pub fn align_y(mut self, align_y: impl Into<alignment::Vertical>) -> Self {
        self.align_y = align_y.into();
        self
    }

    /// Sets the callback for when an action is performed.
    ///
    /// If not set, the editor is disabled.
    pub fn on_action(mut self, on_action: impl Fn(Action) -> Message + 'a) -> Self {
        self.on_action = Some(Box::new(on_action));
        self
    }

    /// Sets the message to emit when the editor loses focus.
    pub fn on_blur(mut self, on_blur: Message) -> Self {
        self.on_blur = Some(on_blur);
        self
    }

    /// Sets the mouse cursor shown when hovering over a read-only editor.
    ///
    /// By default, a read-only editor (no `on_action`) shows [`NotAllowed`](mouse::Interaction::NotAllowed).
    pub fn interaction(mut self, interaction: mouse::Interaction) -> Self {
        self.interaction = Some(interaction);
        self
    }

    /// Sets a custom key binding handler.
    ///
    /// The closure receives a [`KeyPress`] and returns an optional
    /// [`Binding`]. Return `None` to fall through to the default bindings.
    pub fn key_binding(
        mut self,
        key_binding: impl Fn(KeyPress) -> Option<Binding<Message>> + 'a,
    ) -> Self {
        self.key_binding = Some(Box::new(key_binding));
        self
    }

    /// Sets the text size of the [`RichEditor`].
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the [`LineHeight`] of the [`RichEditor`].
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    /// Sets the [`Padding`] of the [`RichEditor`].
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the [`Wrapping`] strategy of the [`RichEditor`].
    pub fn wrapping(mut self, wrapping: Wrapping) -> Self {
        self.wrapping = wrapping;
        self
    }

    /// Sets the letter spacing of the [`RichEditor`].
    pub fn letter_spacing(mut self, letter_spacing: impl Into<crate::core::Em>) -> Self {
        self.letter_spacing = letter_spacing.into();
        self
    }

    /// Sets the default font for new text.
    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.default_style.font = Some(font.into());
        self
    }

    /// Sets the default bold state for new text.
    pub fn bold(mut self, bold: bool) -> Self {
        self.default_style.bold = Some(bold);
        self
    }

    /// Sets the default italic state for new text.
    pub fn italic(mut self, italic: bool) -> Self {
        self.default_style.italic = Some(italic);
        self
    }

    /// Sets the default underline state for new text.
    pub fn underline(mut self, underline: bool) -> Self {
        self.default_style.underline = Some(underline);
        self
    }

    /// Sets the default strikethrough state for new text.
    pub fn strikethrough(mut self, strikethrough: bool) -> Self {
        self.default_style.strikethrough = Some(strikethrough);
        self
    }

    /// Sets the default text color for new text.
    pub fn color(mut self, color: impl Into<Option<crate::core::Color>>) -> Self {
        self.default_style.color = color.into();
        self
    }

    /// Sets the font features (e.g. `smcp`, `onum`).
    pub fn font_features(mut self, features: impl Into<Vec<crate::core::font::Feature>>) -> Self {
        self.font_features = features.into();
        self
    }

    /// Sets the font variations (e.g. `opsz`, `wght`).
    pub fn font_variations(
        mut self,
        variations: impl Into<Vec<crate::core::font::Variation>>,
    ) -> Self {
        self.font_variations = variations.into();
        self
    }

    /// Enable or disable automatic scrolling to keep the cursor visible.
    /// Defaults to `true`.
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    /// Sets the style of the [`RichEditor`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`RichEditor`].
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    fn input_method<'b>(
        &self,
        state: &'b State,
        _renderer: &Renderer,
        layout: Layout<'_>,
    ) -> InputMethod<&'b str> {
        let Some(Focus {
            is_window_focused: true,
            ..
        }) = &state.focus
        else {
            return InputMethod::Disabled;
        };

        let internal = self.content.0.borrow_mut();

        let text_bounds = layout.children().next().expect("content node").bounds();
        let translation = text_bounds.position() - Point::ORIGIN;

        let caret = match internal.editor.selection() {
            Selection::Caret(rect) => rect,
            Selection::Range(ranges) => ranges.first().cloned().unwrap_or_default(),
        };

        let position = caret.position() + translation;

        InputMethod::Enabled {
            cursor: Rectangle::new(position, Size::new(1.0, caret.height)),
            purpose: input_method::Purpose::Normal,
            preedit: state.preedit.as_ref().map(input_method::Preedit::as_ref),
        }
    }
}

/// The state of a [`RichEditor`].
#[derive(Debug)]
pub struct State {
    focus: Option<Focus>,
    preedit: Option<input_method::Preedit>,
    last_click: Option<mouse::Click>,
    drag_click: Option<mouse::click::Kind>,
    partial_scroll: f32,
}

#[derive(Debug, Clone)]
struct Focus {
    updated_at: Instant,
    now: Instant,
    is_window_focused: bool,
}

impl Focus {
    const CURSOR_BLINK_INTERVAL_MILLIS: u128 = 500;

    fn now() -> Self {
        let now = Instant::now();
        Self {
            updated_at: now,
            now,
            is_window_focused: true,
        }
    }

    fn is_cursor_visible(&self) -> bool {
        self.is_window_focused
            && ((self.now - self.updated_at).as_millis() / Self::CURSOR_BLINK_INTERVAL_MILLIS)
                .is_multiple_of(2)
    }
}

impl State {
    /// Returns whether the [`RichEditor`] is currently focused.
    pub fn is_focused(&self) -> bool {
        self.focus.is_some()
    }
}

impl widget_operation::Focusable for State {
    fn is_focused(&self) -> bool {
        self.focus.is_some()
    }

    fn focus(&mut self) {
        self.focus = Some(Focus::now());
    }

    fn unfocus(&mut self) {
        self.focus = None;
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for RichEditor<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: rich_editor::Renderer<Font = Font>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State {
            focus: None,
            preedit: None,
            last_click: None,
            drag_click: None,
            partial_scroll: 0.0,
        })
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let mut internal = self.content.0.borrow_mut();
        let _state = tree.state.downcast_mut::<State>();

        let font = self
            .default_style
            .font
            .unwrap_or_else(|| renderer.default_font());

        let limits = limits
            .width(self.width)
            .height(self.height)
            .min_height(self.min_height)
            .max_height(self.max_height);

        internal.default_style = self.default_style.clone();
        {
            use crate::core::text::rich_editor::Editor as _;
            internal.editor.set_scrollable(self.scrollable);
        }

        internal.editor.update(
            limits.shrink(self.padding).max(),
            font,
            self.text_size.unwrap_or_else(|| renderer.default_size()),
            self.line_height,
            self.letter_spacing,
            self.font_features.clone(),
            self.font_variations.clone(),
            self.wrapping,
            renderer.scale_factor(),
            self.default_style.clone(),
        );

        internal.editor.align_x(self.align_x);

        let min_bounds = internal.editor.min_bounds();
        let align_y = self.align_y;

        layout::positioned(
            &limits,
            self.width,
            self.height,
            self.padding,
            |limits| layout::Node::new(limits.resolve(self.width, self.height, min_bounds)),
            |content, space| {
                content.align(
                    crate::core::Alignment::Start,
                    crate::core::Alignment::from(align_y),
                    space,
                )
            },
        )
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Some(on_action) = self.on_action.as_ref() else {
            return;
        };

        let state = tree.state.downcast_mut::<State>();
        let is_redraw = matches!(event, Event::Window(window::Event::RedrawRequested(_)));

        match event {
            Event::Window(window::Event::Unfocused) => {
                if let Some(focus) = &mut state.focus {
                    focus.is_window_focused = false;
                }
            }
            Event::Window(window::Event::Focused) => {
                if let Some(focus) = &mut state.focus {
                    focus.is_window_focused = true;
                    focus.updated_at = Instant::now();
                    shell.request_redraw();
                }
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                if let Some(focus) = &mut state.focus
                    && focus.is_window_focused
                {
                    focus.now = *now;

                    let millis_until_redraw = Focus::CURSOR_BLINK_INTERVAL_MILLIS
                        - (focus.now - focus.updated_at).as_millis()
                            % Focus::CURSOR_BLINK_INTERVAL_MILLIS;

                    shell.request_redraw_at(
                        focus.now + Duration::from_millis(millis_until_redraw as u64),
                    );
                }
            }
            Event::Clipboard(clipboard::Event::Read(Ok(content))) => {
                if let clipboard::Content::Text(text) = content.as_ref()
                    && let Some(focus) = &mut state.focus
                    && focus.is_window_focused
                {
                    shell.publish(on_action(Action::Edit(Edit::Paste(Arc::new(text.clone())))));
                }
            }
            _ => {}
        }

        let content_bounds = layout.children().next().expect("content node").bounds();
        let content_offset = {
            let p = content_bounds.position() - layout.bounds().position();
            Vector::new(p.x, p.y)
        };

        if let Some(update) = Update::from_event(
            event,
            state,
            layout.bounds(),
            content_offset,
            cursor,
            self.key_binding.as_deref(),
        ) {
            match update {
                Update::Click(click) => {
                    let action = match click.kind() {
                        mouse::click::Kind::Single => Action::Click(click.position()),
                        mouse::click::Kind::Double => Action::SelectWord,
                        mouse::click::Kind::Triple => Action::SelectLine,
                    };

                    state.focus = Some(Focus::now());
                    state.last_click = Some(click);
                    state.drag_click = Some(click.kind());

                    shell.publish(on_action(action));
                    shell.capture_event();
                }
                Update::Drag(position) => {
                    shell.publish(on_action(Action::Drag(position)));
                }
                Update::Release => {
                    state.drag_click = None;
                }
                Update::Scroll(lines) => {
                    let bounds = self.content.0.borrow().editor.bounds();
                    if bounds.height >= i32::MAX as f32 {
                        return;
                    }

                    let lines = lines + state.partial_scroll;
                    state.partial_scroll = lines.fract();

                    shell.publish(on_action(Action::Scroll {
                        lines: lines as i32,
                    }));
                    shell.capture_event();
                }
                Update::InputMethod(update) => match update {
                    Ime::Toggle(is_open) => {
                        state.preedit = is_open.then(input_method::Preedit::new);
                        shell.request_redraw();
                    }
                    Ime::Preedit { content, selection } => {
                        state.preedit = Some(input_method::Preedit {
                            content,
                            selection,
                            text_size: self.text_size,
                        });
                        shell.request_redraw();
                    }
                    Ime::Commit(text) => {
                        shell.publish(on_action(Action::Edit(Edit::Paste(Arc::new(text)))));
                    }
                },
                Update::Binding(binding) => {
                    fn apply_binding<R: rich_editor::Renderer, Message>(
                        binding: Binding<Message>,
                        content: &Content<R>,
                        state: &mut State,
                        on_action: &dyn Fn(Action) -> Message,
                        on_blur: &Option<Message>,
                        shell: &mut Shell<'_, Message>,
                    ) where
                        Message: Clone,
                    {
                        let mut publish = |action| shell.publish(on_action(action));

                        match binding {
                            Binding::Unfocus => {
                                if state.focus.is_some() {
                                    state.focus = None;
                                    state.drag_click = None;
                                    if let Some(on_blur) = on_blur {
                                        shell.publish(on_blur.clone());
                                    }
                                }
                            }
                            Binding::Copy => {
                                if let Some(selection) = content.selection() {
                                    shell.write_clipboard(clipboard::Content::Text(selection));
                                }
                            }
                            Binding::Cut => {
                                if let Some(selection) = content.selection() {
                                    shell.write_clipboard(clipboard::Content::Text(selection));
                                    shell.publish(on_action(Action::Edit(Edit::Delete)));
                                }
                            }
                            Binding::Paste => {
                                shell.read_clipboard(clipboard::Kind::Text);
                            }
                            Binding::Move(motion) => {
                                publish(Action::Move(motion));
                            }
                            Binding::Select(motion) => {
                                publish(Action::Select(motion));
                            }
                            Binding::SelectWord => {
                                publish(Action::SelectWord);
                            }
                            Binding::SelectLine => {
                                publish(Action::SelectLine);
                            }
                            Binding::SelectAll => {
                                publish(Action::SelectAll);
                            }
                            Binding::Insert(c) => {
                                publish(Action::Edit(Edit::Insert(c)));
                            }
                            Binding::Enter => {
                                publish(Action::Edit(Edit::Enter));
                            }
                            Binding::Backspace => {
                                publish(Action::Edit(Edit::Backspace));
                            }
                            Binding::Delete => {
                                publish(Action::Edit(Edit::Delete));
                            }
                            Binding::Format(fmt) => {
                                publish(Action::Edit(Edit::Format(fmt)));
                            }
                            Binding::Undo => {
                                publish(Action::Undo);
                            }
                            Binding::Redo => {
                                publish(Action::Redo);
                            }
                            Binding::Sequence(sequence) => {
                                for binding in sequence {
                                    apply_binding(
                                        binding, content, state, on_action, on_blur, shell,
                                    );
                                }
                            }
                            Binding::Custom(message) => {
                                shell.publish(message);
                            }
                        }
                    }

                    if !matches!(binding, Binding::Unfocus) {
                        shell.capture_event();
                    }

                    apply_binding(
                        binding,
                        self.content,
                        state,
                        on_action,
                        &self.on_blur,
                        shell,
                    );

                    if let Some(focus) = &mut state.focus {
                        focus.updated_at = Instant::now();
                    }
                }
            }
        }

        let status = {
            let is_disabled = self.on_action.is_none();
            let is_hovered = cursor.is_over(layout.bounds());

            if is_disabled {
                Status::Disabled
            } else if state.focus.is_some() {
                Status::Focused { is_hovered }
            } else if is_hovered {
                Status::Hovered
            } else {
                Status::Active
            }
        };

        if is_redraw {
            self.last_status = Some(status);
            shell.request_input_method(&self.input_method(state, renderer, layout));
        } else if self
            .last_status
            .is_some_and(|last_status| status != last_status)
        {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();

        let internal = self.content.0.borrow();
        let state = tree.state.downcast_ref::<State>();

        let font = self
            .default_style
            .font
            .unwrap_or_else(|| renderer.default_font());

        let style = theme.style(&self.class, self.last_status.unwrap_or(Status::Active));

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                ..renderer::Quad::default()
            },
            style.background,
        );

        let text_bounds = layout.children().next().expect("content node").bounds();

        if internal.editor.is_empty() {
            if let Some(placeholder) = self.placeholder.clone() {
                renderer.fill_text(
                    Text {
                        content: placeholder.into_owned(),
                        bounds: text_bounds.size(),
                        size: self.text_size.unwrap_or_else(|| renderer.default_size()),
                        line_height: self.line_height,
                        font,
                        align_x: self.align_x,
                        align_y: self.align_y,
                        shaping: text::Shaping::Advanced,
                        wrapping: self.wrapping,
                        ellipsis: text::Ellipsis::None,
                        letter_spacing: self.letter_spacing,
                        font_features: self.font_features.clone(),
                        font_variations: self.font_variations.clone(),
                        hint_factor: renderer.scale_factor(),
                    },
                    text_bounds.position(),
                    style.placeholder,
                    text_bounds,
                );
            }
        } else {
            renderer.fill_rich_editor(
                &internal.editor,
                text_bounds.position(),
                style.value,
                text_bounds,
            );

            // Draw list markers (bullets/numbers) in the margin space
            let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
            let line_count = internal.editor.line_count();
            let list_indent = internal.list_indent;

            for line_idx in 0..line_count {
                let para_style = internal.paragraph_style(line_idx);
                let Some(ref list_style) = para_style.list else {
                    continue;
                };

                let Some(geom) = internal.editor.line_geometry(line_idx) else {
                    continue;
                };

                let ordinal = list::count_ordinal(&internal.paragraph_styles, line_idx);
                let marker = list::marker_text(list_style, ordinal);

                // x_offset already includes margin + alignment correction,
                // so place the marker one list_indent to the left of it.
                let marker_x = text_bounds.x + geom.x_offset - list_indent;
                let line_top = geom.line_top;
                let line_height = geom.line_height;

                renderer.fill_text(
                    Text {
                        content: marker,
                        bounds: Size::new(list_indent, line_height),
                        size: text_size,
                        line_height: self.line_height,
                        font,
                        align_x: text::Alignment::Center,
                        align_y: alignment::Vertical::Top,
                        shaping: text::Shaping::Advanced,
                        wrapping: Wrapping::None,
                        ellipsis: text::Ellipsis::None,
                        letter_spacing: self.letter_spacing,
                        font_features: self.font_features.clone(),
                        font_variations: self.font_variations.clone(),
                        hint_factor: renderer.scale_factor(),
                    },
                    Point::new(marker_x, text_bounds.y + line_top),
                    style.value,
                    bounds,
                );
            }
        }

        let translation = text_bounds.position() - Point::ORIGIN;

        // Draw selection ranges even when unfocused
        match internal.editor.selection() {
            Selection::Range(ranges) => {
                for range in ranges
                    .into_iter()
                    .filter_map(|range| text_bounds.intersection(&(range + translation)))
                {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: range,
                            ..renderer::Quad::default()
                        },
                        style.selection,
                    );
                }
            }
            Selection::Caret(caret) => {
                // Only draw cursor caret when focused and visible
                if let Some(focus) = state.focus.as_ref()
                    && focus.is_cursor_visible()
                {
                    let cursor = Rectangle::new(
                        caret.position() + translation,
                        Size::new(
                            if renderer::CRISP {
                                (1.0 / renderer.scale_factor().unwrap_or(1.0)).max(1.0)
                            } else {
                                caret.width
                            },
                            caret.height,
                        ),
                    );

                    if let Some(clipped_cursor) = text_bounds.intersection(&cursor) {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: clipped_cursor,
                                ..renderer::Quad::default()
                            },
                            style.value,
                        );
                    }
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let is_disabled = self.on_action.is_none();

        if cursor.is_over(layout.bounds()) {
            if is_disabled {
                self.interaction.unwrap_or(mouse::Interaction::NotAllowed)
            } else {
                mouse::Interaction::Text
            }
        } else {
            mouse::Interaction::default()
        }
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();
        operation.focusable(self.id.as_ref(), layout.bounds(), state);
    }
}

enum Update<Message> {
    Click(mouse::Click),
    Drag(Point),
    Release,
    Scroll(f32),
    InputMethod(Ime),
    Binding(Binding<Message>),
}

impl<Message> Update<Message> {
    fn from_event(
        event: &Event,
        state: &State,
        bounds: Rectangle,
        content_offset: Vector,
        cursor: mouse::Cursor,
        key_binding: Option<&dyn Fn(KeyPress) -> Option<Binding<Message>>>,
    ) -> Option<Self> {
        let binding = |binding| Some(Self::Binding(binding));

        match event {
            Event::Mouse(event) => match event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(cursor_position) = cursor.position_in(bounds) {
                        let cursor_position = cursor_position - content_offset;
                        let click = mouse::Click::new(
                            cursor_position,
                            mouse::Button::Left,
                            state.last_click,
                        );
                        Some(Self::Click(click))
                    } else if state.focus.is_some() {
                        binding(Binding::Unfocus)
                    } else {
                        None
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => Some(Self::Release),
                mouse::Event::CursorMoved { .. } => match state.drag_click {
                    Some(mouse::click::Kind::Single) => {
                        let cursor_position = cursor.position_in(bounds)? - content_offset;
                        Some(Self::Drag(cursor_position))
                    }
                    _ => None,
                },
                mouse::Event::WheelScrolled { delta } if cursor.is_over(bounds) => {
                    Some(Self::Scroll(match delta {
                        mouse::ScrollDelta::Lines { y, .. } => {
                            if y.abs() > 0.0 {
                                y.signum() * -(y.abs() * 4.0).max(1.0)
                            } else {
                                0.0
                            }
                        }
                        mouse::ScrollDelta::Pixels { y, .. } => -y / 4.0,
                    }))
                }
                _ => None,
            },
            Event::InputMethod(event) => match event {
                input_method::Event::Opened | input_method::Event::Closed => Some(
                    Self::InputMethod(Ime::Toggle(matches!(event, input_method::Event::Opened))),
                ),
                input_method::Event::Preedit(content, selection) if state.focus.is_some() => {
                    Some(Self::InputMethod(Ime::Preedit {
                        content: content.clone(),
                        selection: selection.clone(),
                    }))
                }
                input_method::Event::Commit(content) if state.focus.is_some() => {
                    Some(Self::InputMethod(Ime::Commit(content.clone())))
                }
                _ => None,
            },
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modified_key,
                physical_key,
                modifiers,
                text,
                ..
            }) => {
                let status = if state.focus.is_some() {
                    Status::Focused {
                        is_hovered: cursor.is_over(bounds),
                    }
                } else {
                    Status::Active
                };

                let key_press = KeyPress {
                    key: key.clone(),
                    modified_key: modified_key.clone(),
                    physical_key: *physical_key,
                    modifiers: *modifiers,
                    text: text.clone(),
                    status,
                };

                key_binding
                    .and_then(|f| f(key_press.clone()))
                    .or_else(|| Binding::from_key_press(key_press))
                    .map(Self::Binding)
            }
            _ => None,
        }
    }
}

impl<'a, Message, Theme, Renderer> From<RichEditor<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: rich_editor::Renderer<Font = Font>,
{
    fn from(editor: RichEditor<'a, Message, Theme, Renderer>) -> Self {
        Self::new(editor)
    }
}

/// The possible status of a [`RichEditor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The editor can be interacted with.
    Active,
    /// The editor is being hovered.
    Hovered,
    /// The editor is focused.
    Focused {
        /// Whether the editor is hovered while focused.
        is_hovered: bool,
    },
    /// The editor cannot be interacted with.
    Disabled,
}
