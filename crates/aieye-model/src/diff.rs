//! What changed between two snapshots, and which changes are alerts (concept §5.5).
//!
//! The rules are fixed and deterministic:
//!
//! - **Alert:** a tool that was not in the older snapshot; a configured folder or MCP
//!   server that was added or changed.
//! - **Info:** everything else, such as anything removed, an install trace found or
//!   gone, or a tool running from a new executable.
//!
//! A change carries the records behind it, so it keeps their evidence labels.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Fact, Record};

/// What makes two facts from different scans the same thing. Facts with the same key
/// are compared field by field; a running process's PID is ignored.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FactKey {
    ToolPresent {
        tool: String,
        path: PathBuf,
    },
    ToolRunning {
        tool: String,
        exe: PathBuf,
    },
    ConfiguredFolder {
        tool: String,
        file: PathBuf,
        path: String,
    },
    McpServer {
        tool: String,
        file: PathBuf,
        scope: Option<String>,
        name: String,
    },
}

impl Fact {
    /// The AI tool the fact is about.
    pub fn tool(&self) -> &str {
        match self {
            Self::ToolPresent { tool, .. }
            | Self::ToolRunning { tool, .. }
            | Self::ConfiguredFolder { tool, .. }
            | Self::McpServer { tool, .. } => tool,
        }
    }

    /// What identifies this fact across scans.
    pub fn key(&self) -> FactKey {
        match self.clone() {
            Self::ToolPresent { tool, path } => FactKey::ToolPresent { tool, path },
            Self::ToolRunning { tool, exe, .. } => FactKey::ToolRunning { tool, exe },
            Self::ConfiguredFolder {
                tool, file, path, ..
            } => FactKey::ConfiguredFolder { tool, file, path },
            Self::McpServer {
                tool,
                file,
                scope,
                name,
                ..
            } => FactKey::McpServer {
                tool,
                file,
                scope,
                name,
            },
        }
    }

    /// Whether `newer`, a fact with the same key, says something different.
    fn differs_from(&self, newer: &Fact) -> bool {
        match (self, newer) {
            (Self::ToolRunning { .. }, Self::ToolRunning { .. }) => false,
            _ => self != newer,
        }
    }
}

/// How much a change matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// New or wider access: worth telling the user about.
    Alert,
    Info,
}

/// What happened to a fact between two snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Changed,
    Removed,
}

/// One fact that differs between two snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub severity: Severity,
    pub kind: ChangeKind,
    pub tool: String,
    /// The record in the older snapshot, unless the fact was added.
    pub before: Option<Record>,
    /// The record in the newer snapshot, unless the fact was removed.
    pub after: Option<Record>,
}

impl Change {
    /// The newer record's fact, or the older one's if the fact was removed.
    pub fn fact(&self) -> &Fact {
        let record = self.after.as_ref().or(self.before.as_ref());
        &record.expect("a change has at least one record").fact
    }
}

/// Everything that differs between two snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diff {
    /// Tools with facts in the newer snapshot but none in the older one. Each is an alert.
    pub new_tools: Vec<String>,
    /// Tools with facts in the older snapshot but none in the newer one.
    pub gone_tools: Vec<String>,
    /// Changed facts, alerts first, then by tool.
    pub changes: Vec<Change>,
}

impl Diff {
    /// The number of alerts: new tools and alert-level changes.
    pub fn alerts(&self) -> usize {
        self.new_tools.len()
            + self
                .changes
                .iter()
                .filter(|change| change.severity == Severity::Alert)
                .count()
    }

    pub fn is_empty(&self) -> bool {
        self.new_tools.is_empty() && self.gone_tools.is_empty() && self.changes.is_empty()
    }
}

/// Compares the records of an older and a newer snapshot.
pub fn diff(before: &[Record], after: &[Record]) -> Diff {
    let older = by_key(before);
    let newer = by_key(after);
    let mut changes = Vec::new();
    for (key, record) in &newer {
        match older.get(key) {
            None => changes.push(change(ChangeKind::Added, None, Some(record))),
            Some(previous) if previous.fact.differs_from(&record.fact) => {
                changes.push(change(ChangeKind::Changed, Some(previous), Some(record)));
            }
            Some(_) => {}
        }
    }
    for (key, record) in &older {
        if !newer.contains_key(key) {
            changes.push(change(ChangeKind::Removed, Some(record), None));
        }
    }
    changes.sort_by_cached_key(|change| {
        (
            change.severity,
            change.tool.clone(),
            change.kind,
            change.fact().key(),
        )
    });

    let older_tools = tools(before);
    let newer_tools = tools(after);
    Diff {
        new_tools: newer_tools.difference(&older_tools).cloned().collect(),
        gone_tools: older_tools.difference(&newer_tools).cloned().collect(),
        changes,
    }
}

/// Records by key. Several running processes of one executable share a key; the
/// first stands for them all.
fn by_key(records: &[Record]) -> BTreeMap<FactKey, &Record> {
    let mut keyed = BTreeMap::new();
    for record in records {
        keyed.entry(record.fact.key()).or_insert(record);
    }
    keyed
}

fn tools(records: &[Record]) -> BTreeSet<String> {
    records
        .iter()
        .map(|record| record.fact.tool().to_owned())
        .collect()
}

fn change(kind: ChangeKind, before: Option<&Record>, after: Option<&Record>) -> Change {
    let fact = &after.or(before).expect("a change has a record").fact;
    let widens_access = matches!(fact, Fact::ConfiguredFolder { .. } | Fact::McpServer { .. });
    let severity = if widens_access && kind != ChangeKind::Removed {
        Severity::Alert
    } else {
        Severity::Info
    };
    Change {
        severity,
        kind,
        tool: fact.tool().to_owned(),
        before: before.cloned(),
        after: after.cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceId, Evidence, Source};

    fn record(fact: Fact) -> Record {
        let evidence = match fact {
            Fact::ConfiguredFolder { .. } | Fact::McpServer { .. } => Evidence::Declared,
            _ => Evidence::Verified,
        };
        Record {
            device_id: DeviceId::random(),
            org_id: None,
            source: Source::new("test"),
            evidence,
            observed_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            fact,
        }
    }

    fn present(tool: &str, path: &str) -> Record {
        record(Fact::ToolPresent {
            tool: tool.into(),
            path: path.into(),
        })
    }

    fn running(tool: &str, pid: u32, exe: &str) -> Record {
        record(Fact::ToolRunning {
            tool: tool.into(),
            pid,
            exe: exe.into(),
        })
    }

    fn folder(tool: &str, path: &str, trust: &str) -> Record {
        record(Fact::ConfiguredFolder {
            tool: tool.into(),
            file: "config.toml".into(),
            path: path.into(),
            trust: Some(trust.into()),
            boundary: false,
        })
    }

    fn server(tool: &str, name: &str, env: &[&str]) -> Record {
        record(Fact::McpServer {
            tool: tool.into(),
            file: "config.json".into(),
            scope: None,
            name: name.into(),
            command: Some("npx".into()),
            origin: None,
            env_names: env.iter().map(|name| (*name).to_owned()).collect(),
        })
    }

    fn summary(diff: &Diff) -> Vec<(Severity, ChangeKind, String)> {
        diff.changes
            .iter()
            .map(|change| (change.severity, change.kind, change.tool.clone()))
            .collect()
    }

    #[test]
    fn identical_snapshots_have_no_changes() {
        let records = [present("a", "/a"), folder("a", "/work", "trusted")];
        assert!(diff(&records, &records).is_empty());
    }

    #[test]
    fn new_or_wider_access_is_an_alert_and_the_rest_is_info() {
        let before = [
            present("a", "/a"),
            folder("a", "/work", "untrusted"),
            folder("a", "/old", "trusted"),
            server("a", "files", &["TOKEN"]),
        ];
        let after = [
            present("a", "/a"),
            present("a", "/a2"),
            folder("a", "/work", "trusted"),
            folder("a", "/new", "trusted"),
            server("a", "files", &["TOKEN", "EXTRA"]),
            server("a", "mail", &[]),
        ];

        let diff = diff(&before, &after);

        use ChangeKind::*;
        use Severity::*;
        let a = || "a".to_owned();
        assert_eq!(
            summary(&diff),
            [
                (Alert, Added, a()),
                (Alert, Added, a()),
                (Alert, Changed, a()),
                (Alert, Changed, a()),
                (Info, Added, a()),
                (Info, Removed, a()),
            ]
        );
        assert_eq!(diff.alerts(), 4);
        let changed = &diff.changes[2];
        assert!(matches!(
            changed.before.as_ref().map(|record| &record.fact),
            Some(Fact::ConfiguredFolder { trust: Some(trust), .. }) if trust == "untrusted"
        ));
        assert_eq!(changed.after.as_ref().unwrap().evidence, Evidence::Declared);
    }

    #[test]
    fn running_processes_compare_by_executable_not_pid() {
        let before = [running("a", 10, "/a/1.0/a.exe")];
        let same_exe = [
            running("a", 99, "/a/1.0/a.exe"),
            running("a", 100, "/a/1.0/a.exe"),
        ];
        assert!(diff(&before, &same_exe).is_empty());

        let updated = [running("a", 11, "/a/1.1/a.exe")];
        let diff = diff(&before, &updated);
        assert_eq!(
            summary(&diff),
            [
                (Severity::Info, ChangeKind::Added, "a".into()),
                (Severity::Info, ChangeKind::Removed, "a".into()),
            ]
        );
    }

    #[test]
    fn tools_that_appear_are_alerts_and_tools_that_go_are_listed() {
        let before = [present("old", "/old")];
        let after = [present("new", "/new")];

        let diff = diff(&before, &after);

        assert_eq!(diff.new_tools, ["new"]);
        assert_eq!(diff.gone_tools, ["old"]);
        assert_eq!(diff.alerts(), 1);
    }
}
