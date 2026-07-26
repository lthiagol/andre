use std::path::PathBuf;

use crossterm::event::KeyEvent;

use andre_core::{config::Config, AppState as CoreState, Result};

use crate::components::{AppContext, Component, InputMode, LogLevel, Transition};
use crate::execute::AsyncExecutor;

pub struct App {
    pub ctx: AppContext,
    pub component_stack: Vec<Box<dyn Component>>,
    /// Owns the `AsyncExecutor` AbortHandle while Execute is on the stack so
    /// Confirm → Execute cannot drop cancel capability. Handed off via
    /// `Transition::Replace(..., Some(executor))`. `None` outside Execute.
    pub executor: Option<AsyncExecutor>,
    pub quitting: bool,
    pub onboard_mode: bool,
}

impl App {
    pub fn new(config_path: PathBuf, yolo: bool) -> Result<Self> {
        Self::with_debug(config_path, yolo, false, None, None)
    }

    pub fn with_home(config_path: PathBuf, yolo: bool, home_dir: PathBuf) -> Result<Self> {
        Self::with_debug(config_path, yolo, false, None, Some(home_dir))
    }

    pub fn with_debug(
        config_path: PathBuf,
        yolo: bool,
        debug: bool,
        log_file: Option<PathBuf>,
        home_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let config = Config::load(&config_path)?;

        let home_dir = home_dir.unwrap_or_else(|| {
            crate::paths::home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        });

        let canonical_config =
            std::fs::canonicalize(&config_path).unwrap_or_else(|_| config_path.clone());
        let config_dir = canonical_config
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        config.validate_sources(&config_dir, &home_dir)?;
        let core = CoreState::from_config(config, yolo);

        Ok(Self {
            ctx: AppContext {
                core,
                config_dir: config_dir.clone(),
                config_path: canonical_config,
                home_dir: home_dir.clone(),
                status_cache: std::cell::RefCell::new(crate::status_cache::StatusCache::new()),
                terminal_size: (80, 24),
                debug,
                log_file,
                event_log: std::cell::RefCell::new(Vec::new()),
                toast: None,
            },
            quitting: false,
            onboard_mode: false,
            executor: None,
            component_stack: vec![Box::new(crate::components::MainMenuComponent::default())],
        })
    }

    pub fn process_transition(&mut self, transition: Transition) {
        self.ctx.log(LogLevel::Transition, transition.name());
        match transition {
            Transition::None | Transition::Handled => {}
            Transition::Push(component) => {
                self.component_stack.push(component);
            }
            Transition::Pop => {
                self.component_stack.pop();
                if self.onboard_mode && self.component_stack.len() <= 1 {
                    self.quitting = true;
                    self.onboard_mode = false;
                }
            }
            Transition::Replace(component, executor) => {
                self.component_stack.pop();
                self.component_stack.push(component);
                // Take abort-handle ownership only when Execute is the new top.
                // Carried on the Transition (not AppContext) so mis-staged
                // hand-offs cannot silently land on a non-Execute screen.
                if let Some(exec) = executor {
                    if self.component_stack.last().map(|c| c.id()) == Some("Execute") {
                        self.executor = Some(exec);
                    }
                }
            }
            Transition::Quit => {
                self.quitting = true;
            }
            Transition::PopAll => {
                self.component_stack.truncate(1);
            }
        }

        // Release abort-handle ownership when Execute leaves the top of the
        // stack. Dropping AsyncExecutor does not stop the task (it holds its
        // own Sender clone); it only drops App's AbortHandle so cancel is
        // available only while Execute is active.
        let on_execute = self.component_stack.last().map(|c| c.id()) == Some("Execute");
        if !on_execute {
            self.executor = None;
        }
    }

    pub fn component_dispatch(&mut self, key: KeyEvent) -> bool {
        self.ctx.log(
            LogLevel::Key,
            format!(
                "key={:?} component={:?}",
                key.code,
                self.component_stack.last().map(|c| c.id())
            ),
        );
        if let Some(component) = self.component_stack.last_mut() {
            let transition = component.handle_key(key, &mut self.ctx);
            match transition {
                Transition::Handled => return true,
                Transition::None => {}
                other => {
                    self.process_transition(other);
                    return true;
                }
            }
            // Component returned None — block globals if in Modal/TextInput mode
            if component.input_mode() != InputMode::Normal {
                return true;
            }
        }
        // Component didn't handle the key — try global shortcuts
        self.handle_global_key(key)
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.component_dispatch(key);
    }

    /// Handle global keys (q, v, d, n, a, o, t). Returns true if handled.
    pub fn handle_global_key(&mut self, key: KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('q') => {
                self.quitting = true;
                true
            }
            KeyCode::Char('v') => {
                self.ctx.core.verbosity = (self.ctx.core.verbosity + 1) % 6;
                true
            }
            KeyCode::Char('d') => {
                self.ctx.core.dry_run = !self.ctx.core.dry_run;
                true
            }
            KeyCode::Char('n') => {
                self.ctx.core.no_folding = !self.ctx.core.no_folding;
                true
            }
            KeyCode::Char('a') => {
                self.ctx.core.adopt = !self.ctx.core.adopt;
                true
            }
            KeyCode::Char('o') => {
                self.ctx.core.dotfiles = !self.ctx.core.dotfiles;
                true
            }
            KeyCode::Char('t') => {
                self.ctx.core.theme = self.ctx.core.theme.next();
                true
            }
            _ => false,
        }
    }

    pub fn update_components(&mut self) {
        self.ctx.tick_toast();
        if let Some(component) = self.component_stack.last_mut() {
            let transition = component.update(&mut self.ctx);
            self.process_transition(transition);
        }
    }

    pub fn is_quitting(&self) -> bool {
        self.quitting
    }

    pub fn set_terminal_size(&mut self, cols: u16, rows: u16) {
        self.ctx.terminal_size = (cols, rows);
    }

    pub fn invalidate_status_cache(&self) {
        self.ctx.status_cache.borrow_mut().invalidate_all();
    }

    pub fn flush_debug_log(&self) {
        if !self.ctx.debug {
            return;
        }
        let entries: Vec<_> = self.ctx.event_log.borrow_mut().drain(..).collect();
        if entries.is_empty() {
            return;
        }
        if let Some(ref path) = self.ctx.log_file {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                use std::io::Write;
                for entry in &entries {
                    let _ = writeln!(file, "[{:?}] {}", entry.level, entry.message);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_config(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("andre.yml");
        std::fs::write(&path, "global: {}\ngroups: {}\n").unwrap();
        path
    }

    #[test]
    fn app_new_defaults_to_system_home() {
        let tmp = tempfile::TempDir::new().unwrap();
        let app = App::new(empty_config(tmp.path()), false).unwrap();
        if let Some(home) = crate::paths::home_dir() {
            assert_eq!(app.ctx.home_dir, home);
        }
    }

    #[test]
    fn app_with_home_injects_override() {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        let app = App::with_home(empty_config(tmp.path()), false, home.clone()).unwrap();
        assert_eq!(app.ctx.home_dir, home);
    }
}
