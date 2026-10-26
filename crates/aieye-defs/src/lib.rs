//! AI tool definitions: data files that tell AIEye how to recognise each AI tool.
//!
//! The format is described in `definitions/README.md`.

mod pattern;

pub use pattern::{PatternError, expand};

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;
use toml::value::Datetime;

/// An operating system AIEye supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    /// The platform this build runs on, if AIEye supports it.
    pub fn current() -> Option<Self> {
        if cfg!(windows) {
            Some(Self::Windows)
        } else if cfg!(target_os = "macos") {
            Some(Self::Macos)
        } else if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else {
            None
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
        })
    }
}

/// What sort of program an AI tool is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    DesktopApp,
    Cli,
    IdeExtension,
    BrowserExtension,
    McpServer,
}

/// One AI tool definition, read from `<id>.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub kind: Kind,
    /// Platforms the tool exists on.
    pub platforms: Vec<Platform>,
    /// How to recognise the tool, per platform.
    #[serde(default)]
    pub detect: BTreeMap<Platform, Detect>,
    /// Which sections were checked on a real install, per platform.
    #[serde(default)]
    pub verified: Verified,
}

/// How to recognise a tool on one platform. Entries are path patterns (see [`expand`]).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    /// Program files of the tool. A process started from one of them is the tool running.
    #[serde(default)]
    pub executables: Vec<String>,
    /// Other files or folders whose existence shows the tool is installed or has been used.
    #[serde(default)]
    pub paths: Vec<String>,
}

/// Sections checked on a real install, per platform.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verified {
    #[serde(default)]
    pub detect: BTreeMap<Platform, Check>,
}

/// One check of a section on a real install.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    /// The tool version that was checked.
    pub version: String,
    /// The day of the check, a plain TOML date such as `2026-10-26`.
    pub date: Datetime,
}

/// Why a definition is invalid.
#[derive(Debug, Error)]
pub enum DefinitionError {
    #[error(transparent)]
    Syntax(#[from] toml::de::Error),
    #[error("{0}")]
    Rule(String),
}

/// Why definitions could not be loaded.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("cannot read {}", .path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("invalid definition {}", .path.display())]
    Invalid {
        path: PathBuf,
        source: DefinitionError,
    },
}

impl Definition {
    /// Parses and validates one definition.
    pub fn parse(text: &str) -> Result<Self, DefinitionError> {
        let definition: Self = toml::from_str(text)?;
        definition.validate().map_err(DefinitionError::Rule)?;
        Ok(definition)
    }

    fn validate(&self) -> Result<(), String> {
        if !is_valid_id(&self.id) {
            return Err(format!(
                "id `{}` must use only lowercase letters, digits and dashes",
                self.id
            ));
        }
        if self.name.trim().is_empty() || self.vendor.trim().is_empty() {
            return Err("name and vendor must not be empty".into());
        }
        if self.platforms.is_empty() {
            return Err("platforms must not be empty".into());
        }
        for (platform, detect) in &self.detect {
            if !self.platforms.contains(platform) {
                return Err(format!(
                    "detect.{platform} is for a platform not listed in platforms"
                ));
            }
            for entry in detect.executables.iter().chain(&detect.paths) {
                pattern::check(entry).map_err(|error| format!("detect.{platform}: {error}"))?;
            }
        }
        for (platform, check) in &self.verified.detect {
            let section = format!("verified.detect.{platform}");
            if !self.detect.contains_key(platform) {
                return Err(format!(
                    "{section} refers to a missing detect.{platform} section"
                ));
            }
            if check.version.trim().is_empty() {
                return Err(format!("{section}.version must not be empty"));
            }
            let date = &check.date;
            if date.date.is_none() || date.time.is_some() || date.offset.is_some() {
                return Err(format!(
                    "{section}.date must be a plain date such as 2026-10-26"
                ));
            }
        }
        Ok(())
    }
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Loads and validates every `*.toml` file in `dir`, in file-name order.
pub fn load_dir(dir: &Path) -> Result<Vec<Definition>, LoadError> {
    let read_error = |source| LoadError::Read {
        path: dir.to_owned(),
        source,
    };
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(read_error)? {
        let path = entry.map_err(read_error)?.path();
        if path.extension() == Some(OsStr::new("toml")) {
            files.push(path);
        }
    }
    files.sort();
    files.iter().map(|path| load_file(path)).collect()
}

/// Loads and validates one definition file. Its id must match the file name.
pub fn load_file(path: &Path) -> Result<Definition, LoadError> {
    let invalid = |source| LoadError::Invalid {
        path: path.to_owned(),
        source,
    };
    let text = fs::read_to_string(path).map_err(|source| LoadError::Read {
        path: path.to_owned(),
        source,
    })?;
    let definition = Definition::parse(&text).map_err(invalid)?;
    if path.file_stem() != Some(OsStr::new(&definition.id)) {
        return Err(invalid(DefinitionError::Rule(format!(
            "id `{}` does not match the file name",
            definition.id
        ))));
    }
    Ok(definition)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
id = "codex-cli"
name = "Codex CLI"
vendor = "OpenAI"
kind = "cli"
platforms = ["windows", "linux"]

[detect.windows]
executables = ["%LOCALAPPDATA%/Programs/OpenAI/Codex/bin/codex.exe"]
paths = ["~/.codex"]

[verified.detect.windows]
version = "0.164.0"
date = 2026-10-26
"#;

    fn rule_error(text: &str) -> String {
        match Definition::parse(text) {
            Err(DefinitionError::Rule(message)) => message,
            other => panic!("expected a rule error, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_valid_definition() {
        let definition = Definition::parse(VALID).unwrap();
        assert_eq!(definition.id, "codex-cli");
        assert_eq!(definition.kind, Kind::Cli);
        assert_eq!(definition.detect[&Platform::Windows].paths, ["~/.codex"]);
        assert_eq!(
            definition.verified.detect[&Platform::Windows].version,
            "0.164.0"
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let text = VALID.replace("paths =", "pathz =");
        assert!(matches!(
            Definition::parse(&text),
            Err(DefinitionError::Syntax(_))
        ));
    }

    #[test]
    fn rejects_bad_ids() {
        for id in ["Codex", "codex_cli", "-codex", ""] {
            let text = VALID.replace("\"codex-cli\"", &format!("\"{id}\""));
            assert!(rule_error(&text).contains("id"), "{id}");
        }
    }

    #[test]
    fn rejects_detect_for_an_unlisted_platform() {
        let text = VALID.replace(r#"["windows", "linux"]"#, r#"["linux"]"#);
        assert!(rule_error(&text).contains("detect.windows"));
    }

    #[test]
    fn rejects_bad_patterns() {
        let text = VALID.replace(r#""~/.codex""#, r#"".codex""#);
        assert!(rule_error(&text).contains("detect.windows"));
    }

    #[test]
    fn rejects_verification_without_a_detect_section() {
        let text = r#"
id = "x"
name = "X"
vendor = "V"
kind = "cli"
platforms = ["windows"]

[verified.detect.windows]
version = "1"
date = 2026-10-26
"#;
        assert!(rule_error(text).contains("missing detect.windows"));
    }

    #[test]
    fn rejects_verification_dates_with_a_time() {
        let text = VALID.replace("date = 2026-10-26", "date = 2026-10-26T10:00:00Z");
        assert!(rule_error(&text).contains("plain date"));
    }

    #[test]
    fn loads_toml_files_in_name_order() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("codex-cli.toml"), VALID).unwrap();
        fs::write(
            dir.path().join("a-tool.toml"),
            VALID.replace("codex-cli", "a-tool"),
        )
        .unwrap();
        fs::write(dir.path().join("README.md"), "not a definition").unwrap();
        let ids: Vec<_> = load_dir(dir.path())
            .unwrap()
            .into_iter()
            .map(|definition| definition.id)
            .collect();
        assert_eq!(ids, ["a-tool", "codex-cli"]);
    }

    #[test]
    fn rejects_an_id_that_does_not_match_the_file_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("other.toml"), VALID).unwrap();
        assert!(matches!(
            load_dir(dir.path()),
            Err(LoadError::Invalid { .. })
        ));
    }
}
