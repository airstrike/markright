use iced::widget::{center, column, container, text};
use iced::{Element, Length, Padding};

use markright::Content;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Markright")
        .run()
}

struct App {
    content: Content,
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

The editor hides **markdown syntax** when the cursor is away from a line.
Move your cursor to this line to see the `**` markers appear.

### How It Works

- The active line shows ***raw markdown*** with markers visible
- Other lines display **formatted text** with markers hidden
- Click any line to reveal its raw source

## Code Example

```rust
fn main() {
    println!(\"Hello, markright!\");
}
```

Try editing this text! You can use:
- **Bold** with `**double asterisks**`
- *Italic* with `*single asterisks*`
- ***Bold italic*** with `***triple asterisks***`
- `Inline code` with backticks

#### Heading Levels

# Heading 1
## Heading 2
### Heading 3
#### Heading 4
##### Heading 5
###### Heading 6";

        (
            Self {
                content: Content::with_text(sample),
            },
            iced::Task::none(),
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Edit(action) => {
                self.content.perform(action);
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let (cursor_line, cursor_col) = self.content.cursor();
        let status = text(format!("Ln {}, Col {}", cursor_line + 1, cursor_col + 1)).size(12);

        let editor = markright::editor(&self.content)
            .on_action(Message::Edit)
            .padding(20)
            .size(16);

        let content = column![
            container(editor).width(Length::Fill).padding(Padding::ZERO),
            container(status)
                .width(Length::Fill)
                .padding(Padding::ZERO.vertical(4).horizontal(20)),
        ];

        center(content).width(720).into()
    }
}
