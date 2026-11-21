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
        PickerField::Action => StowAction::ALL
            .iter()
            .map(|a| a.as_str().to_string())
            .collect(),
        PickerField::Theme => Theme::ALL.iter().map(|t| t.name().to_string()).collect(),
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
                        pending.flags.action = a;
                    }
                }
                PickerField::Theme => {
                    if let Some(t) = Theme::from_str(&selected) {
                        pending.theme = t;
                    }
                }
            }
        }
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use andre_core::{Config, StowAction, StowFlags, Theme};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::components::settings::pending::PendingSettings;

    fn component_with_pending() -> SettingsComponent {
        SettingsComponent {
            pending: Some(PendingSettings {
                config: Config::default_empty(),
                flags: StowFlags::default(),
                theme: Theme::Default,
            }),
            ..Default::default()
        }
    }

    fn enter() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }

    #[test]
    fn test_theme_picker_lists_every_theme_by_name() {
        let mut component = component_with_pending();
        open_picker(&mut component, "Theme", PickerField::Theme, "default");
        let picker = component.picker.as_ref().unwrap();
        let expected: Vec<&str> = Theme::ALL.iter().map(|t| t.name()).collect();
        let options: Vec<&str> = picker.options.iter().map(String::as_str).collect();
        assert_eq!(options, expected);
    }

    #[test]
    fn test_theme_picker_preselects_current_theme() {
        for theme in Theme::ALL {
            let mut component = component_with_pending();
            open_picker(&mut component, "Theme", PickerField::Theme, theme.name());
            let picker = component.picker.as_ref().unwrap();
            assert_eq!(
                picker.options[picker.selected_index],
                theme.name(),
                "picker did not preselect {}",
                theme.name()
            );
        }
    }

    #[test]
    fn test_theme_picker_enter_applies_every_theme() {
        for theme in Theme::ALL {
            let mut component = component_with_pending();
            open_picker(&mut component, "Theme", PickerField::Theme, "default");
            let picker = component.picker.as_mut().unwrap();
            picker.selected_index = Theme::ALL.iter().position(|t| *t == theme).unwrap();

            assert!(handle_picker_key(&mut component, &enter()));

            let pending = component.pending.as_ref().unwrap();
            assert_eq!(
                pending.theme,
                theme,
                "selecting {} was ignored",
                theme.name()
            );
            assert!(component.config_dirty);
        }
    }

    #[test]
    fn test_action_picker_round_trips_every_action() {
        for action in StowAction::ALL {
            let mut component = component_with_pending();
            open_picker(
                &mut component,
                "Action",
                PickerField::Action,
                action.as_str(),
            );
            let picker = component.picker.as_ref().unwrap();
            assert_eq!(picker.options[picker.selected_index], action.as_str());

            assert!(handle_picker_key(&mut component, &enter()));
            assert_eq!(component.pending.as_ref().unwrap().flags.action, action);
        }
    }
}
