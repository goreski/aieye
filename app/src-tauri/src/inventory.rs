//! What the window shows: a saved snapshot grouped by tool, and the changes since
//! the snapshot before it. Grouping happens here, in tested Rust, so the screens
//! only display.

use aieye_defs::{Definition, Platform};
use aieye_model::{Diff, Fact, Record, Severity, Snapshot};
use aieye_store::SnapshotInfo;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ScanView {
    pub snapshot: SnapshotInfo,
    pub totals: Totals,
    /// Tools with facts on this device: running first, then installed, then by name.
    pub tools: Vec<ToolView>,
    /// Names of known tools with no facts on this device.
    pub not_found: Vec<String>,
    /// Every scheduled task on the device outside Windows' own, linked to an AI tool
    /// or not, by path.
    pub tasks: Vec<Record>,
    /// Changes since the snapshot before, unless this is the first.
    pub changes: Option<ChangesView>,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct Totals {
    pub tools: usize,
    /// Running AI processes.
    pub running: usize,
    pub folders: usize,
    pub servers: usize,
    /// Schedules linked to AI tools: their own, and scheduled tasks that run them.
    pub schedules: usize,
    /// Scheduled tasks on the device, linked or not.
    pub tasks: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    Installed,
    /// Seen only in another tool's config.
    ConfigOnly,
}

#[derive(Debug, Serialize)]
pub struct ToolView {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub status: Status,
    /// Sections of the tool's definition checked on a real install of this platform.
    pub verified: Vec<&'static str>,
    pub present: Vec<Record>,
    pub running: Vec<Record>,
    pub folders: Vec<Record>,
    pub servers: Vec<Record>,
    /// The tool's own schedules and the scheduled tasks that run it.
    pub schedules: Vec<Record>,
}

#[derive(Debug, Serialize)]
pub struct ChangesView {
    pub from: SnapshotInfo,
    pub alerts: usize,
    /// New tools, then changed facts (alerts first), then tools that are gone.
    pub lines: Vec<ChangeLine>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ChangeLine {
    pub severity: Severity,
    /// The AI tool, unless the change belongs to none, as a new scheduled task may.
    pub tool: Option<String>,
    pub what: &'static str,
    pub detail: String,
}

/// Builds the view of a saved snapshot. `previous` is the snapshot before it and the
/// diff between the two.
pub fn view(
    info: SnapshotInfo,
    snapshot: &Snapshot,
    definitions: &[Definition],
    platform: Platform,
    previous: Option<(SnapshotInfo, Diff)>,
) -> ScanView {
    let mut tools: Vec<ToolView> = Vec::new();
    let mut tasks = Vec::new();
    for record in &snapshot.records {
        if let Fact::ScheduledTask { .. } = record.fact {
            tasks.push(record.clone());
        }
        let Some(id) = record.fact.tool() else {
            continue;
        };
        let index = match tools.iter().position(|tool| tool.id == id) {
            Some(index) => index,
            None => {
                tools.push(tool_view(id, definitions, platform));
                tools.len() - 1
            }
        };
        let tool = &mut tools[index];
        let list = match record.fact {
            Fact::ToolPresent { .. } => &mut tool.present,
            Fact::ToolRunning { .. } => &mut tool.running,
            Fact::ConfiguredFolder { .. } => &mut tool.folders,
            Fact::McpServer { .. } => &mut tool.servers,
            Fact::ScheduledTask { .. } | Fact::ToolSchedule { .. } => &mut tool.schedules,
        };
        list.push(record.clone());
    }
    tasks.sort_by_cached_key(|record| match &record.fact {
        Fact::ScheduledTask { path, .. } => path.to_lowercase(),
        _ => String::new(),
    });
    for tool in &mut tools {
        tool.status = if !tool.running.is_empty() {
            Status::Running
        } else if !tool.present.is_empty() {
            Status::Installed
        } else {
            Status::ConfigOnly
        };
    }
    tools.sort_by(|a, b| (a.status, &a.name).cmp(&(b.status, &b.name)));

    let totals = Totals {
        tools: tools.len(),
        running: tools.iter().map(|tool| tool.running.len()).sum(),
        folders: tools.iter().map(|tool| tool.folders.len()).sum(),
        servers: tools.iter().map(|tool| tool.servers.len()).sum(),
        schedules: tools.iter().map(|tool| tool.schedules.len()).sum(),
        tasks: tasks.len(),
    };
    let not_found = definitions
        .iter()
        .filter(|definition| !tools.iter().any(|tool| tool.id == definition.id))
        .map(|definition| definition.name.clone())
        .collect();
    let changes = previous.map(|(from, diff)| ChangesView {
        alerts: diff.alerts(),
        lines: change_lines(&diff),
        from,
    });
    ScanView {
        snapshot: info,
        totals,
        tools,
        not_found,
        tasks,
        changes,
    }
}

fn tool_view(id: &str, definitions: &[Definition], platform: Platform) -> ToolView {
    let definition = definitions.iter().find(|definition| definition.id == id);
    let mut verified = Vec::new();
    if let Some(definition) = definition {
        let checked = &definition.verified;
        if checked.detect.contains_key(&platform) {
            verified.push("detect");
        }
        if checked.folders.contains_key(&platform) {
            verified.push("folders");
        }
        if checked.mcp.contains_key(&platform) {
            verified.push("mcp");
        }
        if checked.schedules.contains_key(&platform) {
            verified.push("schedules");
        }
    }
    ToolView {
        id: id.to_owned(),
        name: definition.map_or_else(|| id.to_owned(), |d| d.name.clone()),
        vendor: definition.map(|d| d.vendor.clone()).unwrap_or_default(),
        status: Status::ConfigOnly,
        verified,
        present: Vec::new(),
        running: Vec::new(),
        folders: Vec::new(),
        servers: Vec::new(),
        schedules: Vec::new(),
    }
}

fn change_lines(diff: &Diff) -> Vec<ChangeLine> {
    let line = |severity, tool: Option<&str>, what, detail| ChangeLine {
        severity,
        tool: tool.map(str::to_owned),
        what,
        detail,
    };
    let mut lines: Vec<ChangeLine> = diff
        .new_tools
        .iter()
        .map(|tool| line(Severity::Alert, Some(tool), "new tool", String::new()))
        .collect();
    lines.extend(diff.changes.iter().map(|change| {
        let (what, detail) = change.describe();
        line(change.severity, change.tool.as_deref(), what, detail)
    }));
    lines.extend(
        diff.gone_tools
            .iter()
            .map(|tool| line(Severity::Info, Some(tool), "tool gone", String::new())),
    );
    lines
}

#[cfg(test)]
mod tests {
    use aieye_model::{DeviceId, Evidence, Source};

    use super::*;

    const WINDOWS: Platform = Platform::Windows;

    fn definition(id: &str, name: &str, verified: bool) -> Definition {
        let check = if verified {
            "[verified.detect.windows]\nversion = \"1\"\ndate = 2026-10-26\n"
        } else {
            ""
        };
        Definition::parse(&format!(
            "id = \"{id}\"\nname = \"{name}\"\nvendor = \"Vendor\"\nkind = \"cli\"\n\
             platforms = [\"windows\"]\n\n[detect.windows]\npaths = [\"~/.{id}\"]\n\n{check}"
        ))
        .unwrap()
    }

    fn record(fact: Fact) -> Record {
        Record {
            device_id: DeviceId::random(),
            org_id: None,
            source: Source::new("test"),
            evidence: Evidence::Verified,
            observed_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            fact,
        }
    }

    fn info(id: i64) -> SnapshotInfo {
        SnapshotInfo {
            id,
            device_id: DeviceId::random(),
            taken_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            records: 0,
            aieye_version: "0.1.0".into(),
        }
    }

    fn snapshot(records: Vec<Record>) -> Snapshot {
        Snapshot {
            device_id: DeviceId::random(),
            taken_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            records,
        }
    }

    #[test]
    fn groups_facts_by_tool_with_status_and_totals() {
        let definitions = [
            definition("alpha", "Alpha", true),
            definition("beta", "Beta", false),
            definition("gamma", "Gamma", false),
        ];
        let records = vec![
            record(Fact::ToolPresent {
                tool: "beta".into(),
                path: "/beta".into(),
            }),
            record(Fact::ToolPresent {
                tool: "alpha".into(),
                path: "/alpha".into(),
            }),
            record(Fact::ToolRunning {
                tool: "beta".into(),
                pid: 7,
                exe: "/beta/beta.exe".into(),
            }),
            record(Fact::ConfiguredFolder {
                tool: "alpha".into(),
                file: "config.toml".into(),
                path: "/work".into(),
                trust: None,
                boundary: false,
            }),
        ];

        let view = view(info(1), &snapshot(records), &definitions, WINDOWS, None);

        let order: Vec<(&str, Status)> = view
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool.status))
            .collect();
        assert_eq!(
            order,
            [("Beta", Status::Running), ("Alpha", Status::Installed)]
        );
        assert_eq!(
            view.totals,
            Totals {
                tools: 2,
                running: 1,
                folders: 1,
                servers: 0,
                schedules: 0,
                tasks: 0,
            }
        );
        assert_eq!(view.tools[1].verified, ["detect"]);
        assert!(view.tools[0].verified.is_empty());
        assert_eq!(view.not_found, ["Gamma"]);
        assert!(view.changes.is_none());
    }

    #[test]
    fn lists_new_tools_then_changes_then_gone_tools() {
        let before = vec![record(Fact::ToolPresent {
            tool: "old".into(),
            path: "/old".into(),
        })];
        let after = vec![record(Fact::McpServer {
            tool: "new".into(),
            file: "config.json".into(),
            scope: None,
            name: "files".into(),
            command: Some("files-server".into()),
            origin: None,
            env_names: Vec::new(),
        })];
        let diff = aieye_model::diff(&before, &after);

        let view = view(
            info(2),
            &snapshot(after),
            &[],
            WINDOWS,
            Some((info(1), diff)),
        );

        let changes = view.changes.unwrap();
        let lines: Vec<(Severity, Option<&str>, &str)> = changes
            .lines
            .iter()
            .map(|line| (line.severity, line.tool.as_deref(), line.what))
            .collect();
        assert_eq!(
            lines,
            [
                (Severity::Alert, Some("new"), "new tool"),
                (Severity::Alert, Some("new"), "MCP server added"),
                (Severity::Info, Some("old"), "gone"),
                (Severity::Info, Some("old"), "tool gone"),
            ]
        );
        assert_eq!(changes.alerts, 2);
        assert_eq!(changes.from.id, 1);
    }

    fn task(tool: Option<&str>, path: &str) -> Record {
        record(Fact::ScheduledTask {
            tool: tool.map(str::to_owned),
            path: path.into(),
            enabled: true,
            author: None,
            run_as: None,
            actions: Vec::new(),
            triggers: vec!["at logon".into()],
            last_run: None,
            next_run: None,
        })
    }

    #[test]
    fn lists_every_scheduled_task_and_gives_linked_ones_to_their_tool() {
        let definitions = [definition("alpha", "Alpha", false)];
        let records = vec![
            task(None, r"\Zeta"),
            task(Some("alpha"), r"\Alpha nightly"),
            record(Fact::ToolSchedule {
                tool: "alpha".into(),
                file: "tasks.json".into(),
                name: "digest".into(),
                schedule: Some("0 7 * * *".into()),
                folder: None,
                enabled: Some(true),
            }),
        ];

        let view = view(info(1), &snapshot(records), &definitions, WINDOWS, None);

        let paths: Vec<&str> = view
            .tasks
            .iter()
            .map(|record| match &record.fact {
                Fact::ScheduledTask { path, .. } => path.as_str(),
                other => panic!("unexpected fact {other:?}"),
            })
            .collect();
        assert_eq!(paths, [r"\Alpha nightly", r"\Zeta"]);
        assert_eq!(view.tools.len(), 1);
        assert_eq!(view.tools[0].schedules.len(), 2);
        assert_eq!((view.totals.schedules, view.totals.tasks), (2, 2));
    }
}
