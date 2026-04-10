//! `FormulaHost` — a wrapper widget that attaches a `Widget::overlay()` popup
//! to its child. The overlay content is provided as an `Element` and is
//! positioned just below the host widget's bounds.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget, Tree};
use iced::advanced::{self, Shell};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector, mouse};

// ── FormulaHost widget ─────────────────────────────────────────────────

pub struct FormulaHost<'a, Message, Theme, Renderer>
where
    Renderer: advanced::Renderer,
{
    child: Element<'a, Message, Theme, Renderer>,
    overlay_content: Option<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> FormulaHost<'a, Message, Theme, Renderer>
where
    Renderer: advanced::Renderer,
{
    pub fn new(child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            child: child.into(),
            overlay_content: None,
        }
    }

    pub fn overlay(mut self, content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        self.overlay_content = Some(content.into());
        self
    }

    pub fn overlay_maybe(
        mut self,
        content: Option<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.overlay_content = content;
        self
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for FormulaHost<'a, Message, Theme, Renderer>
where
    Renderer: advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.child.as_widget().size()
    }

    fn children(&self) -> Vec<Tree> {
        let mut v = vec![Tree::new(&self.child)];
        if let Some(ref ov) = self.overlay_content {
            v.push(Tree::new(ov));
        }
        v
    }

    fn diff(&self, tree: &mut Tree) {
        // Always maintain at least 2 tree children so the overlay tree
        // persists across frames (keeps text_input state alive).
        while tree.children.len() < 2 {
            tree.children.push(Tree::empty());
        }
        tree.children[0].diff(&self.child);
        if let Some(ref ov) = self.overlay_content {
            tree.children[1].diff(ov);
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.child
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.child
            .as_widget()
            .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.child
            .as_widget_mut()
            .update(&mut tree.children[0], event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.child
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.child
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if let Some(content) = self.overlay_content.as_mut() {
            let bounds = layout.bounds();
            let position = Point::new(
                bounds.x + translation.x,
                bounds.y + bounds.height + translation.y + 4.0,
            );

            let (child_tree, rest) = tree.children.split_at_mut(1);
            let overlay_tree = &mut rest[0];

            // Also propagate any child overlay (e.g. RichEditor IME)
            let _child_overlay = self.child.as_widget_mut().overlay(
                &mut child_tree[0],
                layout,
                renderer,
                viewport,
                translation,
            );

            Some(overlay::Element::new(Box::new(HostOverlay {
                content,
                tree: overlay_tree,
                position,
                width: bounds.width,
            })))
        } else {
            // No overlay content — delegate to child
            self.child.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            )
        }
    }
}

impl<'a, Message, Theme, Renderer> From<FormulaHost<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: advanced::Renderer + 'a,
{
    fn from(host: FormulaHost<'a, Message, Theme, Renderer>) -> Self {
        Element::new(host)
    }
}

// ── overlay implementation ─────────────────────────────────────────────

struct HostOverlay<'a, 'b, Message, Theme, Renderer>
where
    Renderer: advanced::Renderer,
{
    content: &'b mut Element<'a, Message, Theme, Renderer>,
    tree: &'b mut Tree,
    position: Point,
    width: f32,
}

impl<'a, 'b, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for HostOverlay<'a, 'b, Message, Theme, Renderer>
where
    Renderer: advanced::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, _bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::ZERO, Size::new(self.width, f32::INFINITY));
        let node = self
            .content
            .as_widget_mut()
            .layout(self.tree, renderer, &limits);
        node.move_to(self.position)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.content
            .as_widget()
            .draw(self.tree, renderer, theme, style, layout, cursor, &layout.bounds());
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        self.content.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            shell,
            &layout.bounds(),
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        )
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(self.tree, layout, renderer, operation);
    }
}
