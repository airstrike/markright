mod debug;
mod fonts;
mod icon;
mod theme;
mod toolbar;

use iced::widget::operation::focus;
use iced::widget::{column, container, mouse_area, row, space, text};
use iced::{Element, Fill, Font, Task};

use markright::widget::rich_editor::{self, Action, Content, cursor};

use theme::Theme;

const BASE_SIZE: f32 = 16.0;
const MONO_FONT: &[u8] = include_bytes!("../fonts/GT-Pressura-Mono-Regular.ttf");

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Markright")
        .theme(App::theme)
        .font(icon::FONT)
        .font(MONO_FONT)
        .default_font(Font::with_name("IBM Plex Sans"))
        .run()
}

struct App {
    content: Content<iced::Renderer>,
    theme_choice: Theme,
    show_debug: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(Action),
    Font(fonts::Message),
    ToggleTheme,
    ToggleDebug,
    FocusEditor,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let sample = include_str!("../sample.txt");

        let font_tasks = fonts::load_defaults().map(Message::Font);

        (
            Self {
                content: Content::with_text(sample),
                theme_choice: Theme::default(),
                show_debug: false,
            },
            font_tasks,
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
            Message::ToggleTheme => {
                self.theme_choice = self.theme_choice.toggle();
                focus("editor")
            }
            Message::ToggleDebug => {
                self.show_debug = !self.show_debug;
                focus("editor")
            }
            Message::Font(res) => {
                if let fonts::Message::Loaded(Err(e)) = res {
                    eprintln!("Font loading failed: {e:?}");
                }
                focus("editor")
            }
            Message::FocusEditor => focus("editor"),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let cursor = self.content.cursor_context();
        let can_undo = self.content.can_undo();
        let can_redo = self.content.can_redo();
        let tools = toolbar(&cursor, self.theme_choice.is_dark(), can_undo, can_redo);
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
            let debug_panel = container(debug::view::<Message>(&self.content))
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

fn toolbar(
    cursor: &cursor::Context,
    is_dark: bool,
    can_undo: bool,
    can_redo: bool,
) -> Element<'static, Message> {
    toolbar::view(
        cursor,
        is_dark,
        can_undo,
        can_redo,
        Message::Editor,
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
