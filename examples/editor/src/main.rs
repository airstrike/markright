mod fonts;
#[allow(dead_code)]
mod icon;
mod theme;
mod toolbar;

use iced::widget::{column, container, text};
use iced::{Element, Fill, Font, Length, Task, padding};

use markright::widget::rich_editor::{self, Action, Content};

use theme::ThemeChoice;

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
    theme_choice: ThemeChoice,
}

#[derive(Debug, Clone)]
enum Message {
    EditorAction(Action),
    Font(fonts::Message),
    ToggleTheme,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let sample = include_str!("../sample.txt");

        let font_tasks = fonts::load_defaults().map(Message::Font);

        (
            Self {
                content: Content::with_text(sample),
                theme_choice: ThemeChoice::default(),
            },
            font_tasks,
        )
    }

    fn theme(&self) -> iced::Theme {
        self.theme_choice.to_theme()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::EditorAction(action) => self.content.perform(action),
            Message::ToggleTheme => self.theme_choice = self.theme_choice.toggle(),
            Message::Font(_) => {}
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let ctx = self.content.cursor_context();
        let tools = toolbar::view(
            &ctx,
            self.theme_choice.is_dark(),
            Message::EditorAction,
            Message::ToggleTheme,
        );

        let editor = container(
            rich_editor::rich_editor(&self.content)
                .on_action(Message::EditorAction)
                .style(theme::text_editor::borderless)
                .padding(20)
                .size(BASE_SIZE),
        )
        .height(Fill)
        .width(Fill);

        let status = text(format!(
            "Line {}, Col {}",
            ctx.position.line + 1,
            ctx.position.column + 1,
        ))
        .size(12)
        .style(theme::text::status_bar);

        let content = column![
            tools,
            editor,
            container(status)
                .width(Fill)
                .padding(padding::vertical(4).horizontal(20)),
        ]
        .width(Fill)
        .height(Fill);

        container(content)
            .center_x(Length::Fill)
            .height(Fill)
            .into()
    }
}
