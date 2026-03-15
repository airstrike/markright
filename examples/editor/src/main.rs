mod debug;
mod fonts;
mod icon;
mod theme;
mod toolbar;

use iced::clipboard;
use iced::widget::operation::focus;
use iced::widget::{column, combo_box, container, mouse_area, row, space, text};
use iced::{Element, Fill, Font, Size, Task, window};

use markright::widget::rich_editor::{self, Action, Content, Format, cursor};

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
        .theme(App::theme)
        .font(icon::FONT)
        .font(MONO_FONT)
        .font(FIRA_CODE)
        .default_font(Font::with_family("IBM Plex Sans"))
        .run()
}

struct App {
    content: Content<iced::Renderer>,
    fonts: fount::Fount,
    system_fonts: Vec<String>,
    recent_fonts: Vec<String>,
    font_list: combo_box::State<String>,
    size_list: combo_box::State<String>,
    theme_choice: Theme,
    show_debug: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    Font(fonts::Message),
    FontSelected(String),
    SizeSelected(String),
    ToggleTheme,
    ToggleDebug,
    CopyDebug(String),
    FocusEditor,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let sample = include_str!("../sample.txt");

        let init_task = fonts::init().map(Message::Font);

        let font_list = combo_box::State::new(vec!["IBM Plex Sans".to_string()]);

        let size_list = combo_box::State::new(
            [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 64, 72]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );

        (
            Self {
                content: Content::with_text(sample),
                fonts: fount::Fount::new(),
                system_fonts: Vec::new(),
                recent_fonts: Vec::new(),
                font_list,
                size_list,
                theme_choice: Theme::default(),
                show_debug: false,
            },
            init_task,
        )
    }

    fn theme(&self) -> iced::Theme {
        self.theme_choice.to_theme()
    }

    /// Rebuild the font combo-box: recently-used first, then the rest
    /// alphabetically. If `promote` is non-empty, move it to most-recent.
    fn rebuild_font_list(&mut self, promote: &str) {
        if !promote.is_empty() {
            self.recent_fonts.retain(|n| n != promote);
            self.recent_fonts.insert(0, promote.to_string());
        }

        let mut names: Vec<String> = self
            .fonts
            .google_catalog()
            .map(|c| c.top(100))
            .unwrap_or_default();

        // Merge in system fonts.
        for name in &self.system_fonts {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }

        names.sort();

        // Move recent picks to the front, most-recent first.
        for (i, recent) in self.recent_fonts.iter().enumerate() {
            if let Some(pos) = names.iter().position(|n| n == recent) {
                names.remove(pos);
                names.insert(i, recent.clone());
            }
        }

        self.font_list = combo_box::State::new(names);
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                self.content.perform(action);
                focus("editor")
            }
            Message::FontSelected(name) => {
                let font = self.fonts.font(&name);
                self.content.perform(Format::SetFont(font));
                self.rebuild_font_list(&name);
                if self.system_fonts.contains(&name) {
                    // Already available via the system font database.
                    focus("editor")
                } else {
                    Task::batch([fonts::load(name).map(Message::Font), focus("editor")])
                }
            }
            Message::SizeSelected(size_str) => {
                if let Ok(size) = size_str.parse::<f32>() {
                    self.content.perform(Format::SetFontSize(size));
                }
                focus("editor")
            }
            Message::ToggleTheme => {
                self.theme_choice = self.theme_choice.toggle();
                focus("editor")
            }
            Message::ToggleDebug => {
                self.show_debug = !self.show_debug;
                let opening = self.show_debug;
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
            Message::CopyDebug(s) => clipboard::write(s).discard(),
            Message::Font(msg) => match msg {
                fonts::Message::SystemFontsLoaded(families) => {
                    self.system_fonts = families
                        .into_iter()
                        .map(|f| f.to_string())
                        .filter(|name| !name.starts_with('.'))
                        .collect();
                    self.system_fonts.sort();
                    self.system_fonts.dedup();
                    self.rebuild_font_list("");
                    focus("editor")
                }
                fonts::Message::CatalogLoaded(Ok(catalog)) => {
                    self.fonts.set_google_catalog(catalog);
                    self.rebuild_font_list("");
                    focus("editor")
                }
                fonts::Message::CatalogLoaded(Err(e)) => {
                    eprintln!("Catalog loading failed: {e}");
                    focus("editor")
                }
                fonts::Message::Loaded(_name, Ok(())) => {
                    self.content
                        .set_default_font(Font::with_family("IBM Plex Sans"));
                    focus("editor")
                }
                fonts::Message::Loaded(name, Err(e)) => {
                    eprintln!("Font loading failed ({name}): {e}");
                    focus("editor")
                }
            },
            Message::FocusEditor => focus("editor"),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let cursor = self.content.cursor_context();
        let can_undo = self.content.can_undo();
        let can_redo = self.content.can_redo();
        let tools = toolbar(
            &cursor,
            &self.font_list,
            &self.size_list,
            self.theme_choice.is_dark(),
            can_undo,
            can_redo,
            self.show_debug,
        );
        let status_bar = status_bar(&cursor);

        let editor = column![
            rich_editor::rich_editor(&self.content)
                .id("editor")
                .on_action(Message::Editor)
                .style(theme::text_editor::borderless)
                .padding(20)
                .size(BASE_SIZE),
            mouse_area(space().height(Fill).width(Fill)).on_press(Message::FocusEditor),
        ];

        let body: Element<'_, Message> = if self.show_debug {
            let debug_panel = container(debug::view(&self.content, Message::CopyDebug))
                .style(theme::container::debug_panel)
                .height(Fill);

            row![container(editor).width(Fill).height(Fill), debug_panel,].into()
        } else {
            container(editor).width(Fill).height(Fill).into()
        };

        let content = column![tools, body, status_bar].width(Fill).height(Fill);

        container(content).center_x(Fill).height(Fill).into()
    }
}

fn toolbar<'a>(
    cursor: &cursor::Context,
    font_list: &'a combo_box::State<String>,
    size_list: &'a combo_box::State<String>,
    is_dark: bool,
    can_undo: bool,
    can_redo: bool,
    show_debug: bool,
) -> Element<'a, Message> {
    toolbar::view(
        cursor,
        font_list,
        size_list,
        is_dark,
        can_undo,
        can_redo,
        show_debug,
        Message::Editor,
        Message::FontSelected,
        Message::SizeSelected,
        Message::ToggleTheme,
        Message::ToggleDebug,
    )
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
