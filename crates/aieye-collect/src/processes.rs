//! Finds running processes started from executables that definitions name.

use std::path::PathBuf;

use aieye_model::{Evidence, Fact, Record};
use anyhow::Result;
use glob::Pattern;
use jiff::Timestamp;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::{Collector, Context, match_options};

/// Reports each running process whose executable matches a definition's
/// `detect.executables`.
///
/// Processes whose executable path AIEye cannot read, typically elevated ones or
/// those of other users, are skipped.
pub struct Processes;

impl Collector for Processes {
    fn source(&self) -> &'static str {
        "os:process-list"
    }

    fn collect(&self, ctx: &Context) -> Result<Vec<Record>> {
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
        );
        let observed_at = Timestamp::now();
        let running = system
            .processes()
            .iter()
            .filter_map(|(pid, process)| Some((pid.as_u32(), process.exe()?.to_path_buf())));
        Ok(match_running(ctx, running, observed_at, self.source()))
    }
}

/// Turns the running processes whose executable a definition names into records,
/// ordered by process ID.
fn match_running(
    ctx: &Context,
    running: impl IntoIterator<Item = (u32, PathBuf)>,
    observed_at: Timestamp,
    source: &str,
) -> Vec<Record> {
    // Patterns were validated when their definitions were loaded.
    let patterns: Vec<(&str, Pattern)> = ctx
        .executable_patterns()
        .into_iter()
        .filter_map(|(tool, pattern)| Some((tool, Pattern::new(&pattern).ok()?)))
        .collect();
    let mut running: Vec<_> = running.into_iter().collect();
    running.sort_by_key(|(pid, _)| *pid);
    running
        .into_iter()
        .filter_map(|(pid, exe)| {
            let (tool, _) = patterns
                .iter()
                .find(|(_, pattern)| pattern.matches_path_with(&exe, match_options()))?;
            let fact = Fact::ToolRunning {
                tool: (*tool).to_owned(),
                pid,
                exe,
            };
            Some(ctx.record(source, Evidence::Verified, observed_at, fact))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::tests::context;

    const HOME: &str = if cfg!(windows) {
        r"C:\Users\me"
    } else {
        "/home/me"
    };

    fn pids(records: &[Record]) -> Vec<u32> {
        records
            .iter()
            .map(|record| match record.fact {
                Fact::ToolRunning { pid, .. } => pid,
                ref other => panic!("unexpected fact {other:?}"),
            })
            .collect()
    }

    #[test]
    fn reports_processes_started_from_listed_executables() {
        let home = Path::new(HOME);
        let ctx = context(home, r#"executables = ["~/apps/*/tool.exe"]"#);
        let apps = home.join("apps");
        let running = [
            (12, apps.join("1.0").join("sub").join("tool.exe")),
            (11, apps.join("1.0").join("other.exe")),
            (10, apps.join("1.0").join("tool.exe")),
            (9, apps.join("2.0").join("tool.exe")),
        ];

        let records = match_running(&ctx, running, Timestamp::now(), "os:process-list");

        assert_eq!(pids(&records), [9, 10]);
        assert!(matches!(
            &records[1].fact,
            Fact::ToolRunning { tool, exe, .. }
                if tool == "test-tool" && *exe == apps.join("1.0").join("tool.exe")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn matching_ignores_case_on_windows() {
        let ctx = context(Path::new(HOME), r#"executables = ["~/Apps/Tool.exe"]"#);
        let running = [(7, PathBuf::from(r"c:\users\ME\apps\TOOL.EXE"))];

        let records = match_running(&ctx, running, Timestamp::now(), "os:process-list");

        assert_eq!(pids(&records), [7]);
    }
}
