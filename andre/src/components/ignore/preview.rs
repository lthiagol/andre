//! What the ignore editor says about a line: the package files it matches. A stow line is
//! matched with the engine's own rules. A `.gitignore` line is matched with a simple glob,
//! which is a preview only: git decides for itself. The glob supports `*`, `?`, a leading
//! or inner `/` (anchors the pattern to the package root) and a trailing `/` (directories
//! only). It does not support `[...]` classes, `**` or `!` negation.

use std::path::Path;

use andre_core::ignore::{IgnoreRules, RuleSource};
use andre_core::stow::collect_package_files;

/// Every file in the package, relative to its root and slash-separated, in name order.
/// The package's own `.stow-local-ignore` is left out: it is never linked.
pub fn package_files(dir: &Path) -> Vec<String> {
    let Ok(nothing) = IgnoreRules::from_text("", RuleSource::Local, "the package") else {
        return Vec::new();
    };
    collect_package_files(dir, &nothing)
        .iter()
        .filter_map(|file| file.strip_prefix(dir).ok())
        .map(|rel| {
            rel.to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
        })
        .collect()
}

/// The files that the stow line `pattern` excludes, itself or through a directory above
/// them. `Err` with the reason when the line does not compile.
pub fn stow_matches(pattern: &str, files: &[String]) -> Result<Vec<String>, String> {
    let rules = IgnoreRules::from_text(pattern, RuleSource::Local, "this line")
        .map_err(|err| short_reason(&err.to_string()))?;
    Ok(files
        .iter()
        .filter(|file| rules.excludes(Path::new(file)))
        .cloned()
        .collect())
}

/// The last line of a compile error without the regex crate's `error: ` prefix, so `bad(`
/// gives `unclosed group`. The core's full message runs over several lines and names the
/// internal wrapper, which the detail cannot show.
fn short_reason(message: &str) -> String {
    let last = message
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    let reason = last.trim().trim_start_matches("error:").trim();
    if reason.is_empty() {
        "not a valid pattern".to_string()
    } else {
        reason.to_string()
    }
}

/// The files that the `.gitignore` line `pattern` matches, by the glob preview.
pub fn gitignore_matches(pattern: &str, files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|file| glob_excludes(pattern, file))
        .cloned()
        .collect()
}

/// True when the glob `pattern` excludes `rel`, itself or through a directory above it.
/// Blank lines, comments and `!` negations exclude nothing.
pub fn glob_excludes(pattern: &str, rel: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern.starts_with('#') || pattern.starts_with('!') {
        return false;
    }
    let dir_only = pattern.ends_with('/');
    let pattern = pattern.trim_end_matches('/');
    let anchored = pattern.trim_start_matches('/').contains('/') || pattern.starts_with('/');
    let pattern = pattern.trim_start_matches('/');
    if pattern.is_empty() {
        return false;
    }

    let parts: Vec<&str> = rel.split('/').collect();
    // A directory above the file excludes it too, so only a directory can end a dir-only match
    // early: the last part is the file itself.
    let may_end = |index: usize| !dir_only || index + 1 < parts.len();
    if anchored {
        let pieces: Vec<&str> = pattern.split('/').collect();
        pieces.len() <= parts.len()
            && may_end(pieces.len() - 1)
            && pieces
                .iter()
                .zip(&parts)
                .all(|(piece, part)| glob_component(piece, part))
    } else {
        parts
            .iter()
            .enumerate()
            .any(|(index, part)| may_end(index) && glob_component(pattern, part))
    }
}

/// Matches one path component against a glob with `*` and `?`.
fn glob_component(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // The last `*` seen, and the text position it has absorbed up to. A later mismatch
    // retries from there with one more character absorbed.
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if let Some((star_at, absorbed)) = star {
            p = star_at + 1;
            t = absorbed + 1;
            star = Some((star_at, absorbed + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn glob_component_handles_star_and_question() {
        assert!(glob_component("*.env", ".env"));
        assert!(glob_component("*.env", "work.env"));
        assert!(!glob_component("*.env", "env.txt"));
        assert!(glob_component("*history*", ".zsh_history"));
        assert!(glob_component("a?c", "abc"));
        assert!(!glob_component("a?c", "ac"));
        assert!(glob_component("*", ""));
        assert!(!glob_component("abc", "abcd"));
    }

    #[test]
    fn unanchored_pattern_matches_a_name_at_any_depth() {
        assert!(glob_excludes("*.env", "work.env"));
        assert!(glob_excludes("*.env", ".config/app/prod.env"));
        assert!(!glob_excludes("*.env", "env.txt"));
    }

    #[test]
    fn trailing_slash_matches_directories_and_what_is_inside_them() {
        assert!(glob_excludes("secrets/", "secrets/token"));
        assert!(glob_excludes("secrets/", "home/secrets/token"));
        assert!(!glob_excludes("secrets/", "secrets"));
    }

    #[test]
    fn leading_or_inner_slash_anchors_to_the_package_root() {
        assert!(glob_excludes("/README*", "README.md"));
        assert!(!glob_excludes("/README*", "docs/README.md"));
        assert!(glob_excludes("docs/*.md", "docs/a.md"));
        assert!(!glob_excludes("docs/*.md", "x/docs/a.md"));
    }

    #[test]
    fn blank_comment_and_negation_match_nothing() {
        assert!(!glob_excludes("", "a"));
        assert!(!glob_excludes("   ", "a"));
        assert!(!glob_excludes("# note", "note"));
        assert!(!glob_excludes("!keep.env", "keep.env"));
        assert!(!glob_excludes("/", "a"));
    }

    #[test]
    fn gitignore_matches_lists_the_files_a_line_hits() {
        let names = files(&["work.env", ".config/app/prod.env", "notes.txt"]);
        assert_eq!(
            gitignore_matches("*.env", &names),
            files(&["work.env", ".config/app/prod.env"])
        );
    }

    #[test]
    fn stow_matches_uses_the_engine_rules() {
        let names = files(&["README.md", "docs/README.md", ".git/config", "bashrc"]);
        assert_eq!(
            stow_matches("^/README.*", &names).unwrap(),
            files(&["README.md"])
        );
        assert_eq!(
            stow_matches("\\.git", &names).unwrap(),
            files(&[".git/config"])
        );
        assert!(stow_matches("# only a comment", &names).unwrap().is_empty());
    }

    #[test]
    fn stow_matches_reports_a_line_that_does_not_compile() {
        let err = stow_matches("bad(", &files(&["a"])).unwrap_err();
        assert_eq!(err, "unclosed group");
    }

    #[test]
    fn stow_matches_reason_is_one_line_without_the_wrapper() {
        let err = stow_matches("bad(", &files(&["a"])).unwrap_err();
        assert!(!err.contains('\n'), "{err:?}");
        assert!(!err.contains("(?:"), "{err}");
        assert!(!err.contains("this line"), "{err}");
    }

    #[test]
    fn package_files_lists_relative_files_and_skips_the_local_ignore_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("top.txt"), "").unwrap();
        std::fs::write(dir.join("sub/inner"), "").unwrap();
        std::fs::write(dir.join(".stow-local-ignore"), "").unwrap();
        assert_eq!(package_files(dir), files(&["sub/inner", "top.txt"]));
    }

    #[test]
    fn package_files_of_a_missing_folder_is_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(package_files(&tmp.path().join("nope")).is_empty());
    }
}
