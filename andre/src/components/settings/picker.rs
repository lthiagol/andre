use andre_core::{StowAction, Theme};
use crossterm::event::{KeyCode, KeyEvent};

use crate::components::settings::{PickerField, PickerState, SettingsComponent};

pub fn open_picker(
    component: &mut SettingsComponent,
    title: &str,
    field: PickerField,
    current_value: &str,
) {
    let options: Vec<String> = match field {
        PickerField::Action => vec!["stow", "restow", "unstow"]
            .into_iter()
            .map(String::from)
            .collect(),
        PickerField::Theme => vec![
            "default",
            "dracula",
            "catppuccin mocha",
            "catppuccin latte",
            "catppuccin frappe",
            "catppuccin macchiato",
            "nord",
            "gruvbox",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        _ => vec!["ON".to_string(), "OFF".to_string()],
    };

    let selected_index = options
        .iter()
        .position(|o| o.to_lowercase() == current_value.to_lowercase())
        .unwrap_or(0);

    component.picker = Some(PickerState {
        options,
        selected_index,
        title: title.to_string(),
        field,
    });
}

pub fn handle_picker_key(component: &mut SettingsComponent, key: &KeyEvent) -> bool {
    if let Some(ref mut picker) = component.picker {
        if key.code == KeyCode::Esc {
            component.picker = None;
        } else if crate::input::Input::handle_list_navigation(
            key,
            &mut picker.selected_index,
            picker.options.len(),
        ) {
        } else if key.code == KeyCode::Enter {
            let pending = component.pending.as_mut().unwrap();
            let field = picker.field;
            let selected = picker.options[picker.selected_index].clone();
            component.picker = None;
            component.config_dirty = true;
            match field {
                PickerField::Action => {
                    if let Some(a) = StowAction::from_str(&selected) {
                        pending.action = a;
                    }
                }
                PickerField::Theme => {
                    if let Some(t) = Theme::from_str(&selected) {
                        pending.theme = t;
                    }
                }
                PickerField::DryRun => pending.dry_run = selected == "ON",
                PickerField::NoFolding => pending.no_folding = selected == "ON",
                PickerField::Adopt => pending.adopt = selected == "ON",
                PickerField::Dotfiles => pending.dotfiles = selected == "ON",
            }
        }
        return true;
    }
    false
}
