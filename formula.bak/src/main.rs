use iced::keyboard;
use iced::widget::operation::focus;
use iced::widget::{column, container, row, text, text_input};
use iced::{color, Element, Length, Subscription, Task};

use iced::advanced::text::rich_editor::span;
use markright::widget::rich_editor::{self, Action, Binding, Content, Edit, KeyPress};

mod eval;
mod token;
mod widget;

use token::{FormulaId, Token, TokenMap};
use widget::FormulaHost;

const FORMULA_COLOR: iced::Color = color!(0x6366F1); // indigo-500

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .subscription(App::subscription)
        .title("Formula Editor")
        .run()
}

// ── app state ──────────────────────────────────────────────────────────

struct App {
    source: String,
    tokens: Vec<Token>,
    token_map: TokenMap,
    content: Content<iced::Renderer>,

    /// Which formula the cursor is adjacent to, if any.
    active_formula: Option<FormulaId>,
    /// Whether the overlay is visible.
    overlay_visible: bool,
    /// The text being edited in the overlay input.
    overlay_draft: String,
    /// The original expression when the overlay opened (for revert on Esc/dismiss).
    overlay_original: String,

    focus: Focus,
}

const OVERLAY_INPUT_ID: &str = "formula-overlay-input";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Editor,
    Overlay,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    TabToOverlay,
    OverlayChanged(String),
    OverlayCommit,
    OverlayCancel,
}

// ── update ─────────────────────────────────────────────────────────────

impl App {
    fn new() -> (Self, Task<Message>) {
        let source =
            "Tim had {=1+3} apples, then picked up {=2*6} more at the store.".to_string();
        let tokens = token::parse(&source);
        let token_map = TokenMap::build(&tokens);
        let content = build_content(&tokens);

        (
            Self {
                source,
                tokens,
                token_map,
                content,
                active_formula: None,
                overlay_visible: false,
                overlay_draft: String::new(),
                overlay_original: String::new(),
                focus: Focus::Editor,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => self.handle_editor_action(action),

            Message::TabToOverlay => {
                if self.overlay_visible {
                    self.focus = Focus::Overlay;
                    return focus(OVERLAY_INPUT_ID);
                }
                Task::none()
            }

            Message::OverlayChanged(draft) => {
                self.overlay_draft = draft;
                self.live_patch_formula();
                Task::none()
            }

            Message::OverlayCommit => {
                if let Some(fid) = self.active_formula {
                    let expr = commit_expr(&self.overlay_draft, &self.overlay_original);
                    self.set_formula_expr(fid, &expr);
                }
                self.close_overlay()
            }

            Message::OverlayCancel => {
                self.revert_and_close()
            }
        }
    }

    fn handle_editor_action(&mut self, action: Action) -> Task<Message> {
        // Any editor action while overlay is focused → revert & dismiss
        if self.focus == Focus::Overlay {
            self.revert_overlay();
            self.focus = Focus::Editor;
        }

        if action.is_edit() {
            self.handle_edit(action)
        } else {
            self.content.perform(action);
            self.update_adjacency();
            Task::none()
        }
    }

    fn handle_edit(&mut self, action: Action) -> Task<Message> {
        let cursor = self.content.cursor();
        let dcol = cursor.position.column;
        let scol = self.token_map.display_to_source(dcol);

        match &action {
            Action::Edit(Edit::Insert(ch)) => {
                // Block insertion inside a formula's display range
                if self.cursor_inside_formula(dcol) {
                    return Task::none();
                }
                self.source.insert(scol, *ch);
                self.rebuild_and_restore(scol + ch.len_utf8());
            }

            Action::Edit(Edit::Backspace) => {
                if dcol == 0 {
                    return Task::none();
                }
                let target = scol - 1;
                if let Some(range) = self.token_map.source_offset_in_formula(target) {
                    self.source.replace_range(range.clone(), "");
                    self.rebuild_and_restore(range.start);
                } else {
                    self.source.remove(target);
                    self.rebuild_and_restore(target);
                }
            }

            Action::Edit(Edit::Delete) => {
                if scol >= self.source.len() {
                    return Task::none();
                }
                if let Some(range) = self.token_map.source_offset_in_formula(scol) {
                    self.source.replace_range(range.clone(), "");
                    self.rebuild_and_restore(range.start);
                } else {
                    self.source.remove(scol);
                    self.rebuild_and_restore(scol);
                }
            }

            Action::Edit(Edit::Paste(s)) => {
                let pasted = s.to_string();
                self.source.insert_str(scol, &pasted);
                self.rebuild_and_restore(scol + pasted.len());
            }

            Action::Edit(Edit::Enter { .. } | Edit::Format(_)) => {}
            _ => {}
        }

        Task::none()
    }

    // ── adjacency & overlay lifecycle ──────────────────────────────────

    fn update_adjacency(&mut self) {
        let col = self.content.cursor().position.column;
        let new_active = self.token_map.adjacent_formula(col, &self.tokens);

        if new_active == self.active_formula {
            return;
        }

        // Cursor moved to a different (or no) formula.
        // Revert any uncommitted overlay edits for the old formula.
        if self.overlay_visible {
            self.revert_overlay();
        }

        if let Some(fid) = new_active {
            // Open overlay for the new formula
            self.open_overlay_for(fid);
        } else {
            // Left all formulas — close overlay
            self.active_formula = None;
            self.overlay_visible = false;
        }
    }

    fn open_overlay_for(&mut self, fid: FormulaId) {
        #[allow(clippy::collapsible_if)]
        if let Some(Token::Formula { expr, .. }) = self
            .tokens
            .iter()
            .find(|t| matches!(t, Token::Formula { id, .. } if *id == fid))
        {
            self.overlay_draft = format!("{{={expr}}}");
            self.overlay_original = expr.clone();
            self.overlay_visible = true;
            self.active_formula = Some(fid);
            // Focus stays in editor — Tab moves to overlay
        }
    }

    fn revert_overlay(&mut self) {
        if let Some(fid) = self.active_formula {
            let original = self.overlay_original.clone();
            self.set_formula_expr(fid, &original);
        }
        self.overlay_visible = false;
        self.overlay_draft.clear();
    }

    fn revert_and_close(&mut self) -> Task<Message> {
        self.revert_overlay();
        self.active_formula = None;
        self.focus = Focus::Editor;
        Task::none()
    }

    fn close_overlay(&mut self) -> Task<Message> {
        self.overlay_visible = false;
        self.overlay_draft.clear();
        self.focus = Focus::Editor;
        Task::none()
    }

    // ── helpers ─────────────────────────────────────────────────────────

    fn cursor_inside_formula(&self, dcol: usize) -> bool {
        // True if dcol is strictly inside (not at edge of) a formula range
        self.token_map
            .adjacent_formula(dcol, &self.tokens)
            .is_some()
            && dcol > 0
            && self
                .token_map
                .adjacent_formula(dcol - 1, &self.tokens)
                .is_some()
    }

    fn rebuild_and_restore(&mut self, source_offset: usize) {
        self.tokens = token::parse(&self.source);
        self.token_map = TokenMap::build(&self.tokens);
        self.content = build_content(&self.tokens);
        let dcol = self.token_map.source_to_display(source_offset);
        self.content.move_to(0, dcol);
        self.update_adjacency();
    }

    #[allow(clippy::collapsible_if)]
    fn live_patch_formula(&mut self) {
        let draft = self.overlay_draft.clone();
        if let Some(expr) = strip_formula_markers(&draft) {
            if let Some(fid) = self.active_formula {
                self.set_formula_expr(fid, expr);
            }
        }
    }

    fn set_formula_expr(&mut self, fid: FormulaId, expr: &str) {
        for tok in &mut self.tokens {
            if let Token::Formula { id, expr: e, .. } = tok
                && *id == fid
            {
                *e = expr.to_string();
                break;
            }
        }
        self.source = token::serialize(&self.tokens);
        self.token_map = TokenMap::build(&self.tokens);
        let dcol = self.content.cursor().position.column;
        self.content = build_content(&self.tokens);
        self.content.move_to(0, dcol);
    }
}

// ── subscription (overlay key handling) ────────────────────────────────

impl App {
    fn subscription(&self) -> Subscription<Message> {
        if self.focus == Focus::Overlay {
            keyboard::listen().map(|event| match event {
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                } => Message::OverlayCancel,
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Tab),
                    ..
                } => Message::OverlayCommit,
                _ => Message::Editor(Action::Deselect), // no-op
            })
        } else {
            Subscription::none()
        }
    }
}

// ── view ───────────────────────────────────────────────────────────────

impl App {
    fn view(&self) -> Element<'_, Message> {
        let has_active = self.overlay_visible && self.focus == Focus::Editor;

        let editor = rich_editor::rich_editor(&self.content)
            .on_action(Message::Editor)
            .height(Length::Shrink)
            .padding(12)
            .key_binding(move |press: KeyPress| {
                if has_active && is_tab(&press) {
                    Some(Binding::Custom(Message::TabToOverlay))
                } else {
                    None
                }
            });

        // Overlay: appears automatically when cursor is near a formula
        let overlay_el = if self.overlay_visible {
            let preview = overlay_preview(&self.overlay_draft, &self.overlay_original);

            // Size overlay to fit the draft text, with a reasonable floor
            let draft_chars = self.overlay_draft.len().max(8) as f32;
            let input_width = (draft_chars * 8.5 + 32.0).min(400.0);

            let overlay_col = column![
                text_input("{=expr}", &self.overlay_draft)
                    .id(OVERLAY_INPUT_ID)
                    .on_input(Message::OverlayChanged)
                    .on_submit(Message::OverlayCommit)
                    .size(14)
                    .width(input_width),
                row![
                    text("= ").size(12).color(color!(0x94A3B8)),
                    text(preview).size(12).color(color!(0x334155)),
                ]
            ]
            .spacing(4)
            .padding(8);

            let styled = container(overlay_col)
                .width(Length::Shrink)
                .style(|_theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(color!(0xFFFFFF))),
                    border: iced::Border {
                        color: color!(0xCBD5E1),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    shadow: iced::Shadow {
                        color: color!(0x000000, 0.1),
                        offset: iced::Vector::new(0.0, 4.0),
                        blur_radius: 12.0,
                    },
                    ..Default::default()
                });

            Some(styled.into())
        } else {
            None
        };

        let host: Element<'_, Message> =
            FormulaHost::new(editor).overlay_maybe(overlay_el).into();

        // Debug: source
        let source_debug = column![
            text("source").size(10).color(color!(0x94A3B8)),
            text(&self.source).size(12).color(color!(0x64748B)),
        ]
        .spacing(2);

        container(column![host, source_debug].spacing(16))
            .padding(32)
            .max_width(640)
            .into()
    }
}

// ── content builder ────────────────────────────────────────────────────

fn build_content(tokens: &[Token]) -> Content<iced::Renderer> {
    use markright_core::StyledLine;

    let mut line_text = String::new();
    let mut runs = Vec::new();

    for token in tokens {
        let display = token.display_value();
        let start = line_text.len();
        line_text.push_str(&display);
        let end = line_text.len();

        if matches!(token, Token::Formula { .. }) && start < end {
            runs.push(markright_core::StyleRun {
                range: start..end,
                style: span::Style {
                    color: Some(FORMULA_COLOR),
                    ..Default::default()
                },
            });
        }
    }

    let styled_line = StyledLine {
        text: line_text,
        runs,
        paragraph: markright_core::Paragraph::default(),
    };

    Content::from_styled_lines(&[styled_line])
}

// ── utilities ──────────────────────────────────────────────────────────

fn strip_formula_markers(draft: &str) -> Option<&str> {
    draft
        .strip_prefix("{=")
        .and_then(|s| s.strip_suffix('}'))
}

fn commit_expr(draft: &str, original: &str) -> String {
    strip_formula_markers(draft)
        .unwrap_or(original)
        .to_string()
}

fn overlay_preview(draft: &str, original: &str) -> String {
    if let Some(expr) = strip_formula_markers(draft) {
        eval::eval_display(expr)
    } else {
        eval::eval_display(original)
    }
}

fn is_tab(press: &KeyPress) -> bool {
    press.key == keyboard::Key::Named(keyboard::key::Named::Tab) && !press.modifiers.shift()
}
