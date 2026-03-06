use iced::widget::text_editor::{Action, Binding, Cursor, Edit, KeyPress, Motion, Position};
use iced::widget::{column, container, text, text_editor};
use iced::{Color, Element, Fill, Font, Length, Padding, Task};

use std::sync::{Arc, RwLock};

use markright::markdown::{self, MarkdownAction};
use markright::toolbar::{ToolbarAction, ToolbarState};
use markright::{HighlightSettings, RichDocument, RichTextHighlighter};

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
    document: Arc<RwLock<RichDocument>>,
    doc_version: u64,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    FontLoaded(Result<(), iced::font::Error>),
    ToggleBold,
    ToggleItalic,
    ToggleUnderline,
    Toolbar(ToolbarAction),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let sample = "\
Welcome to Markright

This is a WYSIWYG rich text editor built with iced.

Features

The editor reads formatting from a RichDocument model.
Bold, italic, and heading formatting are applied via the document API.

How It Works

A RichTextHighlighter reads formatting spans from the RichDocument and converts them into visual formatting in real-time.

Try editing this text!";

        let line_count = sample.lines().count();
        let document = Arc::new(RwLock::new(RichDocument::with_lines(line_count)));

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
                document,
                doc_version: 0,
            },
            font_tasks,
        )
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Edit(action) => {
                let is_edit = action.is_edit();
                let before_lines = self.content.line_count();
                let before_cursor = self.content.cursor();

                self.content.perform(action);

                if is_edit {
                    let after_lines = self.content.line_count();
                    let after_cursor = self.content.cursor();
                    self.sync_document(&before_cursor, before_lines, &after_cursor, after_lines);
                    self.doc_version += 1;
                    self.detect_and_apply_markdown();
                }
            }
            Message::ToggleBold | Message::Toolbar(ToolbarAction::ToggleBold) => {
                self.apply_formatting(|doc, line, range| {
                    doc.toggle_bold(line, range);
                });
            }
            Message::ToggleItalic | Message::Toolbar(ToolbarAction::ToggleItalic) => {
                self.apply_formatting(|doc, line, range| {
                    doc.toggle_italic(line, range);
                });
            }
            Message::ToggleUnderline | Message::Toolbar(ToolbarAction::ToggleUnderline) => {
                self.apply_formatting(|doc, line, range| {
                    doc.toggle_underline(line, range);
                });
            }
            Message::Toolbar(ToolbarAction::SetHeadingLevel(level)) => {
                let cursor = self.content.cursor();
                let mut doc = self.document.write().expect("document lock poisoned");
                doc.line_format_mut(cursor.position.line).heading_level = level;
                self.doc_version += 1;
            }
            Message::Toolbar(ToolbarAction::SetAlignment(alignment)) => {
                let cursor = self.content.cursor();
                let mut doc = self.document.write().expect("document lock poisoned");
                doc.line_format_mut(cursor.position.line).alignment = alignment;
                self.doc_version += 1;
            }
            Message::FontLoaded(_) => {}
        }
    }

    /// Apply a formatting operation to the current selection across all
    /// affected lines.
    fn apply_formatting(
        &mut self,
        apply: impl Fn(&mut RichDocument, usize, std::ops::Range<usize>),
    ) {
        let cursor = self.content.cursor();
        let Some(sel_pos) = cursor.selection else {
            return;
        };

        let (start, end) = ordered_positions(&cursor.position, &sel_pos);

        let mut doc = self.document.write().expect("document lock poisoned");
        for line in start.line..=end.line {
            let col_start = if line == start.line { start.column } else { 0 };
            let col_end = if line == end.line {
                end.column
            } else {
                self.content.line(line).map(|l| l.text.len()).unwrap_or(0)
            };
            if col_start < col_end {
                apply(&mut doc, line, col_start..col_end);
            }
        }
        self.doc_version += 1;
    }

    /// Compute the current toolbar state from the cursor position.
    fn toolbar_state(&self) -> ToolbarState {
        let cursor = self.content.cursor();
        let doc = self.document.read().expect("document lock poisoned");
        let line = cursor.position.line;
        if line >= doc.line_count() {
            return ToolbarState::default();
        }
        let span = doc.format_at(line, cursor.position.column);
        let line_fmt = doc.line_format(line);
        ToolbarState::from_document(&span, line_fmt)
    }

    /// Sync the RichDocument with the Content after an edit action.
    fn sync_document(
        &mut self,
        before_cursor: &text_editor::Cursor,
        before_lines: usize,
        after_cursor: &text_editor::Cursor,
        after_lines: usize,
    ) {
        let mut doc = self.document.write().expect("document lock poisoned");
        let before = &before_cursor.position;
        let after = &after_cursor.position;

        if after_lines > before_lines {
            // Lines were added (e.g. Enter key). Split at the before cursor
            // position for each new line.
            let lines_added = after_lines - before_lines;
            for _ in 0..lines_added {
                if before.line < doc.line_count() {
                    doc.split_line(before.line, before.column);
                }
            }
        } else if after_lines < before_lines {
            // Lines were removed (e.g. Backspace at line start, or Delete at
            // line end, or selection-delete spanning lines).
            let lines_removed = before_lines - after_lines;
            for _ in 0..lines_removed {
                if after.line < doc.line_count().saturating_sub(1) {
                    doc.merge_lines(after.line);
                }
            }
        }

        // Same-line edit: adjust spans for character insertions/deletions.
        if after_lines == before_lines && before.line == after.line {
            if after.column > before.column {
                // Characters were inserted.
                let inserted = after.column - before.column;
                doc.insert_at(before.line, before.column, inserted);
            } else if after.column < before.column {
                // Characters were deleted.
                doc.delete_range(before.line, after.column, before.column);
            }
        }

        // Safety net: ensure line counts always match.
        doc.ensure_lines(after_lines);
    }

    /// Detect completed markdown patterns on the current cursor line and apply
    /// the corresponding formatting. Markers are removed from the Content and
    /// formatting is applied in the RichDocument.
    fn detect_and_apply_markdown(&mut self) {
        let cursor = self.content.cursor();
        let line_idx = cursor.position.line;

        let line_text = match self.content.line(line_idx) {
            Some(l) => l.text.to_string(),
            None => return,
        };

        let actions = markdown::detect_patterns(&line_text);
        if actions.is_empty() {
            return;
        }

        for action in actions {
            match action {
                MarkdownAction::Heading { level, marker } => {
                    self.remove_range_from_content(line_idx, &marker);

                    let mut doc = self.document.write().expect("document lock poisoned");
                    doc.delete_range(line_idx, marker.start, marker.end);
                    doc.line_format_mut(line_idx).heading_level = Some(level);
                    self.doc_version += 1;
                }
                MarkdownAction::Bold {
                    content: _,
                    markers,
                    ..
                } => {
                    let adjusted_content = self.remove_markers_from_content(line_idx, &markers);

                    let mut doc = self.document.write().expect("document lock poisoned");
                    self.remove_markers_from_document(&mut doc, line_idx, &markers);
                    doc.toggle_bold(line_idx, adjusted_content);
                    self.doc_version += 1;
                }
                MarkdownAction::Italic {
                    content: _,
                    markers,
                    ..
                } => {
                    let adjusted_content = self.remove_markers_from_content(line_idx, &markers);

                    let mut doc = self.document.write().expect("document lock poisoned");
                    self.remove_markers_from_document(&mut doc, line_idx, &markers);
                    doc.toggle_italic(line_idx, adjusted_content);
                    self.doc_version += 1;
                }
                MarkdownAction::BoldItalic {
                    content: _,
                    markers,
                    ..
                } => {
                    let adjusted_content = self.remove_markers_from_content(line_idx, &markers);

                    let mut doc = self.document.write().expect("document lock poisoned");
                    self.remove_markers_from_document(&mut doc, line_idx, &markers);
                    doc.toggle_bold(line_idx, adjusted_content.clone());
                    doc.toggle_italic(line_idx, adjusted_content);
                    self.doc_version += 1;
                }
                MarkdownAction::Code {
                    content: _,
                    markers,
                    ..
                } => {
                    let adjusted_content = self.remove_markers_from_content(line_idx, &markers);

                    let mut doc = self.document.write().expect("document lock poisoned");
                    self.remove_markers_from_document(&mut doc, line_idx, &markers);
                    let mono_font = Font::with_name("IBM Plex Mono");
                    doc.set_format_property(line_idx, adjusted_content, |f| {
                        f.font = Some(mono_font);
                    });
                    self.doc_version += 1;
                }
            }
        }
    }

    /// Remove a byte range from Content on a given line by moving the cursor
    /// to the end of the range, selecting backwards, and deleting.
    fn remove_range_from_content(&mut self, line: usize, range: &std::ops::Range<usize>) {
        let range_len = range.end - range.start;
        if range_len == 0 {
            return;
        }

        // Move cursor to the end of the range.
        self.content.move_to(Cursor {
            position: Position {
                line,
                column: range.end,
            },
            selection: None,
        });

        // Select backwards across the range.
        for _ in 0..range_len {
            self.content.perform(Action::Select(Motion::Left));
        }

        // Delete the selection.
        self.content.perform(Action::Edit(Edit::Backspace));
    }

    /// Remove marker ranges from Content (right-to-left to preserve positions)
    /// and return the adjusted content range after marker removal.
    fn remove_markers_from_content(
        &mut self,
        line: usize,
        markers: &[std::ops::Range<usize>],
    ) -> std::ops::Range<usize> {
        // Sort markers by start position in reverse order so we can remove
        // from right to left without invalidating earlier positions.
        let mut sorted_markers: Vec<_> = markers.to_vec();
        sorted_markers.sort_by(|a, b| b.start.cmp(&a.start));

        // Track total bytes removed before the content range to compute the
        // adjusted content range.
        // First, find the content range: it's between the first and last marker.
        let first_marker_end = markers
            .iter()
            .map(|m| m.end)
            .min()
            .expect("markers should be non-empty");
        let last_marker_start = markers
            .iter()
            .map(|m| m.start)
            .max()
            .expect("markers should be non-empty");

        let content_start = first_marker_end;
        let content_end = last_marker_start;

        // Remove markers from right to left.
        let mut removed_before_content = 0usize;
        for marker in &sorted_markers {
            self.remove_range_from_content(line, marker);
            if marker.end <= content_start {
                removed_before_content += marker.end - marker.start;
            }
        }

        let adjusted_start = content_start - removed_before_content;
        let adjusted_end = content_end - removed_before_content;
        adjusted_start..adjusted_end
    }

    /// Remove markers from the RichDocument's span tracking (right-to-left).
    fn remove_markers_from_document(
        &self,
        doc: &mut RichDocument,
        line: usize,
        markers: &[std::ops::Range<usize>],
    ) {
        // Remove from right to left to keep earlier positions valid.
        let mut sorted_markers: Vec<_> = markers.to_vec();
        sorted_markers.sort_by(|a, b| b.start.cmp(&a.start));

        for marker in &sorted_markers {
            doc.delete_range(line, marker.start, marker.end);
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let toolbar_state = self.toolbar_state();
        let toolbar_el = markright::toolbar(&toolbar_state, Message::Toolbar);

        let highlight_settings = HighlightSettings {
            font: Font::with_name("IBM Plex Sans"),
            base_size: BASE_SIZE,
            document: Arc::clone(&self.document),
            version: self.doc_version,
        };

        let editor = text_editor(&self.content)
            .on_action(Message::Edit)
            .key_binding(|key_press| {
                let KeyPress {
                    key,
                    physical_key,
                    modifiers,
                    ..
                } = &key_press;

                let is_cmd = modifiers.command();
                match key.to_latin(*physical_key) {
                    Some('b') if is_cmd => Some(Binding::Custom(Message::ToggleBold)),
                    Some('i') if is_cmd => Some(Binding::Custom(Message::ToggleItalic)),
                    Some('u') if is_cmd => Some(Binding::Custom(Message::ToggleUnderline)),
                    _ => Binding::from_key_press(key_press),
                }
            })
            .highlight_with::<RichTextHighlighter>(highlight_settings, |highlight, _theme| {
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
            toolbar_el,
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

/// Order two cursor positions so that `start` comes before `end`.
fn ordered_positions<'a>(a: &'a Position, b: &'a Position) -> (&'a Position, &'a Position) {
    if a.line < b.line || (a.line == b.line && a.column <= b.column) {
        (a, b)
    } else {
        (b, a)
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
