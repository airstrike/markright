use iced::widget::{column, container, text};
use iced::{Color, Element, Fill, Length, Padding};

use markright::Document;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Markright")
        .run()
}

struct App {
    document: Document,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(markright::Action),
}

impl App {
    fn new() -> (Self, iced::Task<Message>) {
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

        (
            Self {
                document: Document::from_markdown(sample),
            },
            iced::Task::none(),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Edit(action) => match &action {
                markright::Action::Insert(ch) => self.document.insert(*ch),
                markright::Action::Delete => self.document.delete(),
                markright::Action::Backspace => self.document.backspace(),
                markright::Action::Enter => self.document.enter(),
                markright::Action::Move(_motion) => {
                    // TODO: implement cursor movement in document model
                }
                markright::Action::Click { block, offset } => {
                    self.document.set_active_block(*block);
                    self.document.set_cursor(*offset);
                }
                _ => {}
            },
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
            .size(16);

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
