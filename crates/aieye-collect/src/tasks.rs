//! Reads the operating system's scheduled tasks. On Windows that is Task Scheduler,
//! read through its COM interface; other systems report none yet.

#[cfg(windows)]
mod taskschd;

use std::path::Path;

use aieye_model::{Evidence, Fact, Record, TaskAction};
use anyhow::Result;
use glob::Pattern;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use crate::{Collector, Context, VarLookup, match_options};

/// Reports every scheduled task outside Windows' own folders (`\Microsoft\` and
/// `\SoftLanding\`), linked to an AI tool when a definition names what it runs. Every task counts, linked or
/// not: a task an AI agent creates to run `powershell.exe` matches no definition,
/// but it is exactly what Watch should notice. A task the signed-in user cannot read
/// is logged and skipped.
///
/// A task's arguments are read only to link it to a tool, and never kept: they can
/// carry tokens.
pub struct ScheduledTasks;

impl Collector for ScheduledTasks {
    fn source(&self) -> &'static str {
        "os:task-scheduler"
    }

    fn collect(&self, ctx: &Context) -> Result<Vec<Record>> {
        let observed_at = Timestamp::now();
        let mut tasks = read_tasks()?;
        tasks.sort_by(|a, b| a.path.cmp(&b.path));
        let links = Links::new(ctx);
        Ok(tasks
            .into_iter()
            .map(|task| {
                let fact = task.into_fact(&links);
                ctx.record(self.source(), Evidence::Verified, observed_at, fact)
            })
            .collect())
    }
}

#[cfg(windows)]
fn read_tasks() -> Result<Vec<RawTask>> {
    taskschd::read()
}

#[cfg(not(windows))]
fn read_tasks() -> Result<Vec<RawTask>> {
    Ok(Vec::new())
}

/// A task as the scheduler reports it.
#[cfg_attr(not(windows), allow(dead_code))]
struct RawTask {
    path: String,
    enabled: bool,
    author: Option<String>,
    run_as: Option<String>,
    actions: Vec<RawAction>,
    triggers: Vec<Trigger>,
    last_run: Option<Timestamp>,
    next_run: Option<Timestamp>,
}

/// One action of a task. `arguments` stay in memory only.
#[cfg_attr(not(windows), allow(dead_code))]
struct RawAction {
    program: String,
    arguments: Option<String>,
    folder: Option<String>,
}

impl RawTask {
    fn into_fact(self, links: &Links) -> Fact {
        let tool = self
            .actions
            .iter()
            .find_map(|action| links.tool(action))
            .map(str::to_owned);
        Fact::ScheduledTask {
            tool,
            path: self.path,
            enabled: self.enabled,
            author: self.author,
            run_as: self.run_as,
            actions: self
                .actions
                .into_iter()
                .map(|action| TaskAction {
                    has_arguments: action.arguments.is_some(),
                    program: action.program,
                    folder: action.folder,
                })
                .collect(),
            triggers: self.triggers.iter().map(Trigger::describe).collect(),
            last_run: self.last_run,
            next_run: self.next_run,
        }
    }
}

/// Links tasks to AI tools using only definitions. A task belongs to a tool when an
/// action's program is one of the tool's `detect.executables`, or when its program,
/// working folder or a path in its arguments lies inside one of the tool's
/// `detect.paths`, such as a script under `~/.codex`.
struct Links<'a> {
    executables: Vec<(&'a str, Pattern)>,
    paths: Vec<(&'a str, Pattern)>,
    var: &'a VarLookup,
}

impl<'a> Links<'a> {
    fn new(ctx: &'a Context) -> Self {
        // Patterns were validated when their definitions were loaded.
        let compile = |patterns: Vec<(&'a str, String)>| -> Vec<(&'a str, Pattern)> {
            patterns
                .into_iter()
                .filter_map(|(tool, pattern)| Some((tool, Pattern::new(&pattern).ok()?)))
                .collect()
        };
        Self {
            executables: compile(ctx.executable_patterns()),
            paths: compile(ctx.path_patterns()),
            var: &ctx.var,
        }
    }

    fn tool(&self, action: &RawAction) -> Option<&'a str> {
        let program = expand_vars(&action.program, self.var);
        let executable = self
            .executables
            .iter()
            .find(|(_, pattern)| pattern.matches_path_with(Path::new(&program), match_options()));
        if let Some((tool, _)) = executable {
            return Some(tool);
        }
        let mut candidates = vec![program];
        candidates.extend(
            action
                .folder
                .iter()
                .map(|folder| expand_vars(folder, self.var)),
        );
        if let Some(arguments) = &action.arguments {
            candidates.extend(path_words(&expand_vars(arguments, self.var)));
        }
        candidates
            .iter()
            .find_map(|candidate| self.inside_path(candidate))
    }

    /// The tool whose `detect.paths` contain `candidate`, or are it.
    fn inside_path(&self, candidate: &str) -> Option<&'a str> {
        Path::new(candidate).ancestors().find_map(|ancestor| {
            self.paths
                .iter()
                .find(|(_, pattern)| pattern.matches_path_with(ancestor, match_options()))
                .map(|(tool, _)| *tool)
        })
    }
}

/// Replaces `%NAME%` with the variable's value. Unknown names stay as they are.
fn expand_vars(text: &str, var: &VarLookup) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match var(name) {
                    Some(value) => out.push_str(&value),
                    None => out.push_str(&format!("%{name}%")),
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The words of a command line that look like paths. Words split at spaces outside
/// double quotes, and at `=`, so `--config=C:\x` yields `C:\x`.
fn path_words(arguments: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    for c in arguments.chars().chain([' ']) {
        match c {
            '"' => quoted = !quoted,
            c if (c.is_whitespace() && !quoted) || c == '=' => {
                if word.contains(['\\', '/']) {
                    words.push(word.clone());
                }
                word.clear();
            }
            c => word.push(c),
        }
    }
    words
}

/// A program or folder as the scheduler stores it, without surrounding quotes.
#[cfg_attr(not(windows), allow(dead_code))]
fn unquote(text: &str) -> &str {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(text)
}

/// Task Scheduler folders that hold Windows' own tasks. `\SoftLanding` holds the
/// tasks of Windows' tips, which reschedule themselves several times a day and would
/// otherwise raise an alert on every scan.
const WINDOWS_FOLDERS: [&str; 2] = ["\\microsoft", "\\softlanding"];

/// Whether a Task Scheduler folder is one of Windows' own, or inside one.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_windows_own(folder: &str) -> bool {
    let folder = folder.to_ascii_lowercase();
    WINDOWS_FOLDERS.iter().any(|own| {
        folder
            .strip_prefix(own)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('\\'))
    })
}

/// A Task Scheduler time: an OLE Automation date, days since 1899-12-30, in local
/// time. The scheduler reports "never" as a date before 2000, so those are none.
#[cfg_attr(not(windows), allow(dead_code))]
fn ole_time(days: f64, zone: &TimeZone) -> Option<Timestamp> {
    if !(0.0..=3_000_000.0).contains(&days) {
        return None;
    }
    // Whole seconds; the cast is safe because the range is checked above.
    let seconds = (days * 86_400.0).round() as i64;
    let local = jiff::civil::date(1899, 12, 30)
        .at(0, 0, 0, 0)
        .checked_add(SignedDuration::from_secs(seconds))
        .ok()?;
    if local.year() < 2000 {
        return None;
    }
    Some(local.to_zoned(zone.clone()).ok()?.timestamp())
}

/// What starts a task.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerKind {
    Event,
    Once,
    Daily,
    Weekly,
    Monthly,
    MonthlyByWeekday,
    Idle,
    Registration,
    Boot,
    Logon,
    SessionChange,
    Other,
}

/// One trigger of a task, as the scheduler reports it.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug)]
struct Trigger {
    kind: TriggerKind,
    /// When the trigger starts, as the scheduler writes it, such as
    /// `2026-10-26T07:38:00`, maybe with an offset.
    start: Option<String>,
    /// Daily: every how many days. Weekly: every how many weeks.
    interval: Option<i16>,
    /// Weekly: a bit per weekday, Sunday first. Monthly: a bit per day of the month,
    /// then one for the last day.
    days: Option<i32>,
    /// How often it repeats once started: an ISO 8601 duration such as `PT1H`.
    repeat: Option<String>,
    enabled: bool,
}

impl Trigger {
    /// The trigger in words, such as "weekly on Mon, Fri at 07:38".
    fn describe(&self) -> String {
        let at = self
            .start
            .as_deref()
            .and_then(clock)
            .map(|time| format!(" at {time}"))
            .unwrap_or_default();
        let every = |unit: &str| match self.interval {
            Some(n) if n > 1 => format!("every {n} {unit}s"),
            _ => String::new(),
        };
        let mut text = match self.kind {
            TriggerKind::Once => match self.start.as_deref().and_then(day) {
                Some(day) => format!("once on {day}{at}"),
                None => "once".to_owned(),
            },
            TriggerKind::Daily => match every("day") {
                every if every.is_empty() => format!("daily{at}"),
                every => format!("{every}{at}"),
            },
            TriggerKind::Weekly => {
                let days = weekdays(self.days.unwrap_or(0));
                let on = if days.is_empty() {
                    String::new()
                } else {
                    format!(" on {days}")
                };
                match every("week") {
                    every if every.is_empty() => format!("weekly{on}{at}"),
                    every => format!("{every}{on}{at}"),
                }
            }
            TriggerKind::Monthly => {
                let days = month_days(self.days.unwrap_or(0));
                if days.is_empty() {
                    format!("monthly{at}")
                } else {
                    format!("monthly on day {days}{at}")
                }
            }
            TriggerKind::MonthlyByWeekday => format!("monthly on set weekdays{at}"),
            TriggerKind::Idle => "when the computer is idle".to_owned(),
            TriggerKind::Registration => "when the task is registered".to_owned(),
            TriggerKind::Boot => "at startup".to_owned(),
            TriggerKind::Logon => "at logon".to_owned(),
            TriggerKind::SessionChange => "on lock, unlock or remote connection".to_owned(),
            TriggerKind::Event => "on a system event".to_owned(),
            TriggerKind::Other => "on a custom trigger".to_owned(),
        };
        if let Some(repeat) = &self.repeat {
            let every = duration(repeat).unwrap_or_else(|| repeat.clone());
            text.push_str(&format!(", then every {every}"));
        }
        if !self.enabled {
            text.push_str(" (trigger off)");
        }
        text
    }
}

/// `07:38` from `2026-10-26T07:38:00`.
fn clock(start: &str) -> Option<&str> {
    (start.as_bytes().get(10) == Some(&b'T')).then(|| start.get(11..16))?
}

/// `2026-10-26` from `2026-10-26T07:38:00`.
fn day(start: &str) -> Option<&str> {
    (start.as_bytes().get(10) == Some(&b'T')).then(|| start.get(..10))?
}

/// Weekdays from a bit mask with Sunday as the lowest bit, listed from Monday.
fn weekdays(bits: i32) -> String {
    let names = [
        (2, "Mon"),
        (4, "Tue"),
        (8, "Wed"),
        (16, "Thu"),
        (32, "Fri"),
        (64, "Sat"),
        (1, "Sun"),
    ];
    let days: Vec<&str> = names
        .iter()
        .filter(|(bit, _)| bits & bit != 0)
        .map(|(_, name)| *name)
        .collect();
    days.join(", ")
}

/// Days of the month from a bit mask: day 1 is the lowest bit, and the top bit means
/// the last day.
fn month_days(bits: i32) -> String {
    let mut days: Vec<String> = (0..31)
        .filter(|day| bits & (1 << day) != 0)
        .map(|day| (day + 1).to_string())
        .collect();
    if bits < 0 {
        days.push("last".to_owned());
    }
    days.join(", ")
}

/// An ISO 8601 duration in words: `PT1H30M` is "1 h 30 min".
fn duration(iso: &str) -> Option<String> {
    let mut parts = Vec::new();
    let mut number = String::new();
    let mut time = false;
    for c in iso.strip_prefix('P')?.chars() {
        match c {
            'T' => time = true,
            '0'..='9' => number.push(c),
            unit => {
                let n: u32 = number.parse().ok()?;
                number.clear();
                let unit = match (unit, time, n) {
                    ('D', false, 1) => "day",
                    ('D', false, _) => "days",
                    ('W', false, 1) => "week",
                    ('W', false, _) => "weeks",
                    ('H', true, _) => "h",
                    ('M', true, _) => "min",
                    ('S', true, _) => "s",
                    _ => return None,
                };
                parts.push(format!("{n} {unit}"));
            }
        }
    }
    (!parts.is_empty() && number.is_empty()).then(|| parts.join(" "))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::tests::context_with;

    const HOME: &str = if cfg!(windows) {
        r"C:\Users\me"
    } else {
        "/home/me"
    };

    fn trigger(kind: TriggerKind) -> Trigger {
        Trigger {
            kind,
            start: Some("2026-10-26T07:38:00".into()),
            interval: None,
            days: None,
            repeat: None,
            enabled: true,
        }
    }

    #[test]
    fn describes_triggers_in_words() {
        let weekly = Trigger {
            days: Some(2 | 32),
            interval: Some(2),
            ..trigger(TriggerKind::Weekly)
        };
        let monthly = Trigger {
            days: Some(1 | (1 << 14) | i32::MIN),
            ..trigger(TriggerKind::Monthly)
        };
        let hourly = Trigger {
            repeat: Some("PT1H30M".into()),
            enabled: false,
            ..trigger(TriggerKind::Daily)
        };
        let described: Vec<String> = [
            trigger(TriggerKind::Daily),
            Trigger {
                interval: Some(3),
                ..trigger(TriggerKind::Daily)
            },
            weekly,
            monthly,
            trigger(TriggerKind::Once),
            hourly,
            trigger(TriggerKind::Logon),
        ]
        .iter()
        .map(Trigger::describe)
        .collect();
        assert_eq!(
            described,
            [
                "daily at 07:38",
                "every 3 days at 07:38",
                "every 2 weeks on Mon, Fri at 07:38",
                "monthly on day 1, 15, last at 07:38",
                "once on 2026-10-26 at 07:38",
                "daily at 07:38, then every 1 h 30 min (trigger off)",
                "at logon",
            ]
        );
    }

    #[test]
    fn reads_durations() {
        assert_eq!(duration("PT1H").as_deref(), Some("1 h"));
        assert_eq!(duration("P1D").as_deref(), Some("1 day"));
        assert_eq!(duration("P2DT30M").as_deref(), Some("2 days 30 min"));
        assert_eq!(duration("PT"), None);
        assert_eq!(duration("1H"), None);
    }

    #[test]
    fn converts_scheduler_times() {
        let utc = TimeZone::UTC;
        assert_eq!(
            ole_time(45_000.5, &utc),
            Some("2023-03-15T12:00:00Z".parse().unwrap())
        );
        assert_eq!(ole_time(0.0, &utc), None);
        // 1999-11-30, which Task Scheduler shows for a task that never ran.
        assert_eq!(ole_time(36_494.0, &utc), None);
        assert_eq!(ole_time(f64::NAN, &utc), None);
    }

    #[test]
    fn finds_paths_in_arguments_and_expands_variables() {
        assert_eq!(
            path_words(r#"-File "C:\My Scripts\run.ps1" --config=C:\x\c.json -NoLogo"#),
            [r"C:\My Scripts\run.ps1", r"C:\x\c.json"]
        );
        let var: VarLookup = Box::new(|name| (name == "HOME").then(|| "/home/me".to_owned()));
        assert_eq!(expand_vars("%HOME%/x", &var), "/home/me/x");
        assert_eq!(expand_vars("%NOPE%/x and 50%", &var), "%NOPE%/x and 50%");
    }

    #[test]
    fn leaves_out_windows_own_folders() {
        assert!(is_windows_own(r"\Microsoft"));
        assert!(is_windows_own(r"\Microsoft\Windows\Defrag"));
        assert!(!is_windows_own(r"\MicrosoftEdgeUpdate"));
        assert!(is_windows_own(r"\SoftLanding\S-1-5-21-1\SoftLandingTask"));
        assert!(!is_windows_own(r"\"));
        assert_eq!(unquote(r#" "C:\x\y.exe" "#), r"C:\x\y.exe");
    }

    fn task(program: PathBuf, arguments: Option<String>) -> RawTask {
        RawTask {
            path: r"\Job".into(),
            enabled: true,
            author: None,
            run_as: None,
            actions: vec![RawAction {
                program: program.display().to_string(),
                arguments,
                folder: None,
            }],
            triggers: vec![trigger(TriggerKind::Daily)],
            last_run: None,
            next_run: None,
        }
    }

    fn tool_of(fact: &Fact) -> Option<&str> {
        match fact {
            Fact::ScheduledTask { tool, .. } => tool.as_deref(),
            other => panic!("unexpected fact {other:?}"),
        }
    }

    #[test]
    fn links_tasks_to_tools_through_definitions_and_keeps_no_arguments() {
        let home = Path::new(HOME);
        let ctx = context_with(
            home,
            &format!(
                "[detect.{}]\nexecutables = [\"~/apps/tool/tool.exe\"]\npaths = [\"~/.tool\"]\n",
                crate::tests::platform()
            ),
        );
        let links = Links::new(&ctx);
        let script = home.join(".tool").join("jobs").join("digest.js");

        let by_program = task(home.join("apps").join("tool").join("tool.exe"), None);
        let by_argument = task(
            PathBuf::from("node.exe"),
            Some(format!("\"{}\" --token=secret-token", script.display())),
        );
        let unrelated = task(
            PathBuf::from("powershell.exe"),
            Some("-File secret-script.ps1".into()),
        );

        let facts: Vec<Fact> = [by_program, by_argument, unrelated]
            .into_iter()
            .map(|task| task.into_fact(&links))
            .collect();

        let tools: Vec<Option<&str>> = facts.iter().map(tool_of).collect();
        assert_eq!(tools, [Some("test-tool"), Some("test-tool"), None]);
        let json = serde_json::to_string(&facts).unwrap();
        assert!(!json.contains("secret"), "arguments leaked: {json}");
        assert!(json.contains("\"has_arguments\":true"));
        assert!(json.contains("daily at 07:38"));
    }
}
