mod debug;
mod fonts;
mod icon;
mod pull;
mod theme;
mod toolbar;

use std::path::{Path, PathBuf};

use markright_document::format as mr;
use markright_document::markdown;
use markright_document::{Format as _, StyledLine};

use iced::clipboard;
use iced::widget::operation::focus;
use iced::widget::{column, container, mouse_area, row, rule, space, text};
use iced::{Element, Fill, Font, Size, Subscription, Task, window};

use markright::widget::rich_editor;
use markright::widget::rich_editor::{Binding, Content, Format, KeyPress, cursor};

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
        .window_size([1200.0, 800.0])
        .theme(App::theme)
        .font(icon::FONT)
        .font(MONO_FONT)
        .font(FIRA_CODE)
        .default_font(Font::with_family("IBM Plex Sans"))
        .subscription(App::subscription)
        .run()
}

/// Path to the state file (remembers the last-opened document).
fn state_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/markright/last_file")
    } else {
        std::env::temp_dir().join("markright/last_file")
    }
}

/// Read the remembered last-opened file path, if any, and only if the
/// file still exists on disk.
fn load_last_file() -> Option<PathBuf> {
    let text = std::fs::read_to_string(state_path()).ok()?;
    let path = PathBuf::from(text.trim());
    path.exists().then_some(path)
}

/// Persist the given path as the last-opened file.
fn remember_last_file(path: &Path) {
    let state = state_path();
    if let Some(parent) = state.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&state, path.to_string_lossy().as_bytes()) {
        tracing::warn!("Failed to persist last file: {e}");
    }
}

struct App {
    content: Content<iced::Renderer>,
    toolbar: toolbar::State,
    fonts: fount::Fount,
    theme_choice: Theme,
    /// Path of the currently-open file (if any). Updated on Open/SaveAs.
    current_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(rich_editor::Action),
    Toolbar(toolbar::Message),
    Font(fonts::Message),
    CopyDebug(String),
    FocusEditor,
    Save,
    Saved(Result<PathBuf, String>),
    Open,
    Opened(Result<(PathBuf, Vec<StyledLine>), String>),
}

/// Where should a save go: an existing path, or a Save-As dialog?
enum SaveTarget {
    Existing(PathBuf),
    Prompt,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let sample = || {
            mr::parse(include_str!("../sample.mr"))
                .map(|lines| Content::from_styled_lines(&lines))
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to parse sample: {e}");
                    Content::with_text("")
                })
        };

        // Try to restore the last-opened file; fall back to the bundled sample.
        let (content, current_path) = match load_last_file() {
            Some(path) => match std::fs::read_to_string(&path) {
                Ok(text) => {
                    let lines = parse_by_extension(&path, &text);
                    let c = Content::from_styled_lines(&lines);
                    c.mark_saved();
                    (c, Some(path))
                }
                Err(e) => {
                    tracing::warn!("Failed to read last file: {e}");
                    (sample(), None)
                }
            },
            None => (sample(), None),
        };

        let init_task = fonts::init().map(Message::Font);

        (
            Self {
                content,
                toolbar: toolbar::State::default(),
                fonts: fount::Fount::new(),
                theme_choice: Theme::default(),
                current_path,
            },
            init_task,
        )
    }

    fn theme(&self) -> iced::Theme {
        self.theme_choice.to_theme()
    }

    fn subscription(&self) -> Subscription<Message> {
        toolbar::subscription(&self.toolbar).map(Message::Toolbar)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                self.content.perform(action);
                self.toolbar
                    .sync_from_cursor(&self.content.cursor_context());
                focus("editor")
            }
            Message::Toolbar(msg) => {
                let action = toolbar::update(&mut self.toolbar, msg);
                match action {
                    toolbar::Action::None => Task::none(),
                    toolbar::Action::FocusEditor => focus("editor"),
                    toolbar::Action::Editor(a) => {
                        self.content.perform(a);
                        self.toolbar
                            .sync_from_cursor(&self.content.cursor_context());
                        focus("editor")
                    }
                    toolbar::Action::Pending(a) => {
                        self.content.perform(a);
                        self.toolbar
                            .sync_from_cursor(&self.content.cursor_context());
                        Task::none()
                    }
                    toolbar::Action::FontSelected(name) => {
                        let font = self.fonts.font(&name);
                        self.content.perform(Format::SetFont(font));
                        self.toolbar.rebuild_font_list(&self.fonts, &name);
                        if self.fonts.system_families().contains(&name) {
                            focus("editor")
                        } else {
                            Task::batch([
                                fonts::load(name, self.fonts.google_catalog()).map(Message::Font),
                                focus("editor"),
                            ])
                        }
                    }
                    toolbar::Action::Save => {
                        let mr = mr::serialize(&self.content.styled_lines());
                        let target = match &self.current_path {
                            Some(p) => SaveTarget::Existing(p.clone()),
                            None => SaveTarget::Prompt,
                        };
                        Task::perform(save(mr, target), Message::Saved)
                    }
                    toolbar::Action::Open => Task::perform(open(), Message::Opened),
                    toolbar::Action::ToggleTheme => {
                        self.theme_choice = self.theme_choice.toggle();
                        focus("editor")
                    }
                    toolbar::Action::ToggleDebug { opening } => {
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
                }
            }
            Message::Font(msg) => match msg {
                fonts::Message::SystemFontsLoaded(families) => {
                    let names: Vec<String> = families
                        .into_iter()
                        .map(|f| f.to_string())
                        .filter(|name| !name.starts_with('.'))
                        .collect();
                    self.fonts.set_system_families(names);
                    self.toolbar.rebuild_font_list(&self.fonts, "");
                    focus("editor")
                }
                fonts::Message::CatalogLoaded(Ok(catalog)) => {
                    self.fonts.set_google_catalog(catalog);
                    self.toolbar.rebuild_font_list(&self.fonts, "");
                    focus("editor")
                }
                fonts::Message::CatalogLoaded(Err(e)) => {
                    tracing::warn!("Catalog loading failed: {e}");
                    focus("editor")
                }
                fonts::Message::Loaded(_name, Ok(())) => focus("editor"),
                fonts::Message::Loaded(name, Err(e)) => {
                    tracing::warn!("Font loading failed ({name}): {e}");
                    focus("editor")
                }
            },
            Message::CopyDebug(s) => clipboard::write(s).discard(),
            Message::FocusEditor => focus("editor"),
            Message::Save => {
                let mr = mr::serialize(&self.content.styled_lines());
                let target = match &self.current_path {
                    Some(p) => SaveTarget::Existing(p.clone()),
                    None => SaveTarget::Prompt,
                };
                Task::perform(save(mr, target), Message::Saved)
            }
            Message::Saved(result) => {
                match result {
                    Ok(path) => {
                        self.content.mark_saved();
                        remember_last_file(&path);
                        tracing::info!("Saved to {}", path.display());
                        self.current_path = Some(path);
                    }
                    Err(e) => tracing::warn!("Save failed: {e}"),
                }
                focus("editor")
            }
            Message::Open => Task::perform(open(), Message::Opened),
            Message::Opened(result) => {
                match result {
                    Ok((path, lines)) => {
                        self.content = Content::from_styled_lines(&lines);
                        self.content.mark_saved();
                        remember_last_file(&path);
                        self.toolbar
                            .sync_from_cursor(&self.content.cursor_context());
                        tracing::info!("Opened {}", path.display());
                        self.current_path = Some(path);
                    }
                    Err(e) => tracing::warn!("Open failed: {e}"),
                }
                focus("editor")
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let cursor = self.content.cursor_context();
        let is_dirty = self.content.is_dirty();
        let can_undo = self.content.can_undo();
        let can_redo = self.content.can_redo();

        let tools = toolbar::view(
            &self.toolbar,
            &cursor,
            self.theme_choice.is_dark(),
            is_dirty,
            can_undo,
            can_redo,
        )
        .map(Message::Toolbar);

        let status_bar = container(status_bar(&cursor, self.current_path.as_deref()))
            .style(theme::container::toolbar);

        let editor = column![
            rich_editor(&self.content)
                .id("editor")
                .on_action(Message::Editor)
                .key_binding(key_binding)
                .style(theme::text_editor::borderless)
                .padding(20)
                .size(BASE_SIZE),
            mouse_area(space().height(Fill).width(Fill)).on_press(Message::FocusEditor),
        ];

        let body = if !self.toolbar.show_debug() {
            Element::from(container(editor).width(Fill).height(Fill))
        } else {
            let debug_panel = debug::view(&self.content, Message::CopyDebug);

            row![
                container(editor).width(Fill).height(Fill),
                rule::vertical(1).style(theme::rule::separator),
                debug_panel,
            ]
            .into()
        };

        let content = column![
            tools,
            rule::horizontal(1).style(theme::rule::separator),
            body,
            rule::horizontal(1).style(theme::rule::separator),
            status_bar,
        ]
        .width(Fill)
        .height(Fill);

        container(content).center_x(Fill).height(Fill).into()
    }
}

async fn save(text: String, target: SaveTarget) -> Result<PathBuf, String> {
    let path = match target {
        SaveTarget::Existing(p) => p,
        SaveTarget::Prompt => rfd::AsyncFileDialog::new()
            .add_filter("Markright", &["mr"])
            .add_filter("Markdown", &["md", "markdown"])
            .add_filter("Text", &["txt"])
            .set_file_name("untitled.mr")
            .save_file()
            .await
            .ok_or_else(|| "save cancelled".to_string())?
            .path()
            .to_path_buf(),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Show a native file-open dialog and load the chosen file into styled lines.
/// Dispatches to the right parser based on extension (`.mr`, `.md`/`.markdown`,
/// or plain text as fallback).
async fn open() -> Result<(PathBuf, Vec<StyledLine>), String> {
    let handle = rfd::AsyncFileDialog::new()
        .add_filter("Markright", &["mr"])
        .add_filter("Markdown", &["md", "markdown"])
        .add_filter("Text", &["txt"])
        .add_filter("All", &["*"])
        .pick_file()
        .await
        .ok_or_else(|| "no file selected".to_string())?;

    let path: PathBuf = handle.path().to_path_buf();
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let lines = parse_by_extension(&path, &text);
    Ok((path, lines))
}

fn parse_by_extension(path: &Path, text: &str) -> Vec<StyledLine> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "md" | "markdown" => markdown::Markdown::parse(text).unwrap_or_default(),
        "mr" => mr::parse(text).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse .mr file: {e}");
            vec![StyledLine {
                text: text.to_string(),
                runs: vec![],
                paragraph: Default::default(),
            }]
        }),
        _ => text
            .lines()
            .map(|line| StyledLine {
                text: line.to_string(),
                runs: vec![],
                paragraph: Default::default(),
            })
            .collect(),
    }
}

fn key_binding(key_press: KeyPress) -> Option<Binding<Message>> {
    let KeyPress { key, modifiers, .. } = &key_press;

    // Cmd+S → Save
    if let Some('s') = key.to_latin(key_press.physical_key)
        && modifiers.command()
    {
        return Some(Binding::Custom(Message::Save));
    }
    // Cmd+O → Open
    if let Some('o') = key.to_latin(key_press.physical_key)
        && modifiers.command()
    {
        return Some(Binding::Custom(Message::Open));
    }

    // Fall through to default bindings
    None
}

fn status_bar(cursor: &cursor::Context, path: Option<&Path>) -> Element<'static, Message> {
    let position = text(format!(
        "Line {}, Col {}",
        cursor.position.line + 1,
        cursor.position.column + 1,
    ))
    .size(12)
    .style(theme::text::status_bar);

    let path_label = text(match path {
        Some(p) => display_path(p),
        None => "(unsaved)".to_string(),
    })
    .size(12)
    .style(theme::text::status_bar);

    container(row![position, space().width(Fill), path_label].align_y(iced::Alignment::Center))
        .width(Fill)
        .padding([4, 20])
        .into()
}

/// Render a path with `~` for the home directory and truncation if too long.
fn display_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let shortened = match std::env::var("HOME") {
        Ok(home) if raw.starts_with(&home) => format!("~{}", &raw[home.len()..]),
        _ => raw.into_owned(),
    };
    const MAX: usize = 80;
    if shortened.len() > MAX {
        let start = shortened.len() - (MAX - 1);
        format!("…{}", &shortened[start..])
    } else {
        shortened
    }
}
