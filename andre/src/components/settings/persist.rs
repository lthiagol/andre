use crate::components::settings::{ConfigMessage, SettingsComponent};
use crate::components::AppContext;

/// Persist pending settings. Returns `true` on successful write.
/// On failure: does not mutate `ctx.core`, keeps dirty, shows error toast.
pub fn save_config(component: &mut SettingsComponent, ctx: &mut AppContext) -> bool {
    let Some(ref pending) = component.pending else {
        return persist_core_config(component, ctx);
    };

    // Build the on-disk config without committing core until write succeeds.
    let mut to_save = pending.config.clone();
    to_save.set_verbosity(pending.verbosity);
    to_save.set_dry_run(pending.dry_run);
    to_save.set_no_folding(pending.no_folding);
    to_save.set_adopt(pending.adopt);
    to_save.set_dotfiles(pending.dotfiles);
    to_save.set_action(pending.action.as_str());

    match to_save.save(&ctx.config_path) {
        Ok(()) => {
            pending.apply_to(&mut ctx.core);
            component.config_dirty = false;
            component.config_message = ConfigMessage::Saved;
            ctx.show_toast("Config saved");
            true
        }
        Err(e) => {
            eprintln!("Failed to save config: {}", e);
            let msg = crate::ui::elide(&format!("Save failed: {e}"), 60);
            ctx.show_error_toast(msg);
            false
        }
    }
}

fn persist_core_config(component: &mut SettingsComponent, ctx: &mut AppContext) -> bool {
    match ctx.core.config.save(&ctx.config_path) {
        Ok(()) => {
            component.config_dirty = false;
            component.config_message = ConfigMessage::Saved;
            ctx.show_toast("Config saved");
            true
        }
        Err(e) => {
            eprintln!("Failed to save config: {}", e);
            let msg = crate::ui::elide(&format!("Save failed: {e}"), 60);
            ctx.show_error_toast(msg);
            false
        }
    }
}
