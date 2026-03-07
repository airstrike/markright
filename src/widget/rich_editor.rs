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

mod action;
mod binding;
mod content;
pub mod cursor;
mod operation;
pub mod style;

pub use crate::core::text::Alignment;
pub use action::{
    Action, Cursor, Edit, FormatAction, Line, LineEnding, Motion, Position, Selection,
};
pub use binding::{Binding, KeyPress};
pub use content::Content;
pub use style::{Catalog, Style, StyleFn};

use crate::core::Font;
use crate::core::alignment;
use crate::core::clipboard;
use crate::core::input_method;
use crate::core::keyboard;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text::editor::Selection as EditorSelection;
use crate::core::text::rich_editor::{self, Editor as RichEditorTrait};
use crate::core::text::{self, LineHeight, Text, Wrapping};
use crate::core::time::{Duration, Instant};
use crate::core::widget::operation as widget_operation;
use crate::core::widget::{self, Widget};
use crate::core::window;
use crate::core::{
    Element, Event, InputMethod, Length, Padding, Pixels, Point, Rectangle, Shell, Size, Vector,
};

use std::sync::Arc;

use action::Edit as ActionEdit;
use binding::{Binding as BindingType, Ime};

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
    font: Option<Renderer::Font>,
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
    class: Theme::Class<'a>,
    on_action: Option<Box<dyn Fn(Action) -> Message + 'a>>,
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
            font: None,
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
            class: <Theme as Catalog>::default(),
            on_action: None,
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

    /// Sets the callback for when an action is performed.
    ///
    /// If not set, the editor is disabled.
    pub fn on_action(mut self, on_action: impl Fn(Action) -> Message + 'a) -> Self {
        self.on_action = Some(Box::new(on_action));
        self
    }

    /// Sets the [`Font`] of the [`RichEditor`].
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
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
        renderer: &Renderer,
        layout: Layout<'_>,
    ) -> InputMethod<&'b str> {
        let Some(Focus {
            is_window_focused: true,
            ..
        }) = &state.focus
        else {
            return InputMethod::Disabled;
        };

        let bounds = layout.bounds();
        let internal = self.content.0.borrow_mut();

        let text_bounds = bounds.shrink(self.padding);
        let translation = text_bounds.position() - Point::ORIGIN;

        let cursor = match internal.editor.selection() {
            EditorSelection::Caret(position) => position,
            EditorSelection::Range(ranges) => {
                ranges.first().cloned().unwrap_or_default().position()
            }
        };

        let base_size: f32 = self
            .text_size
            .unwrap_or_else(|| renderer.default_size())
            .into();
        let logical = internal.editor.cursor();
        let style = internal
            .editor
            .style_at(logical.position.line, logical.position.column);
        let effective_size = style.size.unwrap_or(base_size);

        let line_height = self.line_height.to_absolute(Pixels(effective_size));

        let position = cursor + translation;

        InputMethod::Enabled {
            cursor: Rectangle::new(position, Size::new(1.0, f32::from(line_height))),
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

        let font = self.font.unwrap_or_else(|| renderer.default_font());

        let limits = limits
            .width(self.width)
            .height(self.height)
            .min_height(self.min_height)
            .max_height(self.max_height);

        // Update the editor layout. No highlighter parameter needed --
        // formatting is stored directly in AttrsList.
        internal.editor.update(
            limits.shrink(self.padding).max(),
            font,
            self.text_size.unwrap_or_else(|| renderer.default_size()),
            self.line_height,
            self.letter_spacing,
            self.font_features.clone(),
            self.wrapping,
            renderer.scale_factor(),
        );

        match self.height {
            Length::Fill | Length::FillPortion(_) | Length::Fixed(_) => {
                layout::Node::new(limits.max())
            }
            Length::Shrink => {
                let min_bounds = internal.editor.min_bounds();
                layout::Node::new(
                    limits
                        .height(min_bounds.height)
                        .max()
                        .expand(Size::new(0.0, self.padding.y())),
                )
            }
        }
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
                    shell.publish(on_action(Action::Edit(ActionEdit::Paste(Arc::new(
                        text.clone(),
                    )))));
                }
            }
            _ => {}
        }

        if let Some(update) =
            Update::from_event(event, state, layout.bounds(), self.padding, cursor)
        {
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
                        shell.publish(on_action(Action::Edit(ActionEdit::Paste(Arc::new(text)))));
                    }
                },
                Update::Binding(binding) => {
                    fn apply_binding<R: rich_editor::Renderer, Message>(
                        binding: BindingType<Message>,
                        content: &Content<R>,
                        state: &mut State,
                        on_action: &dyn Fn(Action) -> Message,
                        shell: &mut Shell<'_, Message>,
                    ) {
                        let mut publish = |action| shell.publish(on_action(action));

                        match binding {
                            BindingType::Unfocus => {
                                state.focus = None;
                                state.drag_click = None;
                            }
                            BindingType::Copy => {
                                if let Some(selection) = content.selection() {
                                    shell.write_clipboard(clipboard::Content::Text(selection));
                                }
                            }
                            BindingType::Cut => {
                                if let Some(selection) = content.selection() {
                                    shell.write_clipboard(clipboard::Content::Text(selection));
                                    shell.publish(on_action(Action::Edit(ActionEdit::Delete)));
                                }
                            }
                            BindingType::Paste => {
                                shell.read_clipboard(clipboard::Kind::Text);
                            }
                            BindingType::Move(motion) => {
                                publish(Action::Move(motion));
                            }
                            BindingType::Select(motion) => {
                                publish(Action::Select(motion));
                            }
                            BindingType::SelectWord => {
                                publish(Action::SelectWord);
                            }
                            BindingType::SelectLine => {
                                publish(Action::SelectLine);
                            }
                            BindingType::SelectAll => {
                                publish(Action::SelectAll);
                            }
                            BindingType::Insert(c) => {
                                publish(Action::Edit(ActionEdit::Insert(c)));
                            }
                            BindingType::Enter => {
                                publish(Action::Edit(ActionEdit::Enter));
                            }
                            BindingType::Backspace => {
                                publish(Action::Edit(ActionEdit::Backspace));
                            }
                            BindingType::Delete => {
                                publish(Action::Edit(ActionEdit::Delete));
                            }
                            BindingType::Format(fmt) => {
                                publish(Action::Edit(ActionEdit::Format(fmt)));
                            }
                            BindingType::Undo => {
                                publish(Action::Undo);
                            }
                            BindingType::Redo => {
                                publish(Action::Redo);
                            }
                            BindingType::Sequence(sequence) => {
                                for binding in sequence {
                                    apply_binding(binding, content, state, on_action, shell);
                                }
                            }
                            BindingType::Custom(message) => {
                                shell.publish(message);
                            }
                        }
                    }

                    if !matches!(binding, BindingType::Unfocus) {
                        shell.capture_event();
                    }

                    apply_binding(binding, self.content, state, on_action, shell);

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

        let internal = self.content.0.borrow_mut();
        let state = tree.state.downcast_ref::<State>();

        let font = self.font.unwrap_or_else(|| renderer.default_font());

        let style = theme.style(&self.class, self.last_status.unwrap_or(Status::Active));

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                ..renderer::Quad::default()
            },
            style.background,
        );

        let text_bounds = bounds.shrink(self.padding);

        if internal.editor.is_empty() {
            if let Some(placeholder) = self.placeholder.clone() {
                renderer.fill_text(
                    Text {
                        content: placeholder.into_owned(),
                        bounds: text_bounds.size(),
                        size: self.text_size.unwrap_or_else(|| renderer.default_size()),
                        line_height: self.line_height,
                        font,
                        align_x: text::Alignment::Default,
                        align_y: alignment::Vertical::Top,
                        shaping: text::Shaping::Advanced,
                        wrapping: self.wrapping,
                        ellipsis: text::Ellipsis::None,
                        letter_spacing: self.letter_spacing,
                        font_features: self.font_features.clone(),
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
        }

        let translation = text_bounds.position() - Point::ORIGIN;

        // Draw selection ranges even when unfocused
        match internal.editor.selection() {
            EditorSelection::Range(ranges) => {
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
            EditorSelection::Caret(position) => {
                // Only draw cursor caret when focused and visible
                if let Some(focus) = state.focus.as_ref()
                    && focus.is_cursor_visible()
                {
                    let base_size: f32 = self
                        .text_size
                        .unwrap_or_else(|| renderer.default_size())
                        .into();
                    let logical = internal.editor.cursor();
                    let char_style = internal
                        .editor
                        .style_at(logical.position.line, logical.position.column);
                    let effective_size = char_style.size.unwrap_or(base_size);

                    let cursor = Rectangle::new(
                        position + translation,
                        Size::new(
                            if renderer::CRISP {
                                (1.0 / renderer.scale_factor().unwrap_or(1.0)).max(1.0)
                            } else {
                                1.0
                            },
                            self.line_height.to_absolute(Pixels(effective_size)).into(),
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
                mouse::Interaction::NotAllowed
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
    Binding(BindingType<Message>),
}

impl<Message> Update<Message> {
    fn from_event(
        event: &Event,
        state: &State,
        bounds: Rectangle,
        padding: Padding,
        cursor: mouse::Cursor,
    ) -> Option<Self> {
        let binding = |binding| Some(Self::Binding(binding));

        match event {
            Event::Mouse(event) => match event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(cursor_position) = cursor.position_in(bounds) {
                        let cursor_position =
                            cursor_position - Vector::new(padding.left, padding.top);
                        let click = mouse::Click::new(
                            cursor_position,
                            mouse::Button::Left,
                            state.last_click,
                        );
                        Some(Self::Click(click))
                    } else if state.focus.is_some() {
                        binding(BindingType::Unfocus)
                    } else {
                        None
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => Some(Self::Release),
                mouse::Event::CursorMoved { .. } => match state.drag_click {
                    Some(mouse::click::Kind::Single) => {
                        let cursor_position =
                            cursor.position_in(bounds)? - Vector::new(padding.left, padding.top);
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

                BindingType::from_key_press(key_press).map(Self::Binding)
            }
            _ => None,
        }
    }
}

impl<'a, Message, Theme, Renderer> From<RichEditor<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
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
