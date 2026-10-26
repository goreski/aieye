//! Text output for people: what changed between snapshots, and the snapshot history.

use aieye_model::{Diff, Severity};
use aieye_store::SnapshotInfo;
use jiff::Timestamp;
use jiff::tz::TimeZone;

/// The changes between two snapshots, one line each: new tools, then alerts, then
/// the rest.
pub fn diff(from: &SnapshotInfo, to: &SnapshotInfo, diff: &Diff) -> String {
    let mut out = format!(
        "Changes from snapshot {} ({}) to {} ({})\n",
        from.id,
        local(from.taken_at),
        to.id,
        local(to.taken_at)
    );
    if diff.is_empty() {
        out.push_str("No changes.\n");
        return out;
    }

    let mut lines: Vec<(&str, &str, &str, String)> = Vec::new();
    for tool in &diff.new_tools {
        lines.push(("ALERT", tool, "new tool", String::new()));
    }
    for change in &diff.changes {
        let level = match change.severity {
            Severity::Alert => "ALERT",
            Severity::Info => "info",
        };
        let (what, detail) = change.describe();
        lines.push((level, &change.tool, what, detail));
    }
    for tool in &diff.gone_tools {
        lines.push(("info", tool, "tool gone", String::new()));
    }

    let tool_width = lines.iter().map(|line| line.1.len()).max().unwrap_or(0);
    let what_width = lines.iter().map(|line| line.2.len()).max().unwrap_or(0);
    for (level, tool, what, detail) in lines {
        let line = format!("{level:<5}  {tool:<tool_width$}  {what:<what_width$}  {detail}");
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// Saved snapshots with their alerts compared with the one before.
pub fn history(snapshots: &[SnapshotInfo], alerts: &[Option<usize>]) -> String {
    let mut out = format!(
        "{:>5}  {:<16}  {:>7}  {:>6}\n",
        "ID", "Taken (local)", "Records", "Alerts"
    );
    for (info, alerts) in snapshots.iter().zip(alerts) {
        let alerts = alerts.map_or_else(|| "-".to_owned(), |count| count.to_string());
        out.push_str(&format!(
            "{:>5}  {:<16}  {:>7}  {:>6}\n",
            info.id,
            local(info.taken_at),
            info.records,
            alerts
        ));
    }
    out
}

/// A time in the local time zone, to the minute.
fn local(time: Timestamp) -> String {
    time.to_zoned(TimeZone::system())
        .strftime("%Y-%m-%d %H:%M")
        .to_string()
}

#[cfg(test)]
mod tests {
    use aieye_model::{DeviceId, Evidence, Fact, Record, Source};

    use super::*;

    fn info(id: i64) -> SnapshotInfo {
        SnapshotInfo {
            id,
            device_id: DeviceId::random(),
            taken_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            records: 1,
            aieye_version: "0.1.0".into(),
        }
    }

    fn record(fact: Fact) -> Record {
        Record {
            device_id: DeviceId::random(),
            org_id: None,
            source: Source::new("config"),
            evidence: Evidence::Declared,
            observed_at: "2026-10-26T12:00:00Z".parse().unwrap(),
            fact,
        }
    }

    fn folder(trust: &str) -> Record {
        record(Fact::ConfiguredFolder {
            tool: "codex-cli".into(),
            file: "config.toml".into(),
            path: r"C:\work".into(),
            trust: Some(trust.into()),
            boundary: false,
        })
    }

    #[test]
    fn describes_each_change_on_one_line() {
        let server = record(Fact::McpServer {
            tool: "claude-desktop".into(),
            file: "config.json".into(),
            scope: None,
            name: "github".into(),
            command: Some("npx".into()),
            origin: None,
            env_names: vec!["GITHUB_TOKEN".into()],
        });
        let changes = aieye_model::diff(&[folder("untrusted")], &[folder("trusted"), server]);

        let text = diff(&info(1), &info(2), &changes);

        let lines: Vec<&str> = text.lines().skip(1).collect();
        assert_eq!(
            lines,
            [
                "ALERT  claude-desktop  new tool",
                "ALERT  claude-desktop  MCP server added  github (npx; env GITHUB_TOKEN)",
                r"ALERT  codex-cli       folder changed    C:\work: trust untrusted → trusted",
            ]
        );
    }

    #[test]
    fn says_so_when_nothing_changed() {
        let records = [folder("trusted")];
        let text = diff(&info(1), &info(2), &aieye_model::diff(&records, &records));
        assert!(text.ends_with("No changes.\n"));
    }

    #[test]
    fn history_marks_the_first_snapshot_as_having_nothing_to_compare() {
        let text = history(&[info(1), info(2)], &[None, Some(3)]);
        let rows: Vec<Vec<&str>> = text
            .lines()
            .skip(1)
            .map(|line| line.split_whitespace().collect())
            .collect();
        assert_eq!(rows[0].last(), Some(&"-"));
        assert_eq!(rows[1].last(), Some(&"3"));
    }
}
