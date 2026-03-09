mod debug;
mod fonts;
mod icon;
mod theme;
mod toolbar;

use iced::clipboard;
use iced::widget::operation::focus;
use iced::widget::{column, combo_box, container, mouse_area, row, space, text};
use iced::{Element, Fill, Font, Size, Task, window};

use markright::widget::rich_editor::{self, Action, Content, Edit, FormatAction, cursor};

use fonts::gfonts;
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
        .default_font(Font::with_name("IBM Plex Sans"))
        .run()
}

struct App {
    content: Content<iced::Renderer>,
    catalog: Option<gfonts::Catalog>,
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

        let font_list = combo_box::State::new(vec![
            "Fira Code".to_string(),
            "GT Pressura Mono".to_string(),
            "IBM Plex Sans".to_string(),
        ]);

        let size_list = combo_box::State::new(
            [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 64, 72]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );

        (
            Self {
                content: Content::with_text(sample),
                catalog: None,
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

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                self.content.perform(action);
                focus("editor")
            }
            Message::FontSelected(name) => {
                self.content
                    .perform(Action::Edit(Edit::Format(FormatAction::SetFont(
                        Font::with_name(gfonts::intern(&name)),
                    ))));
                Task::batch([fonts::load(name).map(Message::Font), focus("editor")])
            }
            Message::SizeSelected(size_str) => {
                if let Ok(size) = size_str.parse::<f32>() {
                    self.content
                        .perform(Action::Edit(Edit::Format(FormatAction::SetFontSize(size))));
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
                fonts::Message::CatalogLoaded(Ok(catalog)) => {
                    let mut names = catalog.top(100);
                    // Keep bundled fonts available even if not in the top 100.
                    for bundled in ["Fira Code", "GT Pressura Mono"] {
                        if !names.iter().any(|n| n == bundled) {
                            names.push(bundled.to_string());
                        }
                    }
                    self.font_list = combo_box::State::new(names);
                    self.catalog = Some(catalog);
                    focus("editor")
                }
                fonts::Message::CatalogLoaded(Err(e)) => {
                    eprintln!("Catalog loading failed: {e}");
                    focus("editor")
                }
                fonts::Message::Loaded(Ok(())) => {
                    self.content
                        .set_default_font(Font::with_name("IBM Plex Sans"));
                    focus("editor")
                }
                fonts::Message::Loaded(Err(e)) => {
                    eprintln!("Font loading failed: {e}");
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
