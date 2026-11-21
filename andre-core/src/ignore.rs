//! Ignore lists: which files inside a package are never linked.
//!
//! Follows GNU Stow 2.4 (`Stow.pm`: `get_ignore_regexps`, `get_ignore_regexps_from_fh`,
//! `compile_ignore_regexps`, `ignore`; manual section "Ignore Lists"):
//!
//! - One Perl-style regex per line. Blank lines and lines starting with `#` are
//!   skipped, ` #comment` after a pattern is dropped, `\#` means a literal `#`.
//! - The first list found is used whole, nothing is merged: the package's
//!   `.stow-local-ignore`, then `$HOME/.stow-global-ignore`, then the built-in list.
//! - A pattern containing `/` is matched against `/<path relative to the package>`
//!   as a sub-path. A pattern without `/` must equal the basename.
//! - A directory that matches is not entered, so nothing under it is linked.
//! - A `.stow-local-ignore` at the package root is never linked when a list is read
//!   from a file.
//!
//! Patterns use Rust `regex` syntax, which covers what ignore lists use in practice.
//! Look-around and backreferences are rejected; use `global.engine: stow` for them.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use regex::Regex;

use crate::error::{Error, Result};

pub const LOCAL_IGNORE_FILE: &str = ".stow-local-ignore";
pub const GLOBAL_IGNORE_FILE: &str = ".stow-global-ignore";
pub const GITIGNORE_FILE: &str = ".gitignore";

/// GNU Stow 2.4.1 built-in list: the `__DATA__` block of `Stow.pm`, verbatim.
pub const DEFAULT_IGNORE_TEXT: &str = r"# Comments and blank lines are allowed.

RCS
.+,v

CVS
\.\#.+       # CVS conflict files / emacs lock files
\.cvsignore

\.svn
_darcs
\.hg

\.git
\.gitignore
\.gitmodules

.+~          # emacs backup files
\#.*\#       # emacs autosave files

^/README.*
^/LICENSE.*
^/COPYING
";

/// `.gitignore` offered when a package is created: keeps secrets and caches out of git.
/// Gitignore globs, not stow regexes. Reviewed list; `secrets/` and `*history*` are
/// deliberately broad.
pub const GITIGNORE_SECRETS_TEXT: &str = "\
# Written by andre: secrets, caches and OS junk stay out of git.
.env
.env.*
*.env
*.key
*.pem
*history*
*.swp
secrets/
.cache/
__pycache__/
.DS_Store
";

/// `.stow-local-ignore` offered when a package is created. Its presence replaces
/// stow's built-in list, so the built-in list is copied in to keep the defaults.
pub fn stow_local_ignore_template() -> String {
    format!(
        "# Written by andre. This file replaces stow's built-in ignore list,\n\
         # so the built-in list is copied below. Edit freely.\n\n{DEFAULT_IGNORE_TEXT}"
    )
}

/// Where an [`IgnoreRules`] set came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSource {
    /// The package's own `.stow-local-ignore`.
    Local,
    /// `$HOME/.stow-global-ignore`.
    Global,
    /// The built-in default list.
    Builtin,
}

/// Compiled ignore rules for one package.
#[derive(Debug, Clone)]
pub struct IgnoreRules {
    source: RuleSource,
    /// Patterns containing `/`, compiled as `(^|/)(...)(/|$)` against `/<rel>`.
    path: Option<Regex>,
    /// Patterns without `/`, compiled as `^(...)$` against the basename.
    segment: Option<Regex>,
}

impl IgnoreRules {
    /// The rules that apply to `package_dir`: its own `.stow-local-ignore`, else
    /// `global` when that file exists, else the built-in list.
    pub fn load(package_dir: &Path, global: Option<&Path>) -> Result<Self> {
        let local = package_dir.join(LOCAL_IGNORE_FILE);
        if local.exists() {
            return Self::from_file(&local, RuleSource::Local);
        }
        if let Some(global) = global.filter(|g| g.exists()) {
            return Self::from_file(global, RuleSource::Global);
        }
        Self::from_text(
            DEFAULT_IGNORE_TEXT,
            RuleSource::Builtin,
            "the built-in ignore list",
        )
    }

    /// Reads and compiles one ignore file.
    pub fn from_file(path: &Path, source: RuleSource) -> Result<Self> {
        let text = fs::read_to_string(path)?;
        Self::from_text(&text, source, &path.display().to_string())
    }

    /// Compiles ignore-list text. `origin` names the list in error messages.
    pub fn from_text(text: &str, source: RuleSource, origin: &str) -> Result<Self> {
        let mut with_slash = Vec::new();
        let mut without_slash = Vec::new();
        for (number, pattern) in parse_patterns(text) {
            Regex::new(&format!("(?:{pattern})")).map_err(|e| {
                Error::Ignore(format!(
                    "invalid pattern '{pattern}' on line {number} of {origin}: {e}"
                ))
            })?;
            if pattern.contains('/') {
                with_slash.push(pattern);
            } else {
                without_slash.push(pattern);
            }
        }
        if source != RuleSource::Builtin {
            // Stow never links its own local list (Stow.pm adds this line for every list).
            with_slash.push(r"^/\.stow-local-ignore$".to_string());
        }
        Ok(Self {
            source,
            path: compile(&with_slash, |alt| format!("(^|/)({alt})(/|$)"))?,
            segment: compile(&without_slash, |alt| format!("^({alt})$"))?,
        })
    }

    pub fn source(&self) -> RuleSource {
        self.source
    }

    /// True when `rel` (slash-separated, relative to the package root) is ignored.
    /// Does not look at ancestors; use [`IgnoreRules::excludes`] for that.
    pub fn is_ignored(&self, rel: &str) -> bool {
        let basename = rel.rsplit('/').next().unwrap_or(rel);
        self.path
            .as_ref()
            .is_some_and(|re| re.is_match(&format!("/{rel}")))
            || self
                .segment
                .as_ref()
                .is_some_and(|re| re.is_match(basename))
    }

    /// True when `rel` itself or any directory above it is ignored, so the walk never
    /// reaches it.
    pub fn excludes(&self, rel: &Path) -> bool {
        let parts: Vec<String> = rel
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();
        (1..=parts.len()).any(|n| self.is_ignored(&parts[..n].join("/")))
    }
}

/// `home/.stow-global-ignore`, the global list for a given home folder. Callers pass
/// the home they resolved (the TUI's `ctx.home_dir`), so tests can point it at a temp dir.
pub fn global_ignore_in(home: &Path) -> PathBuf {
    home.join(GLOBAL_IGNORE_FILE)
}

/// The two per-package ignore files andre reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreFile {
    StowLocal,
    GitIgnore,
}

impl IgnoreFile {
    pub fn file_name(self) -> &'static str {
        match self {
            IgnoreFile::StowLocal => LOCAL_IGNORE_FILE,
            IgnoreFile::GitIgnore => GITIGNORE_FILE,
        }
    }

    pub fn path_in(self, package_dir: &Path) -> PathBuf {
        package_dir.join(self.file_name())
    }

    /// Starter content for a new file.
    pub fn template(self) -> String {
        match self {
            IgnoreFile::StowLocal => stow_local_ignore_template(),
            IgnoreFile::GitIgnore => GITIGNORE_SECRETS_TEXT.to_string(),
        }
    }
}

/// Lines of an ignore file as written, comments and blanks included. `None` when the
/// file does not exist.
pub fn read_lines(path: &Path) -> Result<Option<Vec<String>>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text.lines().map(str::to_string).collect())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Appends `line` to an existing ignore file. Returns `false` when the same line
/// (compared trimmed) is already present. A `.stow-local-ignore` line must compile
/// first; a bad line is refused and the file is left alone.
pub fn add_line(path: &Path, kind: IgnoreFile, line: &str) -> Result<bool> {
    let line = line.trim();
    if line.is_empty() {
        return Err(Error::Ignore("an empty line cannot be added".into()));
    }
    if kind == IgnoreFile::StowLocal {
        IgnoreRules::from_text(line, RuleSource::Local, "the new line")?;
    }
    let mut lines = read_lines(path)?.ok_or_else(|| missing(path))?;
    if lines.iter().any(|l| l.trim() == line) {
        return Ok(false);
    }
    lines.push(line.to_string());
    write_lines(path, &lines)?;
    Ok(true)
}

/// Removes the first line equal (trimmed) to `line`. Returns `false` when none matches
/// or the file does not exist.
pub fn remove_line(path: &Path, line: &str) -> Result<bool> {
    let Some(mut lines) = read_lines(path)? else {
        return Ok(false);
    };
    let target = line.trim();
    let Some(at) = lines.iter().position(|l| l.trim() == target) else {
        return Ok(false);
    };
    lines.remove(at);
    write_lines(path, &lines)?;
    Ok(true)
}

/// Writes `lines` as the whole ignore file at `path`, creating it when missing. A
/// `.stow-local-ignore` whose patterns do not compile is refused and the file is left alone.
pub fn save_lines(path: &Path, kind: IgnoreFile, lines: &[String]) -> Result<()> {
    if kind == IgnoreFile::StowLocal {
        let origin = path.display().to_string();
        IgnoreRules::from_text(&lines.join("\n"), RuleSource::Local, &origin)?;
    }
    write_lines(path, lines)
}

/// Creates the ignore file in `package_dir` with `content`. Fails if it exists.
pub fn create_file(package_dir: &Path, kind: IgnoreFile, content: &str) -> Result<PathBuf> {
    use std::io::Write;

    let path = kind.path_in(package_dir);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| match e.kind() {
            io::ErrorKind::AlreadyExists => {
                Error::Ignore(format!("{} already exists", path.display()))
            }
            _ => e.into(),
        })?;
    file.write_all(content.as_bytes())?;
    Ok(path)
}

/// Deletes the ignore file. Returns `false` when it was not there.
pub fn delete_file(package_dir: &Path, kind: IgnoreFile) -> Result<bool> {
    match fs::remove_file(kind.path_in(package_dir)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn missing(path: &Path) -> Error {
    Error::Io(io::Error::new(
        io::ErrorKind::NotFound,
        format!("{} does not exist", path.display()),
    ))
}

fn write_lines(path: &Path, lines: &[String]) -> Result<()> {
    let mut text = lines.join("\n");
    if !lines.is_empty() {
        text.push('\n');
    }
    fs::write(path, text)?;
    Ok(())
}

/// Pattern lines with their 1-based line numbers. Same reading as GNU Stow's
/// `get_ignore_regexps_from_fh`.
fn parse_patterns(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter_map(|(index, raw)| {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let pattern = strip_comment(line).replace(r"\#", "#");
            Some((index + 1, pattern))
        })
        .collect()
}

/// Drops a trailing ` #comment`: whitespace, `#`, then at least one more character.
fn strip_comment(line: &str) -> &str {
    for (at, _) in line.match_indices('#') {
        let head = &line[..at];
        let kept = head.trim_end();
        if kept.len() < head.len() && at + 1 < line.len() {
            return kept;
        }
    }
    line
}

/// One regex for all patterns of one kind. Each pattern is wrapped on its own, so a
/// pattern cannot change how the others parse.
fn compile(patterns: &[String], wrap: impl Fn(&str) -> String) -> Result<Option<Regex>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let alternation = patterns
        .iter()
        .map(|p| format!("(?:{p})"))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&wrap(&alternation))
        .map(Some)
        .map_err(|e| Error::Ignore(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn rules(text: &str) -> IgnoreRules {
        IgnoreRules::from_text(text, RuleSource::Local, "test").unwrap()
    }

    #[test]
    fn parse_skips_comments_blanks_and_strips_trailing_comments() {
        let parsed =
            parse_patterns("# header\n\n  .DS_Store  \nfoo # trailing\n\\#literal\nbar #\n");
        let patterns: Vec<&str> = parsed.iter().map(|(_, p)| p.as_str()).collect();
        // `bar #` keeps its `#`: nothing follows it, so it is not a comment (GNU Stow).
        assert_eq!(patterns, vec![".DS_Store", "foo", "#literal", "bar #"]);
    }

    #[test]
    fn parse_keeps_line_numbers() {
        let parsed = parse_patterns("# c\n\nfoo\n");
        assert_eq!(parsed, vec![(3, "foo".to_string())]);
    }

    #[test]
    fn builtin_list_compiles_and_matches_gnu_defaults() {
        let r = IgnoreRules::load(Path::new("/nonexistent-pkg"), None).unwrap();
        assert_eq!(r.source(), RuleSource::Builtin);
        for rel in [
            "README.md",
            "LICENSE",
            "COPYING",
            "notes.txt~",
            "#draft#",
            "foo,v",
            "CVS",
            "sub/.git",
            "sub/.gitignore",
            "sub/.cvsignore",
            ".hg",
        ] {
            assert!(r.excludes(Path::new(rel)), "{rel} should be ignored");
        }
        // Top-level-only anchors: nested README is linked.
        assert!(!r.excludes(Path::new("docs/README.md")));
        assert!(!r.excludes(Path::new(".bashrc")));
    }

    #[test]
    fn gnu_manual_example_matches_as_documented() {
        // Stow manual, "Types And Syntax Of Ignore Lists": file bazqux in foo/bar.
        let file = "foo/bar/bazqux";
        for matching in ["bazqux", "baz.*", ".*qux", "bar/.*x", "^/foo/.*qux"] {
            assert!(rules(matching).is_ignored(file), "{matching} should match");
        }
        for not_matching in ["bar", "baz", "qux", "o/bar/b"] {
            assert!(
                !rules(not_matching).is_ignored(file),
                "{not_matching} should not"
            );
        }
    }

    #[test]
    fn directory_pattern_excludes_everything_below_it() {
        let r = rules("bar\n");
        assert!(r.excludes(Path::new("foo/bar/bazqux")));
        assert!(!r.is_ignored("foo/bar/bazqux"));
    }

    #[test]
    fn slash_pattern_matches_any_subpath_unless_caret_anchored() {
        assert!(rules("^/foo/.*").is_ignored("foo/x"));
        assert!(!rules("^/foo/.*").is_ignored("sub/foo/x"));
        assert!(rules("sub/foo").is_ignored("sub/foo"));
        assert!(rules("sub/foo").is_ignored("other/sub/foo/x"));
    }

    #[test]
    fn local_list_replaces_builtin_defaults() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(LOCAL_IGNORE_FILE), "secret\n").unwrap();
        let r = IgnoreRules::load(tmp.path(), None).unwrap();
        assert_eq!(r.source(), RuleSource::Local);
        assert!(r.is_ignored("secret"));
        assert!(
            !r.is_ignored("README.md"),
            "builtin defaults must not apply"
        );
        assert!(!r.is_ignored(".git"), "builtin defaults must not apply");
        assert!(r.excludes(Path::new(LOCAL_IGNORE_FILE)));
    }

    #[test]
    fn nested_local_ignore_file_is_linked_like_gnu() {
        let r = rules("foo\n");
        assert!(!r.excludes(Path::new("sub/.stow-local-ignore")));
    }

    #[test]
    fn global_list_used_when_package_has_none() {
        let tmp = TempDir::new().unwrap();
        let global = tmp.path().join(GLOBAL_IGNORE_FILE);
        fs::write(&global, "# global\nhidden\n").unwrap();
        let r = IgnoreRules::load(tmp.path(), Some(&global)).unwrap();
        assert_eq!(r.source(), RuleSource::Global);
        assert!(r.is_ignored("hidden"));
        assert!(!r.is_ignored("README.md"));
    }

    #[test]
    fn local_list_wins_over_global() {
        let tmp = TempDir::new().unwrap();
        let global = tmp.path().join(GLOBAL_IGNORE_FILE);
        fs::write(&global, "hidden\n").unwrap();
        let pkg = tmp.path().join("pkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join(LOCAL_IGNORE_FILE), "other\n").unwrap();
        let r = IgnoreRules::load(&pkg, Some(&global)).unwrap();
        assert_eq!(r.source(), RuleSource::Local);
        assert!(!r.is_ignored("hidden"));
    }

    #[test]
    fn invalid_pattern_names_line_and_pattern() {
        let err = IgnoreRules::from_text("ok\nfoo(\n", RuleSource::Local, "x").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 2"), "{msg}");
        assert!(msg.contains("foo("), "{msg}");
    }

    #[test]
    fn look_around_is_rejected_not_silently_wrong() {
        assert!(IgnoreRules::from_text("foo(?=bar)\n", RuleSource::Local, "x").is_err());
    }

    #[test]
    fn stow_local_template_keeps_gnu_defaults_and_gitignore() {
        let r = rules(&stow_local_ignore_template());
        assert!(r.is_ignored("README.md"));
        assert!(
            r.is_ignored(".gitignore"),
            "template must keep \\.gitignore ignored"
        );
        assert!(!r.is_ignored("bashrc"));
    }

    #[test]
    fn gitignore_template_is_written_for_secrets_and_caches() {
        let text = IgnoreFile::GitIgnore.template();
        for needle in [".env", "*.key", "*.pem", "*history*", "secrets/", ".cache/"] {
            assert!(text.lines().any(|l| l == needle), "missing {needle}");
        }
    }

    #[test]
    fn add_line_appends_once_and_refuses_bad_regex() {
        let tmp = TempDir::new().unwrap();
        let path = IgnoreFile::StowLocal.path_in(tmp.path());
        fs::write(&path, "# mine\nfoo\n").unwrap();

        assert!(add_line(&path, IgnoreFile::StowLocal, "  bar  ").unwrap());
        assert!(!add_line(&path, IgnoreFile::StowLocal, "bar").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "# mine\nfoo\nbar\n");

        assert!(add_line(&path, IgnoreFile::StowLocal, "bad(").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "# mine\nfoo\nbar\n");
    }

    #[test]
    fn add_line_refuses_empty_and_missing_file() {
        let tmp = TempDir::new().unwrap();
        let path = IgnoreFile::GitIgnore.path_in(tmp.path());
        assert!(add_line(&path, IgnoreFile::GitIgnore, "  ").is_err());
        assert!(add_line(&path, IgnoreFile::GitIgnore, "x").is_err());
        assert!(!path.exists());
    }

    #[test]
    fn remove_line_drops_first_match_and_keeps_rest() {
        let tmp = TempDir::new().unwrap();
        let path = IgnoreFile::GitIgnore.path_in(tmp.path());
        fs::write(&path, "a\nb\na\n").unwrap();
        assert!(remove_line(&path, "a").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "b\na\n");
        assert!(!remove_line(&path, "zzz").unwrap());
        assert!(!remove_line(&tmp.path().join("nope"), "a").unwrap());
    }

    #[test]
    fn save_lines_writes_whole_file_and_refuses_bad_stow_pattern() {
        let tmp = TempDir::new().unwrap();
        let path = IgnoreFile::StowLocal.path_in(tmp.path());
        let lines = vec!["# mine".to_string(), "foo".to_string()];
        save_lines(&path, IgnoreFile::StowLocal, &lines).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "# mine\nfoo\n");

        let bad = vec!["foo".to_string(), "bad(".to_string()];
        let err = save_lines(&path, IgnoreFile::StowLocal, &bad).unwrap_err();
        assert!(err.to_string().contains("bad("), "{err}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "# mine\nfoo\n");
    }

    #[test]
    fn save_lines_takes_any_gitignore_line_and_empty_list_is_empty_file() {
        let tmp = TempDir::new().unwrap();
        let path = IgnoreFile::GitIgnore.path_in(tmp.path());
        save_lines(&path, IgnoreFile::GitIgnore, &["bad(".to_string()]).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "bad(\n");
        save_lines(&path, IgnoreFile::GitIgnore, &[]).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn read_lines_none_when_absent() {
        let tmp = TempDir::new().unwrap();
        let path = IgnoreFile::StowLocal.path_in(tmp.path());
        assert_eq!(read_lines(&path).unwrap(), None);
        fs::write(&path, "x\n# y\n").unwrap();
        assert_eq!(
            read_lines(&path).unwrap(),
            Some(vec!["x".to_string(), "# y".to_string()])
        );
    }

    #[test]
    fn create_file_never_overwrites_and_delete_reports_absence() {
        let tmp = TempDir::new().unwrap();
        let path = create_file(tmp.path(), IgnoreFile::GitIgnore, "keep\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep\n");

        let err = create_file(tmp.path(), IgnoreFile::GitIgnore, "other").unwrap_err();
        assert!(err.to_string().contains("already exists"), "{err}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep\n");

        assert!(delete_file(tmp.path(), IgnoreFile::GitIgnore).unwrap());
        assert!(!delete_file(tmp.path(), IgnoreFile::GitIgnore).unwrap());
    }
}
