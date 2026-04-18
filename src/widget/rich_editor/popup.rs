//! Popup overlay types for the rich editor.

use std::ops::Range;

use iced_widget::text_input;

use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::text::{self, LineHeight, Paragraph as _, Text};
use crate::core::widget::{self, Widget};
use crate::core::{
    Background, Border, Element, Em, Event, Length, Padding, Pixels, Rectangle, Shell, Size,
    Vector, alignment,
};

/// A region of text that triggers a popup overlay when the cursor is near it.
///
/// Owns its `value` and `placeholder` strings so the application can store
/// a `Vec<Span>` on its own state without lifetime self-referencing.
#[derive(Clone, Debug)]
pub struct Span {
    pub line: usize,
    pub range: Range<usize>,
    /// The current text shown in the popup's text input.
    pub value: String,
    pub placeholder: String,
    pub background: Option<Background>,
    pub border: Border,
    /// When `true`, the editor blocks character insertion inside this
    /// span and treats Backspace/Delete at the span boundary as
    /// whole-span deletion (emitting [`Action::Delete`]).
    pub atomic: bool,
}

/// Identifies a popup span by line and range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanRef {
    pub line: usize,
    pub range: Range<usize>,
}

impl From<&Span> for SpanRef {
    fn from(span: &Span) -> Self {
        Self {
            line: span.line,
            range: span.range.clone(),
        }
    }
}

/// An action produced by the popup overlay.
#[derive(Clone, Debug)]
pub enum Action {
    /// The text input value changed.
    Input { span: SpanRef, value: String },
    /// User pressed Enter to confirm.
    Confirm { span: SpanRef },
    /// User pressed Escape to dismiss; `original` is the value when the
    /// popup first opened, for revert.
    Dismiss { span: SpanRef, original: String },
    /// The user deleted an atomic span (Backspace/Delete at boundary).
    Delete { span: SpanRef },
}

/// Widget ID for the popup's text input.
pub const INPUT_ID: &str = "markright-popup-input";

/// Creates a self-sizing text input for use inside a popup overlay.
///
/// Unlike [`iced::widget::text_input`], the returned builder wraps the
/// underlying `TextInput` in a transparent widget that measures `value`
/// via [`Paragraph::with_text`] and constrains the inner `TextInput`'s
/// width to that measured size (plus chrome), clamped to the available
/// overlay width. The enclosing popup container can then use
/// `Length::Shrink` and actually shrink-wrap around the input.
///
/// The builder mirrors `iced::widget::text_input`'s chainable API. If
/// the caller sets an explicit [`width`] via [`Input::width`], the
/// auto-sizing is disabled and the raw `TextInput` is returned instead.
///
/// [`Paragraph::with_text`]: crate::core::text::Paragraph::with_text
/// [`width`]: Input::width
pub fn input<'a, Message, Theme, Renderer>(
    placeholder: &'a str,
    value: &'a str,
) -> Input<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    let inner = iced_widget::TextInput::new(placeholder, value).id(INPUT_ID);

    Input {
        inner,
        value,
        text_size: None,
        font: None,
        padding: text_input::DEFAULT_PADDING,
        line_height: LineHeight::default(),
        width_set: false,
    }
}

/// A self-sizing text input wrapper returned by [`input`].
///
/// Chain the usual `text_input` methods on it; converting into an
/// [`Element`] yields a transparent wrapper that measures the content
/// at layout time and sizes the inner input accordingly.
pub struct Input<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    inner: iced_widget::TextInput<'a, Message, Theme, Renderer>,
    value: &'a str,
    text_size: Option<Pixels>,
    font: Option<Renderer::Font>,
    padding: Padding,
    line_height: LineHeight,
    width_set: bool,
}

impl<'a, Message, Theme, Renderer> Input<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: text_input::Catalog,
    Renderer: text::Renderer,
{
    /// Sets the [`widget::Id`] of the inner text input.
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.inner = self.inner.id(id);
        self
    }

    /// See [`iced::widget::text_input::TextInput::on_input`].
    pub fn on_input(mut self, on_input: impl Fn(String) -> Message + 'a) -> Self {
        self.inner = self.inner.on_input(on_input);
        self
    }

    /// See [`iced::widget::text_input::TextInput::on_submit`].
    pub fn on_submit(mut self, message: Message) -> Self {
        self.inner = self.inner.on_submit(message);
        self
    }

    /// See [`iced::widget::text_input::TextInput::on_paste`].
    pub fn on_paste(mut self, on_paste: impl Fn(String) -> Message + 'a) -> Self {
        self.inner = self.inner.on_paste(on_paste);
        self
    }

    /// See [`iced::widget::text_input::TextInput::secure`].
    pub fn secure(mut self, is_secure: bool) -> Self {
        self.inner = self.inner.secure(is_secure);
        self
    }

    /// See [`iced::widget::text_input::TextInput::icon`].
    pub fn icon(mut self, icon: text_input::Icon<Renderer::Font>) -> Self {
        self.inner = self.inner.icon(icon);
        self
    }

    /// See [`iced::widget::text_input::TextInput::style`].
    #[must_use]
    pub fn style(
        mut self,
        style: impl Fn(&Theme, text_input::Status) -> text_input::Style + 'a,
    ) -> Self
    where
        Theme::Class<'a>: From<text_input::StyleFn<'a, Theme>>,
    {
        self.inner = self.inner.style(style);
        self
    }

    /// See [`iced::widget::text_input::TextInput::class`].
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.inner = self.inner.class(class);
        self
    }

    /// Sets the text size. Tracked internally for measurement and
    /// forwarded to the inner text input.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        let px = size.into();
        self.text_size = Some(px);
        self.inner = self.inner.size(px);
        self
    }

    /// Sets the font. Tracked internally for measurement and forwarded
    /// to the inner text input.
    pub fn font(mut self, font: Renderer::Font) -> Self {
        self.font = Some(font);
        self.inner = self.inner.font(font);
        self
    }

    /// Sets the padding. Tracked internally for measurement and
    /// forwarded to the inner text input.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        let p = padding.into();
        self.padding = p;
        self.inner = self.inner.padding(p);
        self
    }

    /// Sets the line height. Tracked internally for measurement and
    /// forwarded to the inner text input.
    pub fn line_height(mut self, line_height: impl Into<LineHeight>) -> Self {
        let lh = line_height.into();
        self.line_height = lh;
        self.inner = self.inner.line_height(lh);
        self
    }

    /// Sets the width of the inner text input. **Disables auto-sizing.**
    ///
    /// When `width` is set, [`Into<Element>`] returns the raw
    /// `TextInput` — it's assumed the caller wants to control width
    /// directly (e.g. `Length::Fill` inside a fixed-size parent).
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.inner = self.inner.width(width);
        self.width_set = true;
        self
    }
}

impl<'a, Message, Theme, Renderer> From<Input<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: text_input::Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(input: Input<'a, Message, Theme, Renderer>) -> Self {
        if input.width_set {
            return input.inner.into();
        }

        Element::new(SizedInput {
            inner: input.inner.into(),
            value: input.value,
            text_size: input.text_size,
            font: input.font,
            padding: input.padding,
            line_height: input.line_height,
        })
    }
}

/// Transparent wrapper widget that sizes its inner `TextInput` to fit
/// its measured `value`.
///
/// Every `Widget` method other than `layout` is a direct pass-through
/// to `inner`, using `tree` as-is (no `tree.children[0]` indirection).
struct SizedInput<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer,
{
    inner: Element<'a, Message, Theme, Renderer>,
    value: &'a str,
    text_size: Option<Pixels>,
    font: Option<Renderer::Font>,
    padding: Padding,
    line_height: LineHeight,
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SizedInput<'a, Message, Theme, Renderer>
where
    Renderer: text::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.inner.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.inner.as_widget().size_hint()
    }

    fn tag(&self) -> widget::tree::Tag {
        self.inner.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.inner.as_widget().state()
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.inner.as_widget().children()
    }

    fn diff(&self, tree: &mut widget::Tree) {
        self.inner.as_widget().diff(tree);
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
        let font = self.font.unwrap_or_else(|| renderer.default_font());

        let paragraph = <Renderer as text::Renderer>::Paragraph::with_text(Text {
            content: self.value,
            bounds: Size::new(f32::INFINITY, f32::INFINITY),
            size: text_size,
            line_height: self.line_height,
            font,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::None,
            letter_spacing: Em::default(),
            font_features: vec![],
            font_variations: vec![],
            weight: None,
            hint_factor: renderer.scale_factor(),
        });

        // Chrome: horizontal padding + a fraction of the text size for
        // the caret glyph / trailing whitespace. 0.25em is a reasonable
        // match for what text_input itself reserves visually.
        let chrome = self.padding.left + self.padding.right + text_size.0 * 0.25;
        let width = (paragraph.min_bounds().width + chrome).min(limits.max().width);

        let new_limits =
            layout::Limits::new(Size::new(width, 0.0), Size::new(width, limits.max().height));

        self.inner
            .as_widget_mut()
            .layout(tree, renderer, &new_limits)
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.inner
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.inner
            .as_widget_mut()
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: layout::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.inner
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: layout::Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.inner
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: layout::Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.inner
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}
