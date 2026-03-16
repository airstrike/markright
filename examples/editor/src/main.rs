mod debug;
mod fonts;
mod icon;
mod pull;
mod theme;
mod toolbar;

use std::path::PathBuf;

use iced::clipboard;
use iced::widget::operation::focus;
use iced::widget::{column, container, mouse_area, row, rule, space, text};
use iced::{Element, Fill, Font, Size, Subscription, Task, window};

use markright::widget::rich_editor;
use markright::widget::rich_editor::{Binding, Content, Format, KeyPress, cursor};

use theme::Theme;

const BASE_SIZE: f32 = 16.0;
const MONO_FONT: &[u8] = include_bytes!("../fonts/GT-Pressura-Mono-Regular.ttf");
const FIRA_CODE: &[u8] = include_bytes!("../fonts/FiraCode-Variable.ttf");

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    iced::application(App::new, App::update, App::view)
        .title("Markright")
        .window_size([1200.0, 800.0])
        .theme(App::theme)
        .font(icon::FONT)
        .font(MONO_FONT)
        .font(FIRA_CODE)
        .default_font(Font::with_family("IBM Plex Sans"))
        .subscription(App::subscription)
        .run()
}

/// Default save path for the editor's document.
fn document_path() -> PathBuf {
    std::env::temp_dir().join("markright").join("scratch.mr")
}

struct App {
    content: Content<iced::Renderer>,
    toolbar: toolbar::State,
    fonts: fount::Fount,
    theme_choice: Theme,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(rich_editor::Action),
    Toolbar(toolbar::Message),
    Font(fonts::Message),
    CopyDebug(String),
    FocusEditor,
    Save,
    Saved(Result<PathBuf, String>),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let content = match std::fs::read_to_string(document_path()) {
            Ok(mr) => Content::parse(&mr).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse saved document: {e}");
                Content::with_text(include_str!("../sample.txt"))
            }),
            Err(_) => Content::with_text(include_str!("../sample.txt")),
        };

        let init_task = fonts::init().map(Message::Font);

        (
            Self {
                content,
                toolbar: toolbar::State::default(),
                fonts: fount::Fount::new(),
                theme_choice: Theme::default(),
            },
            init_task,
        )
    }

    fn theme(&self) -> iced::Theme {
        self.theme_choice.to_theme()
    }

    fn subscription(&self) -> Subscription<Message> {
        toolbar::subscription(&self.toolbar).map(Message::Toolbar)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                self.content.perform(action);
                self.toolbar
                    .sync_from_cursor(&self.content.cursor_context());
                focus("editor")
            }
            Message::Toolbar(msg) => {
                let action = toolbar::update(&mut self.toolbar, msg);
                match action {
                    toolbar::Action::None => Task::none(),
                    toolbar::Action::FocusEditor => focus("editor"),
                    toolbar::Action::Editor(a) => {
                        self.content.perform(a);
                        self.toolbar
                            .sync_from_cursor(&self.content.cursor_context());
                        focus("editor")
                    }
                    toolbar::Action::Pending(a) => {
                        self.content.perform(a);
                        self.toolbar
                            .sync_from_cursor(&self.content.cursor_context());
                        Task::none()
                    }
                    toolbar::Action::FontSelected(name) => {
                        let font = self.fonts.font(&name);
                        self.content.perform(Format::SetFont(font));
                        self.toolbar.rebuild_font_list(&self.fonts, &name);
                        if self.fonts.system_families().contains(&name) {
                            focus("editor")
                        } else {
                            Task::batch([
                                fonts::load(name, self.fonts.google_catalog()).map(Message::Font),
                                focus("editor"),
                            ])
                        }
                    }
                    toolbar::Action::Save => {
                        let mr = self.content.serialize();
                        Task::perform(save(mr), Message::Saved)
                    }
                    toolbar::Action::ToggleTheme => {
                        self.theme_choice = self.theme_choice.toggle();
                        focus("editor")
                    }
                    toolbar::Action::ToggleDebug { opening } => {
                        let resize_task = window::latest().then(move |opt_id| {
                            let Some(id) = opt_id else {
                                return Task::none();
                            };
                            window::size(id).then(move |size| {
                                let delta = debug::PANEL_W;
                                let new_width = if opening {
                                    size.width + delta
                                } else {
                                    (size.width - delta).max(400.0)
                                };
                                window::resize(id, Size::new(new_width, size.height))
                            })
                        });
                        Task::batch([resize_task, focus("editor")])
                    }
                }
            }
            Message::Font(msg) => match msg {
                fonts::Message::SystemFontsLoaded(families) => {
                    let names: Vec<String> = families
                        .into_iter()
                        .map(|f| f.to_string())
                        .filter(|name| !name.starts_with('.'))
                        .collect();
                    self.fonts.set_system_families(names);
                    self.toolbar.rebuild_font_list(&self.fonts, "");
                    focus("editor")
                }
                fonts::Message::CatalogLoaded(Ok(catalog)) => {
                    self.fonts.set_google_catalog(catalog);
                    self.toolbar.rebuild_font_list(&self.fonts, "");
                    focus("editor")
                }
                fonts::Message::CatalogLoaded(Err(e)) => {
                    tracing::warn!("Catalog loading failed: {e}");
                    focus("editor")
                }
                fonts::Message::Loaded(_name, Ok(())) => focus("editor"),
                fonts::Message::Loaded(name, Err(e)) => {
                    tracing::warn!("Font loading failed ({name}): {e}");
                    focus("editor")
                }
            },
            Message::CopyDebug(s) => clipboard::write(s).discard(),
            Message::FocusEditor => focus("editor"),
            Message::Save => {
                let mr = self.content.serialize();
                Task::perform(save(mr), Message::Saved)
            }
            Message::Saved(result) => {
                match &result {
                    Ok(path) => {
                        self.content.mark_saved();
                        tracing::info!("Saved to {}", path.display());
                    }
                    Err(e) => tracing::warn!("Save failed: {e}"),
                }
                focus("editor")
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let cursor = self.content.cursor_context();
        let is_dirty = self.content.is_dirty();
        let can_undo = self.content.can_undo();
        let can_redo = self.content.can_redo();

        let tools = toolbar::view(
            &self.toolbar,
            &cursor,
            self.theme_choice.is_dark(),
            is_dirty,
            can_undo,
            can_redo,
        )
        .map(Message::Toolbar);

        let status_bar = container(status_bar(&cursor)).style(theme::container::toolbar);

        let editor = column![
            rich_editor(&self.content)
                .id("editor")
                .on_action(Message::Editor)
                .key_binding(key_binding)
                .style(theme::text_editor::borderless)
                .padding(20)
                .size(BASE_SIZE),
            mouse_area(space().height(Fill).width(Fill)).on_press(Message::FocusEditor),
        ];

        let body = if !self.toolbar.show_debug() {
            Element::from(container(editor).width(Fill).height(Fill))
        } else {
            let debug_panel = debug::view(&self.content, Message::CopyDebug);

            row![
                container(editor).width(Fill).height(Fill),
                rule::vertical(1).style(theme::rule::separator),
                debug_panel,
            ]
            .into()
        };

        let content = column![
            tools,
            rule::horizontal(1).style(theme::rule::separator),
            body,
            rule::horizontal(1).style(theme::rule::separator),
            status_bar,
        ]
        .width(Fill)
        .height(Fill);

        container(content).center_x(Fill).height(Fill).into()
    }
}

async fn save(mr: String) -> Result<PathBuf, String> {
    let path = document_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, mr).map_err(|e| e.to_string())?;
    Ok(path)
}

fn key_binding(key_press: KeyPress) -> Option<Binding<Message>> {
    let KeyPress { key, modifiers, .. } = &key_press;

    // Cmd+S → Save
    if let Some('s') = key.to_latin(key_press.physical_key) {
        if modifiers.command() {
            return Some(Binding::Custom(Message::Save));
        }
    }

    // Fall through to default bindings
    None
}

fn status_bar(cursor: &cursor::Context) -> Element<'static, Message> {
    container(
        text(format!(
            "Line {}, Col {}",
            cursor.position.line + 1,
            cursor.position.column + 1,
        ))
        .size(12)
        .style(theme::text::status_bar),
    )
    .width(Fill)
    .padding([4, 20])
    .into()
}
