//! Collectors. Each one asks the OS, or a tool's own config, for one kind of fact
//! and returns it as records.

mod configs;
mod filesystem;
mod processes;
mod tasks;

pub use configs::Configs;
pub use filesystem::Filesystem;
pub use processes::Processes;
pub use tasks::ScheduledTasks;

use std::path::PathBuf;

use aieye_defs::{Definition, Detect, Platform};
use aieye_model::{DeviceId, Evidence, Fact, OrgId, Record, Snapshot, Source};
use anyhow::{Context as _, Result};
use glob::MatchOptions;
use jiff::Timestamp;

/// One source of facts.
pub trait Collector {
    /// The record source this collector reports, e.g. `os:filesystem`.
    fn source(&self) -> &'static str;

    /// Collects the current facts.
    fn collect(&self, ctx: &Context) -> Result<Vec<Record>>;
}

/// Runs every collector and combines their records into one snapshot.
pub fn scan(ctx: &Context) -> Result<Snapshot> {
    let taken_at = Timestamp::now();
    let collectors: [&dyn Collector; 4] = [&Filesystem, &Processes, &Configs, &ScheduledTasks];
    let mut records = Vec::new();
    for collector in collectors {
        let found = collector
            .collect(ctx)
            .with_context(|| format!("collector `{}` failed", collector.source()))?;
        records.extend(found);
    }
    Ok(Snapshot {
        device_id: ctx.device_id,
        taken_at,
        records,
    })
}

/// Looks up an environment variable by name.
pub type VarLookup = Box<dyn Fn(&str) -> Option<String>>;

/// What collectors need to know about the device and the user being scanned.
pub struct Context {
    pub device_id: DeviceId,
    pub org_id: Option<OrgId>,
    pub platform: Platform,
    pub definitions: Vec<Definition>,
    /// The home folder that `~/` patterns expand to.
    pub home: Option<PathBuf>,
    /// Looks up the environment variables that `%NAME%` patterns expand to.
    pub var: VarLookup,
}

impl Context {
    /// A context for the signed-in user on this machine.
    pub fn current(device_id: DeviceId, definitions: Vec<Definition>) -> Result<Self> {
        let platform =
            Platform::current().context("AIEye does not support this operating system yet")?;
        Ok(Self {
            device_id,
            org_id: None,
            platform,
            definitions,
            home: dirs::home_dir(),
            var: Box::new(|name| std::env::var(name).ok()),
        })
    }

    /// Expands a definition entry into a glob pattern for this machine. Entries that
    /// cannot be expanded here, for example because a variable is not set, are
    /// logged and skipped.
    fn expand(&self, tool: &str, entry: &str) -> Option<String> {
        match aieye_defs::expand(entry, self.home.as_deref(), &self.var) {
            Ok(pattern) => Some(pattern),
            Err(error) => {
                tracing::debug!(%tool, %entry, %error, "skipping entry");
                None
            }
        }
    }

    /// The existing files and folders that a definition entry matches. Folders AIEye
    /// cannot list are skipped.
    fn existing(&self, tool: &str, entry: &str) -> Vec<PathBuf> {
        let Some(pattern) = self.expand(tool, entry) else {
            return Vec::new();
        };
        let matches = match glob::glob_with(&pattern, match_options()) {
            Ok(matches) => matches,
            Err(error) => {
                tracing::debug!(%tool, %pattern, %error, "skipping pattern");
                return Vec::new();
            }
        };
        let mut paths = Vec::new();
        for found in matches {
            match found {
                Ok(path) => paths.push(path),
                Err(error) => tracing::debug!(%tool, %error, "cannot read folder"),
            }
        }
        paths
    }

    /// The glob patterns of every definition's `detect.executables` for this
    /// platform, as (tool id, pattern) pairs.
    fn executable_patterns(&self) -> Vec<(&str, String)> {
        self.detect_patterns(|detect| &detect.executables)
    }

    /// The glob patterns of every definition's `detect.paths` for this platform, as
    /// (tool id, pattern) pairs.
    fn path_patterns(&self) -> Vec<(&str, String)> {
        self.detect_patterns(|detect| &detect.paths)
    }

    fn detect_patterns(&self, list: fn(&Detect) -> &Vec<String>) -> Vec<(&str, String)> {
        let mut patterns = Vec::new();
        for definition in &self.definitions {
            let Some(detect) = definition.detect.get(&self.platform) else {
                continue;
            };
            for entry in list(detect) {
                if let Some(pattern) = self.expand(&definition.id, entry) {
                    patterns.push((definition.id.as_str(), pattern));
                }
            }
        }
        patterns
    }

    fn record(
        &self,
        source: &str,
        evidence: Evidence,
        observed_at: Timestamp,
        fact: Fact,
    ) -> Record {
        Record {
            device_id: self.device_id,
            org_id: self.org_id.clone(),
            source: Source::new(source),
            evidence,
            observed_at,
            fact,
        }
    }
}

/// How definition patterns match: case-insensitive on Windows, and `*` never
/// crosses a folder boundary.
fn match_options() -> MatchOptions {
    MatchOptions {
        case_sensitive: !cfg!(windows),
        require_literal_separator: true,
        require_literal_leading_dot: false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    pub(crate) fn platform() -> Platform {
        Platform::current().expect("tests run on a supported platform")
    }

    /// A context with one definition, `test-tool`, made of the TOML `sections`.
    pub(crate) fn context_with(home: &Path, sections: &str) -> Context {
        let platform = platform();
        let text = format!(
            "id = \"test-tool\"\nname = \"Test tool\"\nvendor = \"Test\"\nkind = \"cli\"\n\
             platforms = [\"{platform}\"]\n\n{sections}\n"
        );
        Context {
            device_id: DeviceId::random(),
            org_id: None,
            platform,
            definitions: vec![Definition::parse(&text).unwrap()],
            home: Some(home.to_owned()),
            var: Box::new(|_| None),
        }
    }

    /// A context with one definition, `test-tool`, whose `detect` section for this
    /// platform is `detect`.
    pub(crate) fn context(home: &Path, detect: &str) -> Context {
        context_with(home, &format!("[detect.{}]\n{detect}", platform()))
    }
}
