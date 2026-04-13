use iced::widget::operation::focus;
use iced::widget::{column, container, row, text};
use iced::{Element, Length, Task, color};

use markright::widget::rich_editor::popup;
use markright::widget::rich_editor::{self, Action, Content, Edit, Instruction};

mod eval;
mod format;
mod token;

use token::{FormulaId, Token, TokenMap};

const FORMULA_COLOR: iced::Color = color!(0x8C8C7A);
const EDITOR_ID: &str = "formula-editor";

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Formula Editor")
        .run()
}

struct App {
    source: String,
    tokens: Vec<Token>,
    token_map: TokenMap,
    content: Content<iced::Renderer>,
    spans: Vec<popup::Span>,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    Popup(popup::Action),
    Instruction(Instruction),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let source = "Tim had {=1+3} apples, then picked up {=2*6} more at the store.".to_string();
        let tokens = token::parse(&source);
        let token_map = TokenMap::build(&tokens);
        let content = Content::from_styled_lines(&[format::styled_line_from_tokens(&tokens)]);
        let spans = parse(&tokens, &token_map);

        (
            Self {
                source,
                tokens,
                token_map,
                content,
                spans,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => self.handle_editor_action(action),

            Message::Popup(popup::Action::Input { span, value }) => {
                if let Some(expr) = strip_formula_markers(&value)
                    && let Some(fid) = self.formula_id_for_span(&span)
                {
                    self.set_formula_expr(fid, expr);
                }
                Task::none()
            }

            Message::Popup(popup::Action::Confirm { .. }) => Task::none(),

            Message::Popup(popup::Action::Dismiss { span, original }) => {
                if let Some(expr) = strip_formula_markers(&original)
                    && let Some(fid) = self.formula_id_for_span(&span)
                {
                    self.set_formula_expr(fid, expr);
                }
                Task::none()
            }

            Message::Instruction(Instruction::Focus(id)) => focus(id),
        }
    }

    fn handle_editor_action(&mut self, action: Action) -> Task<Message> {
        if action.is_edit() {
            self.handle_edit(action)
        } else {
            self.content.perform(action);
            Task::none()
        }
    }

    fn handle_edit(&mut self, action: Action) -> Task<Message> {
        let cursor = self.content.cursor();
        let dcol = cursor.position.column;
        let scol = self.token_map.display_to_source(dcol);

        match &action {
            Action::Edit(Edit::Insert(ch)) => {
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

    fn cursor_inside_formula(&self, dcol: usize) -> bool {
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
        self.content = Content::from_styled_lines(&[format::styled_line_from_tokens(&self.tokens)]);
        self.spans = parse(&self.tokens, &self.token_map);
        let dcol = self.token_map.source_to_display(source_offset);
        self.content.move_to(0, dcol);
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
        self.spans = parse(&self.tokens, &self.token_map);
        let dcol = self.content.cursor().position.column;
        self.content = Content::from_styled_lines(&[format::styled_line_from_tokens(&self.tokens)]);
        self.content.move_to(0, dcol);
    }

    fn formula_id_for_span(&self, span: &popup::SpanRef) -> Option<FormulaId> {
        self.token_map
            .regions
            .iter()
            .find(|r| r.is_formula && r.display_range == span.range)
            .and_then(|r| match &self.tokens[r.token_index] {
                Token::Formula { id, .. } => Some(*id),
                _ => None,
            })
    }
}

impl App {
    fn view(&self) -> Element<'_, Message> {
        let editor = rich_editor::rich_editor(&self.content)
            .id(EDITOR_ID)
            .on_action(Message::Editor)
            .on_instruction(Message::Instruction)
            .height(Length::Shrink)
            .padding(12)
            .popup_spans(&self.spans, Message::Popup, |input, span| {
                let preview = eval_preview(&span.value);
                let chars = span.value.len().max(8) as f32;
                let input_width = (chars * 8.5 + 32.0).min(400.0);

                container(
                    column![
                        input.size(14).width(input_width),
                        row![
                            text("= ").size(12).color(color!(0x94A3B8)),
                            text(preview).size(12).color(color!(0x334155)),
                        ]
                    ]
                    .spacing(4)
                    .padding(8),
                )
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
                })
                .into()
            });

        let source_debug = column![
            text("source").size(10).color(color!(0x94A3B8)),
            text(&self.source).size(12).color(color!(0x64748B)),
        ]
        .spacing(2);

        container(column![editor, source_debug].spacing(16))
            .padding(32)
            .max_width(640)
            .into()
    }
}

fn parse(tokens: &[Token], token_map: &TokenMap) -> Vec<popup::Span> {
    token_map
        .regions
        .iter()
        .filter(|r| r.is_formula)
        .filter_map(|r| {
            let Token::Formula { expr, .. } = &tokens[r.token_index] else {
                return None;
            };
            Some(popup::Span {
                line: 0,
                range: r.display_range.clone(),
                value: format!("{{={expr}}}"),
                placeholder: "{=expr}".to_string(),
                background: Some(iced::Background::Color(color!(0xFAF9F5))),
                border: iced::Border {
                    color: color!(0xE5E4DC),
                    width: 1.0,
                    radius: 3.0.into(),
                },
            })
        })
        .collect()
}

fn strip_formula_markers(draft: &str) -> Option<&str> {
    draft.strip_prefix("{=").and_then(|s| s.strip_suffix('}'))
}

fn eval_preview(value: &str) -> String {
    let expr = strip_formula_markers(value).unwrap_or("");
    eval::eval_display(expr)
}
