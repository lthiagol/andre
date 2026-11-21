use crate::components::settings::{ConfigMessage, SettingsComponent};
use crate::components::AppContext;

/// Persist pending settings. Returns `true` on successful write.
/// On failure: does not mutate `ctx.core`, keeps dirty, shows error toast.
pub fn save_config(component: &mut SettingsComponent, ctx: &mut AppContext) -> bool {
    let pending = component
        .pending
        .as_ref()
        .expect("pending settings are initialised before any save");

    // Write first; core only changes once the file is on disk.
    match pending.to_config().save(&ctx.config_path) {
        Ok(()) => {
            pending.apply_to(&mut ctx.core);
            component.config_dirty = false;
            component.config_message = ConfigMessage::Saved;
            ctx.show_toast("Config saved");
            true
        }
        Err(e) => {
            let msg = crate::ui::elide(&format!("Save failed: {e}"), 60);
            ctx.show_error_toast(msg);
            false
        }
    }
}
