pub mod adopt;
pub mod config;
pub mod engine;
pub mod error;
pub mod packages;
pub mod path;
pub mod state;
pub mod stow;
pub mod yolo;

pub use adopt::{
    execute_adoption, list_target_dirs, plan_adoption, AdoptionAction, AdoptionPlan, AdoptionStep,
    TargetEntry,
};
pub use config::{Config, ConfigWarning, GlobalSettings, Group, StowConfig};
pub use engine::{engine_for, LinkAction, LinkEngine, LinkOp, NativeEngine, StowEngine};
pub use error::{Error, Result};
pub use packages::{discover_packages, get_package_preview};
pub use path::resolve_path;
pub use state::{AppState, StowAction, Theme};
pub use stow::{
    build_stow_args, build_stow_args_multi, check_stowed_status, execute_stow,
    unlink_stowed_symlink, StowStatus,
};
pub use yolo::{build_plan, run_all, run_job, StowJob, YoloResult};
