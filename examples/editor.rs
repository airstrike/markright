use iced::widget::{column, container, text};
use iced::{Color, Element, Fill, Font, Length, Padding, Task};

use markright::Document;

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

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Markright")
        .default_font(Font::with_name("IBM Plex Sans"))
        .run()
}

struct App {
    document: Document,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(markright::Action),
    FontLoaded(Result<(), iced::font::Error>),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let sample = "\
# Welcome to Markright

This is a **WYSIWYG** markdown editor built as a custom *iced* widget.

## Features

The editor hides **markdown syntax** when the cursor is away from a block.
Move your cursor to any block to see the raw markdown.

### How It Works

The active block shows raw markdown with markers visible. Other blocks display formatted text with markers hidden. Click any block to reveal its raw source.

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
                document: Document::from_markdown(sample),
            },
            font_tasks,
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Edit(action) => match &action {
                markright::Action::Insert(ch) => self.document.insert(*ch),
                markright::Action::Delete => self.document.delete(),
                markright::Action::Backspace => self.document.backspace(),
                markright::Action::Enter => self.document.enter(),
                markright::Action::Move(motion) => match motion {
                    markright::Motion::Left => self.document.move_left(),
                    markright::Motion::Right => self.document.move_right(),
                    markright::Motion::Up => self.document.move_up(),
                    markright::Motion::Down => self.document.move_down(),
                    markright::Motion::Home => self.document.move_home(),
                    markright::Motion::End => self.document.move_end(),
                    _ => {}
                },
                markright::Action::Click { block, offset } => {
                    self.document.set_active_block(*block);
                    self.document.set_cursor(*offset);
                }
                _ => {}
            },
            Message::FontLoaded(_) => {
                // Fonts are loaded into the global font system automatically.
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let (cursor_block, cursor_col) = self.document.cursor();
        let status = text(format!(
            "Block {}, Col {}",
            cursor_block + 1,
            cursor_col + 1
        ))
        .size(12)
        .color(Color::from_rgb(0.5, 0.5, 0.5));

        let editor = markright::editor(&self.document)
            .on_action(Message::Edit)
            .padding(20)
            .size(16)
            .monospace_font(Font::with_name("IBM Plex Mono"));

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
