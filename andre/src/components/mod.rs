use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

use andre_core::AppState as CoreState;

use crate::status_cache::StatusCache;

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
    Info,
    Key,
    Transition,
    Exec,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

pub mod adopt;
pub mod confirm;
pub mod execute;
pub mod help;
pub mod menu;
pub mod settings;
pub mod status;
pub mod stow;
pub mod unstow;

pub use menu::MainMenuComponent;

/// Resources shared by all components.
pub struct AppContext {
    pub core: CoreState,
    pub config_dir: PathBuf,
    pub config_path: PathBuf,
    pub home_dir: PathBuf,
    pub status_cache: RefCell<StatusCache>,
    pub terminal_size: (u16, u16),
    pub debug: bool,
    pub log_file: Option<PathBuf>,
    pub event_log: RefCell<Vec<LogEntry>>,
    /// Single toast; newer replaces older. Cleared by `tick_toast`.
    pub toast: Option<Toast>,
    /// Pre-formatted breadcrumb for the current component stack (e.g.
    /// `"Main > Groups > bash-env"`). Refreshed each frame in
    /// `ui::render`; components read it when building panel titles.
    pub breadcrumb: String,
    /// Last component stack we formatted `breadcrumb` for. The render loop
    /// compares the live stack to this Vec and only re-runs `format_breadcrumb`
    /// (which allocates a Vec + String) when it changes — typically rare
    /// (Enter/Esc transitions), not per-frame.
    pub breadcrumb_cache: Vec<&'static str>,
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

impl AppContext {
    /// Record a log entry. No-op when debug is disabled (zero overhead on hot path).
    pub fn log(&self, level: LogLevel, message: String) {
        if !self.debug {
            return;
        }
        self.event_log
            .borrow_mut()
            .push(LogEntry { level, message });
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

    /// Show a toast with an explicit TTL (tests may use a short duration).
    pub fn show_toast_for(&mut self, message: impl Into<String>, ttl: Duration) {
        self.show_toast_kind(message, ToastKind::Success, ttl);
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
    pub fn name(&self) -> String {
        match self {
            Transition::None => "None".into(),
            Transition::Handled => "Handled".into(),
            Transition::Push(_) => "Push".into(),
            Transition::Pop => "Pop".into(),
            Transition::Replace(_, _) => "Replace".into(),
            Transition::Quit => "Quit".into(),
            Transition::PopAll => "PopAll".into(),
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
    /// Optional `AsyncExecutor` is taken by `App` when the replacement is Execute
    /// (abort-handle ownership for the execute lifetime). Use `None` otherwise.
    Replace(Box<dyn Component>, Option<crate::execute::AsyncExecutor>),
    /// Pop all components and exit the application.
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

    /// Pre-render bookkeeping using the area the component will be painted
    /// into. Viewport/scroll mutations belong here (or in `update`), NOT
    /// inside `render`. Called by `ui::render` immediately before `render`
    /// so paint draws from already-prepared state.
    fn prepare(&mut self, _area: Rect) {}

    /// Render the component.
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &AppContext);
}
