use andre_core::{AppState, StowAction, Theme};

pub struct PendingSettings {
    pub config: andre_core::Config,
    pub verbosity: u8,
    pub dry_run: bool,
    pub no_folding: bool,
    pub adopt: bool,
    pub dotfiles: bool,
    pub action: StowAction,
    pub theme: Theme,
}

impl PendingSettings {
    pub fn from_core(core: &AppState) -> Self {
        Self {
            config: core.config.clone(),
            verbosity: core.verbosity,
            dry_run: core.dry_run,
            no_folding: core.no_folding,
            adopt: core.adopt,
            dotfiles: core.dotfiles,
            action: core.action,
            theme: core.theme,
        }
    }

    pub fn apply_to(&self, core: &mut AppState) {
        core.config = self.config.clone();
        core.verbosity = self.verbosity;
        core.dry_run = self.dry_run;
        core.no_folding = self.no_folding;
        core.adopt = self.adopt;
        core.dotfiles = self.dotfiles;
        core.action = self.action;
        core.theme = self.theme;
        core.config.set_verbosity(self.verbosity);
        core.config.set_dry_run(self.dry_run);
        core.config.set_no_folding(self.no_folding);
        core.config.set_adopt(self.adopt);
        core.config.set_dotfiles(self.dotfiles);
        core.config.set_action(self.action.as_str());
    }
}
