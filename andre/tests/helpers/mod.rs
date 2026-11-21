#![allow(dead_code)]

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
use tempfile::TempDir;
use unicode_width::UnicodeWidthChar;

use andre::app::App;

pub fn make_test_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let config_path = write_test_config(&tmp);
    create_test_package(&tmp);
    let app = App::with_home(config_path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

/// A flat app: one packages folder (`dotfiles`) and no groups, holding the package `test`
/// (`dotfiles/test/.bashrc`). HOME is the temp dir, so the target `~` is the temp dir.
pub fn make_flat_test_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        "packages: dotfiles\nglobal:\n  stow:\n    verbosity: 0\n",
    )
    .unwrap();
    let pkg_dir = tmp.path().join("dotfiles/test");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(pkg_dir.join(".bashrc"), "alias l='ls -la'").unwrap();
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

/// A flat app whose packages folder `dotfiles` holds one package per name in `packages`, each
/// with one file, `.rc`. HOME is the temp dir, so the target `~` is the temp dir.
pub fn make_flat_packages_app(packages: &[&str]) -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        "packages: dotfiles\nglobal:\n  stow:\n    verbosity: 0\n",
    )
    .unwrap();
    let folder = tmp.path().join("dotfiles");
    std::fs::create_dir_all(&folder).unwrap();
    for name in packages {
        let dir = folder.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".rc"), "x").unwrap();
    }
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

pub fn write_test_config(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("andre.yml");
    std::fs::write(
        &path,
        r#"
global:
  stow:
    verbosity: 0
groups:
  home:
    source: dotfiles/home
    target: "~"
"#,
    )
    .unwrap();
    path
}

pub fn create_test_package(dir: &TempDir) {
    let pkg_dir = dir.path().join("dotfiles/home/test");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(pkg_dir.join(".bashrc"), "alias l='ls -la'").unwrap();
}

pub fn key_char(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

pub fn enter_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
}

pub fn esc_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
}

pub fn space_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
}

pub fn up_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)
}

pub fn down_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)
}

pub fn ctrl_c() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
}

/// An app whose config has no packages folder yet, as `--onboard` writes it. HOME is the temp
/// dir.
pub fn make_empty_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(&path, "global: {}\n").unwrap();
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

pub fn component_id(app: &App) -> &str {
    app.component_stack.last().map(|c| c.id()).unwrap_or("None")
}

/// Push a named component onto the stack (for test setup).
pub fn push_component(app: &mut App, name: &str) {
    use andre::components::*;
    app.component_stack.push(match name {
        "MainMenu" => Box::new(MainMenuComponent::default()),
        "Packages" => Box::new(packages::PackagesComponent::default()),
        "Settings" => Box::new(settings::SettingsComponent::default()),
        "Status" => Box::new(status::StatusComponent::default()),
        "Help" => Box::new(help::HelpComponent::default()),
        "NewPackage" => Box::new(wizard::NewPackageWizard::default()),
        _ => return,
    });
}

pub fn make_two_group_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        r#"
global:
  stow:
    verbosity: 0
groups:
  home:
    source: dotfiles/home
    target: "~"
  config:
    source: dotfiles/config
    target: ~/.config
"#,
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("dotfiles/home/bash-env")).unwrap();
    std::fs::write(
        tmp.path().join("dotfiles/home/bash-env/.bashrc"),
        "export PATH",
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("dotfiles/config/nvim")).unwrap();
    std::fs::write(
        tmp.path().join("dotfiles/config/nvim/init.lua"),
        "vim.cmd('set nu')",
    )
    .unwrap();
    let app = App::with_home(path, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

/// An app whose loaded config folder sits inside package `andre` of group `config`, so the
/// set-up item is offered: `dotfiles/config/andre/.config/andre/andre.yml` links to
/// `<home>/.config/andre`, which does not exist yet.
pub fn make_bootstrap_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let source = tmp.path().join("dotfiles").join("config");
    let folder = source.join("andre").join(".config").join("andre");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let config_path = folder.join("andre.yml");
    std::fs::write(
        &config_path,
        format!(
            "global: {{}}\ngroups:\n  config:\n    source: \"{}\"\n    target: \"{}\"\n",
            source.display(),
            home.display()
        ),
    )
    .unwrap();
    let app = App::with_home(config_path, home).unwrap();
    (app, tmp)
}

/// A flat bootstrap app: the packages folder `dotfiles` holds the package `andre`, whose
/// `.config/andre/andre.yml` is the running config. HOME is `home`, which has no `.config`
/// yet, so the package's link (`~/.config/andre`) is free.
pub fn make_flat_bootstrap_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let packages = tmp.path().join("dotfiles");
    let folder = packages.join("andre").join(".config").join("andre");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let config_path = folder.join("andre.yml");
    std::fs::write(
        &config_path,
        format!(
            "packages: \"{}\"\nglobal:\n  stow:\n    verbosity: 0\n",
            packages.display()
        ),
    )
    .unwrap();
    let app = App::with_home(config_path, home).unwrap();
    (app, tmp)
}

/// `make_flat_test_app` with two stowed links into the `test` package, `.bashrc` and
/// `.profile`, and the package's `.profile` file in place.
pub fn make_unstow_app() -> (App, TempDir) {
    let (app, tmp) = make_flat_test_app();
    std::fs::write(tmp.path().join("dotfiles/test/.profile"), "export X=1").unwrap();
    std::os::unix::fs::symlink("dotfiles/test/.bashrc", tmp.path().join(".bashrc")).unwrap();
    std::os::unix::fs::symlink("dotfiles/test/.profile", tmp.path().join(".profile")).unwrap();
    (app, tmp)
}

/// Pushes the unstow browse screen opened at `folder` (the target, unless a test browses
/// a folder inside it), and updates it.
pub fn push_browse(app: &mut App, folder: &std::path::Path) {
    app.component_stack.push(Box::new(
        andre::components::unstow::UnstowBrowseComponent::at(folder.to_path_buf()),
    ));
    app.update_components();
}

/// Moves the browse cursor onto the entry called `name`, with the down key.
pub fn browse_cursor_to(app: &mut App, name: &str) {
    let (at, current, len) = {
        let browse: &andre::components::unstow::UnstowBrowseComponent = app
            .component_stack
            .last()
            .unwrap()
            .as_any()
            .downcast_ref()
            .unwrap();
        let entries = browse.browser.entries();
        let at = entries
            .iter()
            .position(|e| e.name == name)
            .expect("entry listed");
        (at, browse.list.cursor, entries.len())
    };
    for _ in 0..(at + len - current) % len {
        app.component_dispatch(down_key());
    }
}

/// Sends a left click at column `x`, row `y`, as the terminal would.
pub fn click(app: &mut App, x: u16, y: u16) {
    mouse(app, MouseEventKind::Down(MouseButton::Left), x, y);
}

/// Sends one wheel notch at column `x`, row `y`: down when `down`, else up.
pub fn wheel(app: &mut App, x: u16, y: u16, down: bool) {
    let kind = if down {
        MouseEventKind::ScrollDown
    } else {
        MouseEventKind::ScrollUp
    };
    mouse(app, kind, x, y);
}

fn mouse(app: &mut App, kind: MouseEventKind, x: u16, y: u16) {
    app.mouse_dispatch(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    });
}

/// Draws `app` at `width` x `height` into a test buffer. As in the real loop, the frame is
/// what stores the screen's hit rects and the chrome's clickable parts.
pub fn draw(app: &mut App, width: u16, height: u16) -> Buffer {
    app.set_terminal_size(width, height);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| andre::ui::render(app, f)).unwrap();
    terminal.backend().buffer().clone()
}

/// The text of row `y` of `buf`, one symbol per cell.
pub fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area().width)
        .map(|x| buf[(x, y)].symbol())
        .collect()
}

/// The column and row where `text` first starts in `buf`, scanning rows top to bottom.
pub fn find_text(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    for y in 0..buf.area().height {
        let row = row_text(buf, y);
        if let Some(byte) = row.find(text) {
            let column: usize = row[..byte]
                .chars()
                .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                .sum();
            return Some((column as u16, y));
        }
    }
    None
}
