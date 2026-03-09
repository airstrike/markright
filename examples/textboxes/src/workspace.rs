use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay::Group;
use iced::advanced::renderer;
use iced::advanced::widget::tree::Tree;
use iced::advanced::widget::{Operation, Widget};
use iced::mouse;
use iced::overlay;
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

/// A child element positioned absolutely within the workspace.
pub struct Child<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    pub position: Point,
    pub size: Size,
    pub element: Element<'a, Message, Theme, Renderer>,
}

/// A workspace that positions children at absolute coordinates.
///
/// Children are drawn back-to-front (last child is visually on top).
/// Events are dispatched front-to-back (topmost child gets first dibs).
pub struct Workspace<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    children: Vec<Child<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Workspace<'a, Message, Theme, Renderer> {
    pub fn new(children: Vec<Child<'a, Message, Theme, Renderer>>) -> Self {
        Self { children }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Workspace<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn children(&self) -> Vec<Tree> {
        self.children
            .iter()
            .map(|c| Tree::new(&c.element))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let elements: Vec<&Element<'_, _, _, _>> =
            self.children.iter().map(|c| &c.element).collect();
        tree.diff_children(&elements);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let nodes = self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .map(|(child, child_tree)| {
                let child_limits = layout::Limits::new(Size::ZERO, child.size)
                    .width(child.size.width)
                    .height(child.size.height);

                child
                    .element
                    .as_widget_mut()
                    .layout(child_tree, renderer, &child_limits)
                    .move_to(child.position)
            })
            .collect();

        layout::Node::with_children(limits.max(), nodes)
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
        let n = self.children.len();
        // Collect layouts into a vec so we can index in reverse.
        let layouts: Vec<_> = layout.children().collect();

        // Topmost child (last in vec) gets first dibs on events.
        let mut cursor_for_child = cursor;
        for i in (0..n).rev() {
            if shell.is_event_captured() {
                break;
            }

            let child_layout = layouts[i];

            self.children[i].element.as_widget_mut().update(
                &mut tree.children[i],
                event,
                child_layout,
                cursor_for_child,
                renderer,
                shell,
                viewport,
            );

            // Occlude cursor for children underneath this one.
            if cursor_for_child.is_over(child_layout.bounds()) {
                cursor_for_child = mouse::Cursor::Unavailable;
            }
        }
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
        // Draw back-to-front (forward iteration = lowest z first).
        for ((child, child_tree), child_layout) in self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
        {
            child.element.as_widget().draw(
                child_tree,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                viewport,
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        // Check topmost child first (reverse iteration).
        for ((child, child_tree), child_layout) in self
            .children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .rev()
        {
            // Only ask the child if the cursor is over its bounds.
            if cursor.is_over(child_layout.bounds()) {
                let interaction = child.element.as_widget().mouse_interaction(
                    child_tree,
                    child_layout,
                    cursor,
                    viewport,
                    renderer,
                );
                if interaction != mouse::Interaction::None {
                    return interaction;
                }
                // Cursor is over this child's bounds — don't check lower children.
                return mouse::Interaction::None;
            }
        }
        mouse::Interaction::None
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.traverse(&mut |operation| {
            for ((child, child_tree), child_layout) in self
                .children
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                child.element.as_widget_mut().operate(
                    child_tree,
                    child_layout,
                    renderer,
                    operation,
                );
            }
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let children = self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .filter_map(|((child, state), layout)| {
                child.element.as_widget_mut().overlay(
                    state,
                    layout,
                    renderer,
                    viewport,
                    translation,
                )
            })
            .collect::<Vec<_>>();

        if children.is_empty() {
            None
        } else {
            Some(Group::with_children(children).overlay())
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Workspace<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(workspace: Workspace<'a, Message, Theme, Renderer>) -> Self {
        Element::new(workspace)
    }
}
