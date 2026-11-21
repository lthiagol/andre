use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{layout::Rect, Frame};

use andre_core::AppState as CoreState;

use crate::status_cache::StatusCache;
use crate::ui::footer::{screen_hints, Hint};

/// Default how long a toast stays visible.
pub const TOAST_TTL: Duration = Duration::from_secs(3);

/// Visual severity for toast chrome (success vs error colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastKind {
    #[default]
    Success,
    Error,
}

/// Non-blocking app-level feedback (does not capture keys).
#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub deadline: Instant,
    pub kind: ToastKind,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Key,
    Transition,
    Exec,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

pub mod browser;
pub mod confirm;
pub mod execute;
pub mod help;
pub mod ignore;
pub mod menu;
pub mod packages;
pub mod profiles;
pub mod settings;
pub mod status;
pub mod target_select;
pub mod unstow;
pub mod wizard;

pub use menu::MainMenuComponent;

/// Resources shared by all components.
pub struct AppContext {
    pub core: CoreState,
    pub config_dir: PathBuf,
    pub config_path: PathBuf,
    pub home_dir: PathBuf,
    /// The folder plain `andre` reads its config from: `$XDG_CONFIG_HOME/andre`, or
    /// `$HOME/.config/andre`. `None` when no base resolves.
    pub search_dir: Option<PathBuf>,
    pub status_cache: RefCell<StatusCache>,
    pub terminal_size: (u16, u16),
    pub debug: bool,
    pub log_file: Option<PathBuf>,
    pub event_log: RefCell<Vec<LogEntry>>,
    /// Single toast; newer replaces older. Cleared by `tick_toast`.
    pub toast: Option<Toast>,
}

/// Input mode for a component. Controls whether global shortcuts are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    /// Global shortcuts are allowed (list screens, menus).
    #[default]
    Normal,
    /// A modal popup is open — global shortcuts are blocked.
    Modal,
    /// A text input field is active — global shortcuts are blocked.
    TextInput,
}

#[cfg(test)]
impl AppContext {
    /// A context over `config` with `dir` as both config and home directory.
    pub(crate) fn for_tests(config: andre_core::config::Config, dir: &std::path::Path) -> Self {
        Self {
            core: CoreState::from_config(config),
            config_dir: dir.to_path_buf(),
            config_path: dir.join("andre.yml"),
            home_dir: dir.to_path_buf(),
            search_dir: Some(dir.join(".config").join("andre")),
            status_cache: RefCell::new(StatusCache::new()),
            terminal_size: (80, 24),
            debug: false,
            log_file: None,
            event_log: RefCell::new(Vec::new()),
            toast: None,
        }
    }
}

impl AppContext {
    /// Record a log entry. Nothing runs when debug is disabled: `message` is only
    /// evaluated (and allocated) once a debug session is recording.
    pub fn log(&self, level: LogLevel, message: impl FnOnce() -> String) {
        if !self.debug {
            return;
        }
        self.event_log.borrow_mut().push(LogEntry {
            level,
            message: message(),
        });
    }

    /// Show a short-lived success toast (replaces any existing toast).
    pub fn show_toast(&mut self, message: impl Into<String>) {
        self.show_toast_kind(message, ToastKind::Success, TOAST_TTL);
    }

    /// Show a short-lived error toast (replaces any existing toast).
    pub fn show_error_toast(&mut self, message: impl Into<String>) {
        self.show_toast_kind(message, ToastKind::Error, TOAST_TTL);
    }

    /// Show a toast with kind + explicit TTL (tests may use a short duration).
    /// Empty / whitespace-only messages are ignored.
    pub fn show_toast_kind(&mut self, message: impl Into<String>, kind: ToastKind, ttl: Duration) {
        let message = message.into();
        if message.trim().is_empty() {
            return;
        }
        self.toast = Some(Toast {
            message,
            deadline: Instant::now() + ttl,
            kind,
        });
    }

    /// Clear toast when its deadline has passed.
    pub fn tick_toast(&mut self) {
        if self
            .toast
            .as_ref()
            .is_some_and(|t| Instant::now() >= t.deadline)
        {
            self.toast = None;
        }
    }
}

impl Transition {
    pub fn name(&self) -> &'static str {
        match self {
            Transition::None => "None",
            Transition::Handled => "Handled",
            Transition::Push(_) => "Push",
            Transition::Pop => "Pop",
            Transition::Replace(_) => "Replace",
            Transition::Quit => "Quit",
            Transition::PopAll => "PopAll",
        }
    }
}

/// Navigation transitions returned by components after handling input or updates.
pub enum Transition {
    /// The key was not handled; let the global handler process it.
    None,
    /// The key was handled, but no screen transition is needed.
    Handled,
    /// Push a new component onto the stack.
    Push(Box<dyn Component>),
    /// Pop the current component from the stack.
    Pop,
    /// Replace the current component with a new one.
    Replace(Box<dyn Component>),
    /// Mark the application as quitting; the main loop exits after this frame.
    Quit,
    /// Pop all components except the root (MainMenu).
    PopAll,
}

/// The core trait for all UI screens.
pub trait Component {
    /// Unique identifier for the component (useful for testing/debugging).
    fn id(&self) -> &'static str;

    /// Downcast to `Any` for testing access to component state.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Mutable downcast to `Any` for testing.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Handle keyboard input.
    fn handle_key(&mut self, key: KeyEvent, ctx: &mut AppContext) -> Transition;

    /// Update internal state (called on every tick).
    fn update(&mut self, _ctx: &mut AppContext) -> Transition {
        Transition::None
    }

    /// Returns the current input mode. Defaults to `Normal`.
    /// Components with text input or modal popups should override this.
    fn input_mode(&self) -> InputMode {
        InputMode::Normal
    }

    /// The key hints the footer shows while this screen is on top. Defaults to the fixed
    /// hints for `id()`. A screen whose hints depend on its state or its open dialog
    /// overrides this. It must not touch the disk: paint calls it.
    fn hints(&self, _ctx: &AppContext) -> Vec<Hint> {
        screen_hints(self.id())
    }

    /// The crumbs this screen adds to the title row, such as `packages` or `home/shell › ignore`.
    /// Defaults to the screen's own label; a screen about one entity names the entity, and a
    /// sub-screen adds its own crumb after that.
    fn crumbs(&self, _ctx: &AppContext) -> Vec<Cow<'static, str>> {
        vec![Cow::Borrowed(crate::ui::breadcrumb_label(self.id()))]
    }

    /// Pre-render bookkeeping using the area the component will be painted
    /// into. Viewport/scroll mutations belong here (or in `update`), NOT
    /// inside `render`. Called by `ui::render` immediately before `render`
    /// so paint draws from already-prepared state.
    fn prepare(&mut self, _area: Rect) {}

    /// Handle a mouse click or wheel notch inside the content area. The screen's hit rects
    /// were stored by `prepare`, so they match what is on screen. Clicks on the top bar and
    /// the footer never reach here; `App::mouse_dispatch` sends them as keys.
    fn handle_mouse(&mut self, _ev: MouseEvent, _ctx: &mut AppContext) -> Transition {
        Transition::None
    }

    /// Whether `App::mouse_dispatch` holds back list clicks and wheel notches. Defaults to any
    /// non-`Normal` input mode: an open dialog or text field takes the input. A screen whose
    /// text field is part of the screen, not a dialog, overrides this so its lists still
    /// take the mouse.
    fn mouse_blocked(&self) -> bool {
        self.input_mode() != InputMode::Normal
    }

    /// Render the component.
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext);
}
