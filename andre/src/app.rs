use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use andre_core::profile::{ProfileSet, PROFILES_DIR};
use andre_core::{config::Config, AppState as CoreState, Result};

use crate::components::{AppContext, Component, InputMode, LogLevel, Transition};
use crate::input::flag_for_key;
use crate::ui::hits::{key_at, ChromeHits};

pub struct App {
    pub ctx: AppContext,
    pub component_stack: Vec<Box<dyn Component>>,
    pub quitting: bool,
    pub onboard_mode: bool,
    /// The top bar's and the footer's clickable parts, as drawn by the last frame.
    pub chrome_hits: ChromeHits,
}

impl App {
    pub fn new(config_path: PathBuf) -> Result<Self> {
        Self::with_debug(config_path, false, None, None)
    }

    pub fn with_home(config_path: PathBuf, home_dir: PathBuf) -> Result<Self> {
        Self::with_debug(config_path, false, None, Some(home_dir))
    }

    pub fn with_debug(
        config_path: PathBuf,
        debug: bool,
        log_file: Option<PathBuf>,
        home_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let config = Config::load(&config_path)?;

        // An injected home (tests) ignores `$XDG_CONFIG_HOME`, so the result does not depend
        // on the host environment.
        let search_dir = match &home_dir {
            Some(home) => Some(home.join(".config").join("andre")),
            None => crate::paths::config_dir().map(|dir| dir.join("andre")),
        };
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
        let mut core = CoreState::from_config(config);
        core.profiles = ProfileSet::load(&config_dir.join(PROFILES_DIR))?;

        Ok(Self {
            ctx: AppContext {
                core,
                config_dir,
                config_path: canonical_config,
                home_dir,
                search_dir,
                status_cache: std::cell::RefCell::new(crate::status_cache::StatusCache::new()),
                terminal_size: (80, 24),
                debug,
                log_file,
                event_log: std::cell::RefCell::new(Vec::new()),
                toast: None,
            },
            quitting: false,
            onboard_mode: false,
            chrome_hits: ChromeHits::default(),
            component_stack: vec![Box::new(crate::components::MainMenuComponent::default())],
        })
    }

    pub fn process_transition(&mut self, transition: Transition) {
        self.ctx
            .log(LogLevel::Transition, || transition.name().to_string());
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
            Transition::Replace(component) => {
                self.component_stack.pop();
                self.component_stack.push(component);
            }
            Transition::Quit => {
                self.quitting = true;
            }
            Transition::PopAll => {
                self.component_stack.truncate(1);
            }
        }
    }

    pub fn component_dispatch(&mut self, key: KeyEvent) -> bool {
        self.ctx.log(LogLevel::Key, || {
            format!(
                "key={:?} component={:?}",
                key.code,
                self.component_stack.last().map(|c| c.id())
            )
        });
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

    /// Handle global keys (q, s, v, d, n, a, o, t). Returns true if handled.
    pub fn handle_global_key(&mut self, key: KeyEvent) -> bool {
        if let Some(flag) = flag_for_key(key.code) {
            self.ctx.core.flags.toggle(flag);
            return true;
        }
        match key.code {
            KeyCode::Char('q') => {
                self.quitting = true;
                true
            }
            KeyCode::Char('s') => {
                self.ctx.core.flags.action = self.ctx.core.flags.action.next();
                true
            }
            KeyCode::Char('t') => {
                self.ctx.core.theme = self.ctx.core.theme.next();
                true
            }
            _ => false,
        }
    }

    /// Routes a mouse event. Only a left click and the wheel do anything. A click on the top
    /// bar or the footer sends the key that part shows: a top-bar chip only when the screen
    /// takes no text, the same rule as keys. Anything else goes to the screen, unless a dialog
    /// or a text field is open: those take the input, so clicks outside them change nothing.
    pub fn mouse_dispatch(&mut self, ev: MouseEvent) {
        let click = ev.kind == MouseEventKind::Down(MouseButton::Left);
        let wheel = matches!(
            ev.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        );
        if !click && !wheel {
            return;
        }
        // The bars answer clicks only, so a wheel notch over a chip or a hint does nothing.
        if click {
            if let Some(key) = key_at(&self.chrome_hits.top_bar, ev.column, ev.row) {
                if self.top_mode() == InputMode::Normal {
                    self.handle_global_key(key);
                }
                return;
            }
            if let Some(key) = key_at(&self.chrome_hits.footer, ev.column, ev.row) {
                // `t` and `q` are typed characters in a text field, so their chips do nothing
                // there, as the top bar's do. The other hints go to the screen.
                let typed = matches!(key.code, KeyCode::Char('t' | 'q'));
                if !typed || self.top_mode() == InputMode::Normal {
                    self.component_dispatch(key);
                }
                return;
            }
        }
        if let Some(component) = self.component_stack.last_mut() {
            if component.mouse_blocked() {
                return;
            }
            let transition = component.handle_mouse(ev, &mut self.ctx);
            self.process_transition(transition);
        }
    }

    /// The input mode of the screen on top, the one that takes keys and clicks.
    fn top_mode(&self) -> InputMode {
        self.component_stack
            .last()
            .map_or(InputMode::Normal, |c| c.input_mode())
    }

    pub fn update_components(&mut self) {
        self.ctx.tick_toast();
        if let Some(component) = self.component_stack.last_mut() {
            let transition = component.update(&mut self.ctx);
            self.process_transition(transition);
        }
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
        std::fs::write(&path, "global: {}\n").unwrap();
        path
    }

    #[test]
    fn app_new_defaults_to_system_home() {
        let tmp = tempfile::TempDir::new().unwrap();
        let app = App::new(empty_config(tmp.path())).unwrap();
        if let Some(home) = crate::paths::home_dir() {
            assert_eq!(app.ctx.home_dir, home);
        }
    }

    #[test]
    fn app_loads_profiles_from_profiles_dir_next_to_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let profiles = tmp.path().join("profiles");
        std::fs::create_dir_all(&profiles).unwrap();
        std::fs::write(profiles.join("work.yml"), "packages: [git]\n").unwrap();

        let app = App::with_home(empty_config(tmp.path()), tmp.path().to_path_buf()).unwrap();
        let work = app.ctx.core.profiles.get("work").expect("profile loaded");
        assert_eq!(work.packages, ["git"]);
        assert!(app.ctx.core.profiles.issues().is_empty());
    }

    #[test]
    fn app_with_no_profiles_folder_has_empty_profile_set() {
        let tmp = tempfile::TempDir::new().unwrap();
        let app = App::with_home(empty_config(tmp.path()), tmp.path().to_path_buf()).unwrap();
        assert_eq!(app.ctx.core.profiles.iter().count(), 0);
    }

    #[test]
    fn app_with_home_injects_override() {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        let app = App::with_home(empty_config(tmp.path()), home.clone()).unwrap();
        assert_eq!(app.ctx.home_dir, home);
    }
}
