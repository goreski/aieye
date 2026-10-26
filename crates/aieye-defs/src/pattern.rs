//! Path patterns in definitions.
//!
//! Patterns use `/` as the separator on every platform and must start with one of:
//!
//! - `~/`, the user's home folder;
//! - `%NAME%`, the value of an environment variable such as `%LOCALAPPDATA%`;
//! - an absolute path, `/…` or a drive such as `C:/…`.
//!
//! The rest may use the wildcards `*` and `?`, which stay within one folder or file name.

use std::path::{MAIN_SEPARATOR_STR, Path};

use glob::Pattern;
use thiserror::Error;

/// Why a pattern cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PatternError {
    #[error("`{0}` must start with `~/`, `%NAME%` or an absolute path")]
    NotAnchored(String),
    #[error("`{0}` must use `/` as the separator and must not contain `..`")]
    BadPath(String),
    #[error("`{0}` has an unclosed `%`")]
    UnclosedVariable(String),
    #[error("`{pattern}` is not a valid wildcard pattern: {reason}")]
    BadWildcard { pattern: String, reason: String },
    #[error("the home folder is unknown")]
    NoHome,
    #[error("environment variable `{0}` is not set")]
    MissingVariable(String),
    #[error("environment variable `{0}` is not an absolute path")]
    RelativeVariable(String),
}

enum Anchor<'a> {
    Home,
    Variable(&'a str),
    Absolute,
}

/// Splits a pattern into its anchor and the rest, which is empty or starts with `/`.
fn split(pattern: &str) -> Result<(Anchor<'_>, &str), PatternError> {
    let not_anchored = || PatternError::NotAnchored(pattern.to_owned());
    if let Some(rest) = pattern.strip_prefix('~') {
        return if rest.is_empty() || rest.starts_with('/') {
            Ok((Anchor::Home, rest))
        } else {
            Err(not_anchored())
        };
    }
    if let Some(after) = pattern.strip_prefix('%') {
        let end = after
            .find('%')
            .ok_or_else(|| PatternError::UnclosedVariable(pattern.to_owned()))?;
        let (name, rest) = (&after[..end], &after[end + 1..]);
        let valid_name = !name.is_empty() && !name.contains('/');
        return if valid_name && (rest.is_empty() || rest.starts_with('/')) {
            Ok((Anchor::Variable(name), rest))
        } else {
            Err(not_anchored())
        };
    }
    let bytes = pattern.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && &bytes[1..3] == b":/";
    if pattern.starts_with('/') || drive {
        Ok((Anchor::Absolute, pattern))
    } else {
        Err(not_anchored())
    }
}

/// Checks a pattern's syntax without expanding it.
pub(crate) fn check(pattern: &str) -> Result<(), PatternError> {
    let (_, rest) = split(pattern)?;
    if pattern.contains('\\') || rest.split('/').any(|part| part == "..") {
        return Err(PatternError::BadPath(pattern.to_owned()));
    }
    Pattern::new(rest).map_err(|error| PatternError::BadWildcard {
        pattern: pattern.to_owned(),
        reason: error.msg.to_owned(),
    })?;
    Ok(())
}

/// Expands a pattern into a glob pattern for this machine.
///
/// The home folder and variable values are escaped, so wildcard characters in them,
/// for example in a user name, match only themselves.
pub fn expand(
    pattern: &str,
    home: Option<&Path>,
    var: impl Fn(&str) -> Option<String>,
) -> Result<String, PatternError> {
    check(pattern)?;
    let (anchor, rest) = split(pattern)?;
    let base = match anchor {
        Anchor::Home => home
            .ok_or(PatternError::NoHome)?
            .to_string_lossy()
            .into_owned(),
        Anchor::Variable(name) => {
            let value = var(name).ok_or_else(|| PatternError::MissingVariable(name.to_owned()))?;
            if !Path::new(&value).is_absolute() {
                return Err(PatternError::RelativeVariable(name.to_owned()));
            }
            value
        }
        Anchor::Absolute => String::new(),
    };
    let base = Pattern::escape(base.trim_end_matches(['/', '\\']));
    Ok(format!("{base}{}", rest.replace('/', MAIN_SEPARATOR_STR)))
}

#[cfg(test)]
mod tests {
    use std::path::MAIN_SEPARATOR as SEP;

    use super::*;

    const BASE: &str = if cfg!(windows) { r"C:\base" } else { "/base" };

    fn no_vars(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn accepts_anchored_patterns() {
        for pattern in [
            "~",
            "~/.codex",
            "~/releases/*/bin/codex.exe",
            "%APPDATA%",
            "%LOCALAPPDATA%/Programs/x.exe",
            "C:/Program Files/x",
            "/usr/bin/x",
        ] {
            assert_eq!(check(pattern), Ok(()), "{pattern}");
        }
    }

    #[test]
    fn rejects_unanchored_or_unsafe_patterns() {
        for pattern in [
            "relative/x",
            "~user/x",
            "%APPDATA%x",
            "%%/x",
            "%UNCLOSED/x",
            r"~\.codex",
            "~/../x",
            "~/a/[b",
        ] {
            assert!(check(pattern).is_err(), "{pattern}");
        }
    }

    #[test]
    fn expands_the_home_folder() {
        let expanded = expand("~/.codex/config.toml", Some(Path::new(BASE)), no_vars).unwrap();
        assert_eq!(expanded, format!("{BASE}{SEP}.codex{SEP}config.toml"));
    }

    #[test]
    fn expands_variables() {
        let var = |name: &str| (name == "LOCALAPPDATA").then(|| format!("{BASE}{SEP}"));
        let expanded = expand("%LOCALAPPDATA%/x.exe", None, var).unwrap();
        assert_eq!(expanded, format!("{BASE}{SEP}x.exe"));
    }

    #[test]
    fn reports_missing_or_relative_values() {
        assert_eq!(expand("~/x", None, no_vars), Err(PatternError::NoHome));
        assert_eq!(
            expand("%NOPE%/x", None, no_vars),
            Err(PatternError::MissingVariable("NOPE".into()))
        );
        assert_eq!(
            expand("%REL%/x", None, |_: &str| Some("relative".into())),
            Err(PatternError::RelativeVariable("REL".into()))
        );
    }

    #[test]
    fn wildcards_in_the_home_folder_match_only_themselves() {
        let home = format!("{BASE}{SEP}[me]*");
        let expanded = expand("~/x", Some(Path::new(&home)), no_vars).unwrap();
        let pattern = Pattern::new(&expanded).unwrap();
        assert!(pattern.matches(&format!("{home}{SEP}x")));
        assert!(!pattern.matches(&format!("{BASE}{SEP}m{SEP}x")));
    }
}
