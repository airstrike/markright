<div align="center">

# markright

A rich text editor widget for [iced](https://github.com/iced-rs/iced)

[![Made with iced](https://iced.rs/badge.svg)](https://github.com/iced-rs/iced)

</div>

## Overview

`markright` is a rich text editor widget built on `iced` and `cosmic-text`. It
provides formatted text editing with inline styling, paragraph formatting, list
support, and full undo/redo — all through a clean, operation-based API.

> **Note:** This project is under active development and is not yet published to
> crates.io. It depends on custom forks of `iced` and `cosmic-text`.

## Features

- **Inline formatting** — bold, italic, underline, font, size, color
- **Paragraph styles** — alignment (left, center, right, justified), line spacing
- **Lists** — bullet and ordered lists with indent/dedent
- **Undo/redo** — operation-based history with grouping
- **Built-in key bindings** — Cmd+B/I/U, Cmd+Z/Y, Tab/Shift+Tab for lists

## Usage

Add `markright` as a git dependency:

```toml
[dependencies]
markright = { git = "https://github.com/airstrike/markright" }
```

Create an editor in your iced application:

```rust
use markright::widget::rich_editor::{self, Content, Action};

// In your app state
let content = Content::with_text("Hello, world!");

// In your view
rich_editor::rich_editor(&content)
    .on_action(Message::Editor)
    .size(16.0)
    .padding(8)
    .into()

// In your update
fn update(&mut self, message: Message) {
    match message {
        Message::Editor(action) => self.content.perform(action),
        Message::Undo => self.content.perform(Action::Undo),
        Message::Redo => self.content.perform(Action::Redo),
    }
}
```

## Examples

```bash
cargo run -p editor       # Full editor with toolbar
cargo run -p textboxes    # Floating textbox workspace
```

## Architecture

- **`markright`** — the widget crate, providing `Content`, `Action`, and the
  `rich_editor` widget
- **`markright_document`** — the document model, with operation types (`Op`),
  undo/redo history, and style capture utilities

All edits flow through `Content::perform(action)`, are recorded as atomic `Op`
values, and pushed onto the undo stack. `Content` owns the full undo/redo
history — there's nothing external to manage.

## Dependencies

This project uses custom forks with additional features:

- [`airstrike/iced`](https://github.com/airstrike/iced) — rich editor trait,
  variable line heights, paragraph styles
- [`airstrike/cosmic-text`](https://github.com/airstrike/cosmic-text) — font
  features, margin support, layout extensions

## License

MIT
