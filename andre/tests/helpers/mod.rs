use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;

use andre::app::App;

#[allow(dead_code)]
pub fn make_test_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let config_path = write_test_config(&tmp);
    create_test_package(&tmp);
    let app = App::with_home(config_path, false, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn create_test_package(dir: &TempDir) {
    let pkg_dir = dir.path().join("dotfiles/home/test");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(pkg_dir.join(".bashrc"), "alias l='ls -la'").unwrap();
}

#[allow(dead_code)]
pub fn key_char(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

#[allow(dead_code)]
pub fn enter_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
}

#[allow(dead_code)]
pub fn esc_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
}

#[allow(dead_code)]
pub fn space_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)
}

#[allow(dead_code)]
pub fn up_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)
}

#[allow(dead_code)]
pub fn down_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)
}

#[allow(dead_code)]
pub fn ctrl_c() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
}

#[allow(dead_code)]
pub fn make_empty_app() -> (App, TempDir) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("andre.yml");
    std::fs::write(
        &path,
        r#"
global: {}
groups: {}
"#,
    )
    .unwrap();
    let app = App::with_home(path, false, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}

#[allow(dead_code)]
pub fn component_id(app: &App) -> &str {
    app.component_stack.last().map(|c| c.id()).unwrap_or("None")
}

/// Push a named component onto the stack (for test setup).
#[allow(dead_code)]
pub fn push_component(app: &mut App, name: &str) {
    use andre::components::*;
    app.component_stack.push(match name {
        "MainMenu" => Box::new(MainMenuComponent::default()),
        "GroupSelect" => Box::new(stow::GroupSelectComponent::default()),
        "Settings" => Box::new(settings::SettingsComponent::default()),
        "Status" => Box::new(status::StatusComponent::default()),
        "Help" => Box::new(help::HelpComponent::default()),
        "AdoptTargetSelect" => Box::new(adopt::AdoptTargetSelectComponent::default()),
        "UnstowTargetSelect" => Box::new(unstow::UnstowTargetSelectComponent::default()),
        _ => return,
    });
}

#[allow(dead_code)]
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
    let app = App::with_home(path, false, tmp.path().to_path_buf()).unwrap();
    (app, tmp)
}
