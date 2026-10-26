//! Collectors. Each one asks the OS for one kind of fact and returns it as records.

mod filesystem;
mod processes;

pub use filesystem::Filesystem;
pub use processes::Processes;

use std::path::PathBuf;

use aieye_defs::{Definition, Platform};
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
    let collectors: [&dyn Collector; 2] = [&Filesystem, &Processes];
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

    /// Expands one `detect` list of every definition for this platform into
    /// (tool id, glob pattern) pairs. Entries that cannot be expanded here, for
    /// example because a variable is not set, are skipped.
    fn patterns(&self, list: List) -> Vec<(&str, String)> {
        let mut patterns = Vec::new();
        for definition in &self.definitions {
            let Some(detect) = definition.detect.get(&self.platform) else {
                continue;
            };
            let entries = match list {
                List::Executables => &detect.executables,
                List::Paths => &detect.paths,
            };
            for entry in entries {
                match aieye_defs::expand(entry, self.home.as_deref(), &self.var) {
                    Ok(pattern) => patterns.push((definition.id.as_str(), pattern)),
                    Err(error) => {
                        tracing::debug!(tool = %definition.id, %entry, %error, "skipping entry");
                    }
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

/// The two lists in a definition's `detect` section.
#[derive(Clone, Copy)]
enum List {
    Executables,
    Paths,
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

    /// A context with one definition, `test-tool`, whose `detect` section for this
    /// platform is `detect`.
    pub(crate) fn context(home: &Path, detect: &str) -> Context {
        let platform = Platform::current().expect("tests run on a supported platform");
        let text = format!(
            "id = \"test-tool\"\nname = \"Test tool\"\nvendor = \"Test\"\nkind = \"cli\"\n\
             platforms = [\"{platform}\"]\n\n[detect.{platform}]\n{detect}\n"
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
}
