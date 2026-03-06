use iced::widget::{column, container, text, text_editor};
use iced::{Color, Element, Fill, Font, Length, Padding, Task};

use markright::{HighlightSettings, MarkdownHighlighter};

/// IBM Plex Sans Regular
const PLEX_SANS_REGULAR: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-sans/fonts/complete/ttf/IBMPlexSans-Regular.ttf";
/// IBM Plex Sans Bold
const PLEX_SANS_BOLD: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-sans/fonts/complete/ttf/IBMPlexSans-Bold.ttf";
/// IBM Plex Sans Italic
const PLEX_SANS_ITALIC: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-sans/fonts/complete/ttf/IBMPlexSans-Italic.ttf";
/// IBM Plex Sans Bold Italic
const PLEX_SANS_BOLD_ITALIC: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-sans/fonts/complete/ttf/IBMPlexSans-BoldItalic.ttf";
/// IBM Plex Mono Regular
const PLEX_MONO_REGULAR: &str = "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-mono/fonts/complete/ttf/IBMPlexMono-Regular.ttf";

const BASE_SIZE: f32 = 16.0;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Markright")
        .default_font(Font::with_name("IBM Plex Sans"))
        .run()
}

struct App {
    content: text_editor::Content,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    FontLoaded(Result<(), iced::font::Error>),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let sample = "\
# Welcome to Markright

This is a **WYSIWYG** markdown editor built with *iced*.

## Features

The editor hides **markdown syntax** when it forms valid constructs.
Type `# Heading` and it becomes a heading. Type `**bold**` and it becomes bold.

### How It Works

A custom `MarkdownHighlighter` parses each line and applies formatting in real-time. Markers like `#`, `**`, and backticks are hidden by coloring them to match the background.

## Code Example

```rust
fn main() {
    println!(\"Hello, markright!\");
}
```

Try editing this text! You can use **bold**, *italic*, ***bold italic***, and `inline code`.

---

#### Heading Levels

# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6";

        let font_tasks = Task::batch(
            [
                PLEX_SANS_REGULAR,
                PLEX_SANS_BOLD,
                PLEX_SANS_ITALIC,
                PLEX_SANS_BOLD_ITALIC,
                PLEX_MONO_REGULAR,
            ]
            .into_iter()
            .map(|url| {
                Task::future(fetch_font(url.to_owned()))
                    .then(iced::font::load)
                    .map(Message::FontLoaded)
            }),
        );

        (
            Self {
                content: text_editor::Content::with_text(sample),
            },
            font_tasks,
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Edit(action) => {
                self.content.perform(action);
            }
            Message::FontLoaded(_) => {}
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let highlight_settings = HighlightSettings {
            font: Font::with_name("IBM Plex Sans"),
            mono_font: Font::with_name("IBM Plex Mono"),
            base_size: BASE_SIZE,
            background_color: Color::WHITE,
        };

        let editor = text_editor(&self.content)
            .on_action(Message::Edit)
            .highlight_with::<MarkdownHighlighter>(highlight_settings, |highlight, _theme| {
                highlight.to_format()
            })
            .padding(20)
            .size(BASE_SIZE);

        let cursor = self.content.cursor();
        let status = text(format!(
            "Line {}, Col {}",
            cursor.position.line + 1,
            cursor.position.column + 1,
        ))
        .size(12)
        .color(Color::from_rgb(0.5, 0.5, 0.5));

        let content = column![
            editor,
            container(status)
                .width(Fill)
                .padding(Padding::ZERO.vertical(4).horizontal(20)),
        ]
        .width(Fill)
        .height(Fill);

        container(content)
            .center_x(Length::Fill)
            .max_width(720)
            .height(Fill)
            .into()
    }
}

/// Fetch font bytes from a URL.
async fn fetch_font(url: String) -> Vec<u8> {
    reqwest::get(&url)
        .await
        .expect("font fetch failed")
        .bytes()
        .await
        .expect("font bytes failed")
        .to_vec()
}
