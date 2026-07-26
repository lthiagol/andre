use std::fmt;
use std::path::PathBuf;

/// Core error type for andre.
///
/// Hand-written `Display`/`Error`/`From` impls replace `thiserror` (M06 DIY-3).
/// The `From` impls preserve `?` ergonomics for YAML parse and IO errors.
#[derive(Debug)]
pub enum Error {
    ConfigNotFound(PathBuf),
    YamlParse(serde_yaml_ng::Error),
    StowFailed(String),
    SourceNotFound(PathBuf),
    StowNotFound,
    Io(std::io::Error),
    Subprocess(i32),
    /// Native engine v1 does not implement a flag (adopt/dotfiles/regex-ignore);
    /// message recommends `global.engine: stow`.
    NativeUnsupported(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ConfigNotFound(p) => write!(f, "Config file not found: {}", p.display()),
            Error::YamlParse(e) => write!(f, "Failed to parse YAML: {}", e),
            Error::StowFailed(s) => write!(f, "Stow failed: {}", s),
            Error::SourceNotFound(p) => write!(f, "Source directory not found: {}", p.display()),
            Error::StowNotFound => write!(f, "GNU Stow binary not found. Is stow installed?"),
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::Subprocess(code) => write!(f, "Subprocess error: exit code {}", code),
            Error::NativeUnsupported(flag) => write!(
                f,
                "native engine does not support '{}' in v1; set global.engine: stow",
                flag
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::YamlParse(e) => Some(e),
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_yaml_ng::Error> for Error {
    fn from(e: serde_yaml_ng::Error) -> Self {
        Error::YamlParse(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
