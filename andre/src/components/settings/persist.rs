use crate::components::settings::SettingsComponent;
use crate::components::AppContext;
use crate::ui::{elide, plural};

/// Writes the pending settings to `andre.yml`, then has the profiles follow any group
/// renames and removals. Returns `true` on success: the saved settings are the pending ones
/// and the screen is clean. If `andre.yml` cannot be written nothing changes. If the profiles
/// cannot be rewritten the config is saved but the screen stays dirty, so `w` retries.
pub fn save_config(component: &mut SettingsComponent, ctx: &mut AppContext) -> bool {
    let Some(pending) = component.pending.as_mut() else {
        return false;
    };

    // Write first; core only changes once the file is on disk.
    if let Err(e) = pending.to_config().save(&ctx.config_path) {
        let msg = elide(&format!("Save failed: {e}"), 60);
        ctx.show_error_toast(msg);
        return false;
    }
    pending.apply_to(&mut ctx.core);

    let rewritten = match pending.apply_to_profiles(&mut ctx.core.profiles) {
        Ok(count) => count,
        Err(e) => {
            let msg = elide(&format!("Profiles not updated: {e}"), 60);
            ctx.show_error_toast(msg);
            component.dirty = true;
            return false;
        }
    };
    pending.group_changes.clear();
    component.dirty = false;
    let message = match rewritten {
        0 => "Config saved".to_string(),
        n => format!(
            "Config saved \u{b7} {n} {} updated",
            plural(n, "profile", "profiles")
        ),
    };
    ctx.show_toast(message);
    true
}
