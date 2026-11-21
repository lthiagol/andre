use andre_core::config::KEYS_CONFLICT;

use crate::components::settings::SettingsComponent;
use crate::components::AppContext;
use crate::ui::elide;

/// Writes the pending settings to `andre.yml`. Returns `true` on success: the saved settings
/// are the pending ones, the status cache is dropped (the folder or the ignores may have
/// changed what a package's status is), and the screen is clean. If the file cannot be
/// written, or would not load (the checks every launch runs), nothing changes and the screen
/// stays dirty.
pub fn save_config(component: &mut SettingsComponent, ctx: &mut AppContext) -> bool {
    let Some(pending) = component.pending.as_mut() else {
        return false;
    };

    let config = pending.to_config();
    if config.keys_conflict() {
        let msg = elide(&format!("Save failed: {KEYS_CONFLICT}"), 60);
        ctx.show_error_toast(msg);
        return false;
    }
    if let Err(e) = config.validate_sources(&ctx.config_dir, &ctx.home_dir) {
        let msg = elide(&format!("Save failed: {e}"), 60);
        ctx.show_error_toast(msg);
        return false;
    }
    // Write first; core only changes once the file is on disk.
    if let Err(e) = config.save(&ctx.config_path) {
        let msg = elide(&format!("Save failed: {e}"), 60);
        ctx.show_error_toast(msg);
        return false;
    }
    pending.apply_to(&mut ctx.core);
    ctx.status_cache.borrow_mut().invalidate_all();
    component.dirty = false;
    ctx.show_toast("Config saved".to_string());
    true
}
