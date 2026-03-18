use std::cell::{Cell, RefCell};

use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::overlay::Group;
use iced::advanced::renderer;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::widget::{Operation, Widget};
use iced::keyboard;
use iced::{Border, Color, Element, Event, Length, Point, Rectangle, Size, Vector};
use iced::{alignment, overlay};

use indexmap::{IndexMap, IndexSet};

// ---------------------------------------------------------------------------
// Id
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(u64);

// ---------------------------------------------------------------------------
// Entry (private)
// ---------------------------------------------------------------------------

struct Entry {
    bounds: Cell<Rectangle>,
    v_align: Cell<alignment::Vertical>,
}

// ---------------------------------------------------------------------------
// Interaction (private) — all variants are Copy (no heap allocations)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
enum Interaction {
    #[default]
    Idle,
    Pressed {
        id: Id,
        origin: Point,
    },
    Dragging {
        origin: Point,
    },
    Selecting {
        origin: Point,
        current: Point,
    },
    Editing {
        id: Id,
    },
}

// ---------------------------------------------------------------------------
// State (public, app-owned)
// ---------------------------------------------------------------------------

pub struct State {
    boxes: IndexMap<Id, Entry>,
    interaction: Cell<Interaction>,
    selected: RefCell<IndexSet<Id>>,
    current: u64,
}

impl State {
    pub fn new() -> Self {
        Self {
            boxes: IndexMap::new(),
            interaction: Cell::new(Interaction::Idle),
            selected: RefCell::new(IndexSet::new()),
            current: 0,
        }
    }

    pub fn insert(&mut self, bounds: Rectangle, v_align: alignment::Vertical) -> Id {
        let id = Id(self.current);
        self.current += 1;
        self.boxes.insert(
            id,
            Entry {
                bounds: Cell::new(bounds),
                v_align: Cell::new(v_align),
            },
        );
        id
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, id: Id) {
        self.boxes.shift_remove(&id);
        self.selected.borrow_mut().shift_remove(&id);
    }

    pub fn bounds(&self, id: Id) -> Rectangle {
        self.boxes[&id].bounds.get()
    }

    pub fn set_bounds(&self, id: Id, bounds: Rectangle) {
        self.boxes[&id].bounds.set(bounds);
    }

    pub fn v_align(&self, id: Id) -> alignment::Vertical {
        self.boxes[&id].v_align.get()
    }

    pub fn set_v_align(&self, id: Id, v: alignment::Vertical) {
        self.boxes[&id].v_align.set(v);
    }

    pub fn editing(&self) -> Option<Id> {
        match self.interaction.get() {
            Interaction::Editing { id } => Some(id),
            _ => None,
        }
    }

    pub fn is_selected(&self, id: Id) -> bool {
        self.selected.borrow().contains(&id)
    }

    fn select(&self, id: Id) {
        self.selected.borrow_mut().insert(id);
    }

    fn toggle_selected(&self, id: Id) {
        let mut sel = self.selected.borrow_mut();
        if sel.contains(&id) {
            sel.shift_remove(&id);
        } else {
            sel.insert(id);
        }
    }

    fn clear_selection(&self) {
        self.selected.borrow_mut().clear();
    }

    #[allow(dead_code)]
    pub fn ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.boxes.keys().copied()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.boxes.len()
    }
}

// ---------------------------------------------------------------------------
// View (public, read-only view passed to the closure)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct View<'a> {
    entry: &'a Entry,
    is_editing: bool,
    is_selected: bool,
}

impl View<'_> {
    pub fn bounds(&self) -> Rectangle {
        self.entry.bounds.get()
    }

    pub fn v_align(&self) -> alignment::Vertical {
        self.entry.v_align.get()
    }

    pub fn is_editing(&self) -> bool {
        self.is_editing
    }

    #[allow(dead_code)]
    pub fn is_selected(&self) -> bool {
        self.is_selected
    }
}

// ---------------------------------------------------------------------------
// WidgetState (private, stored in the widget tree)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct WidgetState {
    last_click: Option<mouse::Click>,
    /// Original positions of boxes being dragged (populated on Pressed→Dragging).
    drag_origins: Vec<(Id, Point)>,
    /// Current mouse position during drag (draw uses this for with_translation).
    drag_current: Option<Point>,
    modifiers: keyboard::Modifiers,
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

pub fn workspace<'a, Message, Theme, Renderer>(
    state: &'a State,
    view: impl Fn(Id, View<'_>) -> Element<'a, Message, Theme, Renderer>,
) -> Workspace<'a, Message, Theme, Renderer> {
    let editing_id = state.editing();
    let selected = state.selected.borrow();
    let elements = state
        .boxes
        .iter()
        .map(|(&id, entry)| {
            let bx = View {
                entry,
                is_editing: editing_id == Some(id),
                is_selected: selected.contains(&id),
            };
            view(id, bx)
        })
        .collect();

    Workspace {
        state,
        elements,
        extra: Vec::new(),
        on_edit: None,
        on_edit_exit: None,
        on_move: None,
    }
}

// ---------------------------------------------------------------------------
// Workspace
// ---------------------------------------------------------------------------

pub struct Workspace<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    state: &'a State,
    elements: Vec<Element<'a, Message, Theme, Renderer>>,
    extra: Vec<(Point, Element<'a, Message, Theme, Renderer>)>,
    on_edit: Option<Box<dyn Fn(Id) -> Message + 'a>>,
    on_edit_exit: Option<Box<dyn Fn(Id) -> Message + 'a>>,
    on_move: Option<Box<dyn Fn(Id, Rectangle) -> Message + 'a>>,
}

impl<'a, Message, Theme, Renderer> Workspace<'a, Message, Theme, Renderer> {
    pub fn on_edit(mut self, f: impl Fn(Id) -> Message + 'a) -> Self {
        self.on_edit = Some(Box::new(f));
        self
    }

    pub fn on_edit_exit(mut self, f: impl Fn(Id) -> Message + 'a) -> Self {
        self.on_edit_exit = Some(Box::new(f));
        self
    }

    pub fn on_move(mut self, f: impl Fn(Id, Rectangle) -> Message + 'a) -> Self {
        self.on_move = Some(Box::new(f));
        self
    }

    pub fn push(
        mut self,
        position: Point,
        element: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.extra.push((position, element.into()));
        self
    }

    fn child_count(&self) -> usize {
        self.elements.len() + self.extra.len()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn selection_rect(a: Point, b: Point) -> Rectangle {
    Rectangle::new(
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Size::new((a.x - b.x).abs(), (a.y - b.y).abs()),
    )
}

fn rects_intersect(a: Rectangle, b: Rectangle) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

// Rubberband colors (hardcoded, not themed).
const SELECTION_FILL: Color = Color {
    r: 0.3,
    g: 0.55,
    b: 1.0,
    a: 0.12,
};
const SELECTION_BORDER: Color = Color {
    r: 0.3,
    g: 0.55,
    b: 1.0,
    a: 0.5,
};

// ---------------------------------------------------------------------------
// Widget impl
// ---------------------------------------------------------------------------

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Workspace<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<WidgetState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(WidgetState::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.elements
            .iter()
            .map(Tree::new)
            .chain(self.extra.iter().map(|(_, e)| Tree::new(e)))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let all: Vec<&Element<'_, _, _, _>> = self
            .elements
            .iter()
            .chain(self.extra.iter().map(|(_, e)| e))
            .collect();
        tree.diff_children(&all);
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
        let n = self.elements.len();
        let (box_trees, extra_trees) = tree.children.split_at_mut(n);

        let box_nodes = self
            .elements
            .iter_mut()
            .zip(box_trees)
            .zip(self.state.boxes.values())
            .map(|((elem, child_tree), entry)| {
                let bounds = entry.bounds.get();
                let child_limits = layout::Limits::new(Size::ZERO, bounds.size())
                    .width(bounds.width)
                    .height(bounds.height);

                elem.as_widget_mut()
                    .layout(child_tree, renderer, &child_limits)
                    .move_to(bounds.position())
            });

        let extra_nodes =
            self.extra
                .iter_mut()
                .zip(extra_trees)
                .map(|((pos, elem), child_tree)| {
                    let child_limits = layout::Limits::new(Size::ZERO, limits.max());

                    let node = elem
                        .as_widget_mut()
                        .layout(child_tree, renderer, &child_limits);

                    // Treat `pos` as the center-x anchor: shift left by half the
                    // element's intrinsic width so it's centered on that point.
                    let centered = Point::new(pos.x - node.size().width / 2.0, pos.y);
                    node.move_to(centered)
                });

        let nodes: Vec<_> = box_nodes.chain(extra_nodes).collect();
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
        let n = self.elements.len();
        let total = self.child_count();
        let layouts: Vec<_> = layout.children().collect();

        // Phase 1: Forward events to children in reverse z-order.
        let mut cursor_for_child = cursor;
        for i in (0..total).rev() {
            if shell.is_event_captured() {
                break;
            }

            let child_layout = layouts[i];

            if i < n {
                self.elements[i].as_widget_mut().update(
                    &mut tree.children[i],
                    event,
                    child_layout,
                    cursor_for_child,
                    renderer,
                    shell,
                    viewport,
                );
            } else {
                self.extra[i - n].1.as_widget_mut().update(
                    &mut tree.children[i],
                    event,
                    child_layout,
                    cursor_for_child,
                    renderer,
                    shell,
                    viewport,
                );
            }

            if cursor_for_child.is_over(child_layout.bounds()) {
                cursor_for_child = mouse::Cursor::Unavailable;
            }
        }

        // Phase 2: Workspace-level interactions (only if event not captured).
        if shell.is_event_captured() {
            return;
        }

        let widget_state = tree.state.downcast_mut::<WidgetState>();

        // Track modifier keys.
        if let Event::Keyboard(keyboard::Event::ModifiersChanged(m)) = event {
            widget_state.modifiers = *m;
        }

        let interaction = self.state.interaction.get();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position() {
                    // Skip if cursor is over an extra element (toolbar etc.).
                    let over_extra = (n..total).any(|i| cursor.is_over(layouts[i].bounds()));
                    if over_extra {
                        return;
                    }

                    // Hit-test boxes in reverse z-order.
                    let hit = (0..n).rev().find(|&i| cursor.is_over(layouts[i].bounds()));
                    let shift = widget_state.modifiers.shift();

                    if let Some(i) = hit {
                        let (&id, _) = self.state.boxes.get_index(i).unwrap();

                        // If editing this box, re-focus the editor (click may have
                        // landed in container padding outside the editor bounds).
                        if matches!(interaction, Interaction::Editing { id: eid } if eid == id) {
                            if let Some(ref on_edit) = self.on_edit {
                                shell.publish(on_edit(id));
                            }
                            shell.capture_event();
                            return;
                        }

                        // Exit edit mode if we were editing a different box.
                        if let Interaction::Editing { id: eid } = interaction
                            && let Some(ref on_exit) = self.on_edit_exit
                        {
                            shell.publish(on_exit(eid));
                        }

                        if shift {
                            // Shift+click: toggle selection, no drag initiation.
                            self.state.toggle_selected(id);
                            shell.capture_event();
                            shell.request_redraw();
                        } else {
                            // Double-click detection.
                            let click = mouse::Click::new(
                                pos,
                                mouse::Button::Left,
                                widget_state.last_click,
                            );
                            widget_state.last_click = Some(click);

                            if click.kind() == mouse::click::Kind::Double {
                                self.state.clear_selection();
                                self.state.interaction.set(Interaction::Editing { id });
                                if let Some(ref on_edit) = self.on_edit {
                                    shell.publish(on_edit(id));
                                }
                            } else {
                                // Single click: if box not in selection, clear selection.
                                if !self.state.is_selected(id) {
                                    self.state.clear_selection();
                                }
                                self.state
                                    .interaction
                                    .set(Interaction::Pressed { id, origin: pos });
                            }
                            shell.capture_event();
                        }
                    } else {
                        // Clicked empty space.
                        widget_state.last_click = None;

                        if let Interaction::Editing { id: eid } = interaction
                            && let Some(ref on_exit) = self.on_edit_exit
                        {
                            shell.publish(on_exit(eid));
                        }

                        if !shift {
                            self.state.clear_selection();
                        }

                        // Start rubberband selection.
                        self.state.interaction.set(Interaction::Selecting {
                            origin: pos,
                            current: pos,
                        });
                    }
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let position = *position;
                match interaction {
                    Interaction::Pressed { id, origin } => {
                        let dx = position.x - origin.x;
                        let dy = position.y - origin.y;
                        if dx * dx + dy * dy > 9.0 {
                            // Collect boxes to drag.
                            let selected = self.state.selected.borrow();
                            if selected.contains(&id) {
                                // Drag all selected boxes.
                                widget_state.drag_origins = selected
                                    .iter()
                                    .map(|&bid| (bid, self.state.bounds(bid).position()))
                                    .collect();
                            } else {
                                // Drag just the pressed box.
                                widget_state.drag_origins =
                                    vec![(id, self.state.bounds(id).position())];
                            }
                            drop(selected);

                            self.state.interaction.set(Interaction::Dragging { origin });
                            widget_state.drag_current = Some(position);
                            shell.request_redraw();
                        }
                    }
                    Interaction::Dragging { .. } => {
                        widget_state.drag_current = Some(position);
                        shell.request_redraw();
                    }
                    Interaction::Selecting { origin, .. } => {
                        self.state.interaction.set(Interaction::Selecting {
                            origin,
                            current: position,
                        });
                        shell.request_redraw();
                    }
                    _ => {}
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => match interaction {
                Interaction::Dragging { origin } => {
                    // Commit final positions to Cell bounds.
                    if let Some(current) = widget_state.drag_current {
                        let dx = current.x - origin.x;
                        let dy = current.y - origin.y;
                        for &(bid, box_origin) in &widget_state.drag_origins {
                            let size = self.state.bounds(bid).size();
                            self.state.set_bounds(
                                bid,
                                Rectangle::new(
                                    Point::new(box_origin.x + dx, box_origin.y + dy),
                                    size,
                                ),
                            );
                        }
                    }
                    if let Some(ref on_move) = self.on_move {
                        for &(bid, _) in &widget_state.drag_origins {
                            shell.publish(on_move(bid, self.state.bounds(bid)));
                        }
                    }
                    widget_state.drag_origins.clear();
                    widget_state.drag_current = None;
                    self.state.interaction.set(Interaction::Idle);
                    shell.invalidate_layout();
                }
                Interaction::Pressed { id, .. } => {
                    // Click without drag — select just this box.
                    if !widget_state.modifiers.shift() {
                        self.state.clear_selection();
                    }
                    self.state.select(id);
                    self.state.interaction.set(Interaction::Idle);
                    shell.request_redraw();
                }
                Interaction::Selecting { origin, current } => {
                    let rect = selection_rect(origin, current);
                    // Only select if the rectangle has meaningful size.
                    if rect.width > 3.0 && rect.height > 3.0 {
                        for (&id, entry) in &self.state.boxes {
                            if rects_intersect(rect, entry.bounds.get()) {
                                self.state.select(id);
                            }
                        }
                    }
                    self.state.interaction.set(Interaction::Idle);
                    shell.request_redraw();
                }
                _ => {}
            },

            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
                if let Interaction::Editing { id } = interaction {
                    self.state.interaction.set(Interaction::Idle);
                    if let Some(ref on_exit) = self.on_edit_exit {
                        shell.publish(on_exit(id));
                    }
                    shell.capture_event();
                }
            }

            _ => {}
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
        let n = self.elements.len();
        let layouts: Vec<_> = layout.children().collect();
        let widget_state = tree.state.downcast_ref::<WidgetState>();

        // Compute drag delta if actively dragging.
        let drag_delta = match (self.state.interaction.get(), widget_state.drag_current) {
            (Interaction::Dragging { origin }, Some(current)) => {
                Some(Vector::new(current.x - origin.x, current.y - origin.y))
            }
            _ => None,
        };

        // Box elements — each in its own layer for proper occlusion.
        for (i, ((elem, child_tree), child_layout)) in self
            .elements
            .iter()
            .zip(&tree.children[..n])
            .zip(&layouts[..n])
            .enumerate()
        {
            let (&id, _) = self.state.boxes.get_index(i).unwrap();
            let is_dragged =
                drag_delta.is_some() && widget_state.drag_origins.iter().any(|&(bid, _)| bid == id);

            if is_dragged {
                let delta = drag_delta.unwrap();
                let bounds = child_layout.bounds();
                let translated_bounds = Rectangle {
                    x: bounds.x + delta.x,
                    y: bounds.y + delta.y,
                    ..bounds
                };
                renderer.with_layer(translated_bounds, |renderer| {
                    renderer.with_translation(delta, |renderer| {
                        elem.as_widget().draw(
                            child_tree,
                            renderer,
                            theme,
                            style,
                            *child_layout,
                            cursor,
                            viewport,
                        );
                    });
                });
            } else {
                renderer.with_layer(child_layout.bounds(), |renderer| {
                    elem.as_widget().draw(
                        child_tree,
                        renderer,
                        theme,
                        style,
                        *child_layout,
                        cursor,
                        viewport,
                    );
                });
            }
        }

        // Selection borders (drawn over all box layers in a separate layer).
        renderer.with_layer(*viewport, |renderer| {
            let selected = self.state.selected.borrow();
            for (i, child_layout) in layouts[..n].iter().enumerate() {
                let (&id, _) = self.state.boxes.get_index(i).unwrap();
                if selected.contains(&id) {
                    let is_dragged = drag_delta.is_some()
                        && widget_state.drag_origins.iter().any(|&(bid, _)| bid == id);
                    let bounds = if is_dragged {
                        let delta = drag_delta.unwrap();
                        let b = child_layout.bounds().expand(2.0);
                        Rectangle {
                            x: b.x + delta.x,
                            y: b.y + delta.y,
                            ..b
                        }
                    } else {
                        child_layout.bounds()
                    };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border: Border {
                                width: 2.0,
                                color: SELECTION_BORDER,
                                radius: 6.0.into(),
                            },
                            ..renderer::Quad::default()
                        },
                        SELECTION_FILL,
                    );
                }
            }
        });

        // Rubberband selection rectangle.
        if let Interaction::Selecting { origin, current } = self.state.interaction.get() {
            let rect = selection_rect(origin, current);
            if rect.width > 0.5 || rect.height > 0.5 {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: rect,
                        border: Border {
                            width: 1.0,
                            color: SELECTION_BORDER,
                            radius: 0.0.into(),
                        },
                        ..renderer::Quad::default()
                    },
                    SELECTION_FILL,
                );
            }
        }

        // Extra elements on top — each in its own layer.
        for (((_, elem), child_tree), child_layout) in self
            .extra
            .iter()
            .zip(&tree.children[n..])
            .zip(&layouts[n..])
        {
            renderer.with_layer(child_layout.bounds(), |renderer| {
                elem.as_widget().draw(
                    child_tree,
                    renderer,
                    theme,
                    style,
                    *child_layout,
                    cursor,
                    viewport,
                );
            });
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
        let interaction = self.state.interaction.get();

        if matches!(interaction, Interaction::Dragging { .. }) {
            return mouse::Interaction::Grabbing;
        }
        if matches!(interaction, Interaction::Selecting { .. }) {
            return mouse::Interaction::Crosshair;
        }

        let n = self.elements.len();
        let total = self.child_count();
        let layouts: Vec<_> = layout.children().collect();

        for i in (0..total).rev() {
            if !cursor.is_over(layouts[i].bounds()) {
                continue;
            }

            if i >= n {
                // Extra element — delegate to child.
                let extra_idx = i - n;
                let child_interaction = self.extra[extra_idx].1.as_widget().mouse_interaction(
                    &tree.children[i],
                    layouts[i],
                    cursor,
                    viewport,
                    renderer,
                );
                if child_interaction != mouse::Interaction::None {
                    return child_interaction;
                }
                return mouse::Interaction::None;
            }

            // Box element.
            let (&id, _) = self.state.boxes.get_index(i).unwrap();

            // If editing this box, delegate to child (text cursor etc.).
            if matches!(interaction, Interaction::Editing { id: eid } if eid == id) {
                return self.elements[i].as_widget().mouse_interaction(
                    &tree.children[i],
                    layouts[i],
                    cursor,
                    viewport,
                    renderer,
                );
            }

            // Non-editing box: show grab cursor.
            return mouse::Interaction::Grab;
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
            for ((elem, child_tree), child_layout) in self
                .elements
                .iter_mut()
                .chain(self.extra.iter_mut().map(|(_, e)| e))
                .zip(&mut tree.children)
                .zip(layout.children())
            {
                elem.as_widget_mut()
                    .operate(child_tree, child_layout, renderer, operation);
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
        let n = self.elements.len();
        let (box_trees, extra_trees) = tree.children.split_at_mut(n);
        let mut layouts = layout.children();

        let mut overlays = Vec::new();

        for (elem, child_tree) in self.elements.iter_mut().zip(box_trees) {
            let child_layout = layouts.next().unwrap();
            if let Some(o) = elem.as_widget_mut().overlay(
                child_tree,
                child_layout,
                renderer,
                viewport,
                translation,
            ) {
                overlays.push(o);
            }
        }

        for ((_, elem), child_tree) in self.extra.iter_mut().zip(extra_trees) {
            let child_layout = layouts.next().unwrap();
            if let Some(o) = elem.as_widget_mut().overlay(
                child_tree,
                child_layout,
                renderer,
                viewport,
                translation,
            ) {
                overlays.push(o);
            }
        }

        if overlays.is_empty() {
            None
        } else {
            Some(Group::with_children(overlays).overlay())
        }
    }
}

// ---------------------------------------------------------------------------
// From
// ---------------------------------------------------------------------------

impl<'a, Message, Theme, Renderer> From<Workspace<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(workspace: Workspace<'a, Message, Theme, Renderer>) -> Self {
        Element::new(workspace)
    }
}
