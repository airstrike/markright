mod fonts;
mod icon;
mod theme;
mod toolbar;

use iced::widget::operation::focus;
use iced::widget::{column, container, mouse_area, space, text};
use iced::{Element, Fill, Font, Task, padding};

use markright::widget::rich_editor::{self, Action, Content, cursor};

use theme::Theme;

const BASE_SIZE: f32 = 16.0;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Markright")
        .theme(App::theme)
        .font(icon::FONT)
        .default_font(Font::with_name("IBM Plex Sans"))
        .run()
}

struct App {
    content: Content<iced::Renderer>,
    theme_choice: Theme,
}

#[derive(Debug, Clone)]
enum Message {
    EditorAction(Action),
    Font(fonts::Message),
    ToggleTheme,
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
            },
            font_tasks,
        )
    }

    fn theme(&self) -> iced::Theme {
        self.theme_choice.to_theme()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditorAction(action) => self.content.perform(action),
            Message::ToggleTheme => self.theme_choice = self.theme_choice.toggle(),
            Message::Font(res) => {
                if let fonts::Message::Loaded(Err(e)) = res {
                    eprintln!("Font loading failed: {e:?}");
                }
            }
            Message::FocusEditor => return focus("editor"),
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let cursor = self.content.cursor_context();
        let tools = toolbar(&cursor, self.theme_choice.is_dark());
        let status_bar = status_bar(&cursor);

        let editor = column![
            rich_editor::rich_editor(&self.content)
                .id("editor")
                .on_action(Message::EditorAction)
                .style(theme::text_editor::borderless)
                .padding(20)
                .size(BASE_SIZE),
            mouse_area(space().height(Fill).width(Fill)).on_press(Message::FocusEditor),
        ];

        let content = column![tools, editor, status_bar].width(Fill).height(Fill);

        container(content).center_x(Fill).height(Fill).into()
    }
}

fn toolbar(cursor: &cursor::Context, is_dark: bool) -> Element<'static, Message> {
    toolbar::view(cursor, is_dark, Message::EditorAction, Message::ToggleTheme)
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
    .padding(padding::vertical(4).horizontal(20))
    .into()
}
