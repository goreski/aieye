//! Plain-language wording for changes, shared by the command line and the app.

use crate::{Change, ChangeKind, Fact, TaskAction};

impl Change {
    /// What happened, such as "folder added", and to what.
    pub fn describe(&self) -> (&'static str, String) {
        let fact = self.fact();
        match (self.kind, fact) {
            (ChangeKind::Added, Fact::ToolPresent { path, .. }) => {
                ("found", path.display().to_string())
            }
            (_, Fact::ToolPresent { path, .. }) => ("gone", path.display().to_string()),
            (ChangeKind::Removed, Fact::ToolRunning { exe, .. }) => {
                ("stopped running", exe.display().to_string())
            }
            (_, Fact::ToolRunning { exe, .. }) => ("running from", exe.display().to_string()),
            (ChangeKind::Added, Fact::ConfiguredFolder { path, trust, .. }) => {
                ("folder added", with_trust(path, trust.as_deref()))
            }
            (ChangeKind::Removed, Fact::ConfiguredFolder { path, trust, .. }) => {
                ("folder removed", with_trust(path, trust.as_deref()))
            }
            (ChangeKind::Changed, Fact::ConfiguredFolder { path, .. }) => (
                "folder changed",
                format!("{path}: {}", self.field_changes()),
            ),
            (ChangeKind::Added, Fact::McpServer { .. }) => ("MCP server added", server(fact)),
            (ChangeKind::Removed, Fact::McpServer { .. }) => ("MCP server removed", server(fact)),
            (ChangeKind::Changed, Fact::McpServer { name, .. }) => (
                "MCP server changed",
                format!("{name}: {}", self.field_changes()),
            ),
            (ChangeKind::Added, Fact::ScheduledTask { .. }) => ("scheduled task added", task(fact)),
            (ChangeKind::Removed, Fact::ScheduledTask { .. }) => {
                ("scheduled task removed", task(fact))
            }
            (ChangeKind::Changed, Fact::ScheduledTask { path, .. }) => (
                "scheduled task changed",
                format!("{path}: {}", self.field_changes()),
            ),
            (ChangeKind::Added, Fact::ToolSchedule { .. }) => ("schedule added", schedule(fact)),
            (ChangeKind::Removed, Fact::ToolSchedule { .. }) => {
                ("schedule removed", schedule(fact))
            }
            (ChangeKind::Changed, Fact::ToolSchedule { name, .. }) => (
                "schedule changed",
                format!("{name}: {}", self.field_changes()),
            ),
        }
    }

    /// The fields that differ between a changed fact's two versions.
    fn field_changes(&self) -> String {
        let (Some(before), Some(after)) = (&self.before, &self.after) else {
            return String::new();
        };
        let mut parts = Vec::new();
        match (&before.fact, &after.fact) {
            (
                Fact::ConfiguredFolder {
                    trust: old_trust,
                    boundary: old_boundary,
                    ..
                },
                Fact::ConfiguredFolder {
                    trust, boundary, ..
                },
            ) => {
                if old_trust != trust {
                    parts.push(format!("trust {} → {}", or_none(old_trust), or_none(trust)));
                }
                if old_boundary != boundary {
                    parts.push(format!("boundary {old_boundary} → {boundary}"));
                }
            }
            (
                Fact::McpServer {
                    command: old_command,
                    origin: old_origin,
                    env_names: old_env,
                    ..
                },
                Fact::McpServer {
                    command,
                    origin,
                    env_names,
                    ..
                },
            ) => {
                if old_command != command {
                    parts.push(format!(
                        "command {} → {}",
                        or_none(old_command),
                        or_none(command)
                    ));
                }
                if old_origin != origin {
                    parts.push(format!("URL {} → {}", or_none(old_origin), or_none(origin)));
                }
                if old_env != env_names {
                    parts.push(format!("env {} → {}", names(old_env), names(env_names)));
                }
            }
            (
                Fact::ScheduledTask {
                    tool: old_tool,
                    enabled: old_enabled,
                    author: old_author,
                    run_as: old_run_as,
                    actions: old_actions,
                    triggers: old_triggers,
                    ..
                },
                Fact::ScheduledTask {
                    tool,
                    enabled,
                    author,
                    run_as,
                    actions,
                    triggers,
                    ..
                },
            ) => {
                if old_enabled != enabled {
                    parts.push(format!("{} → {}", on_off(*old_enabled), on_off(*enabled)));
                }
                if old_actions != actions {
                    parts.push(format!("runs {} → {}", runs(old_actions), runs(actions)));
                }
                if old_triggers != triggers {
                    parts.push(format!("{} → {}", when(old_triggers), when(triggers)));
                }
                if old_run_as != run_as {
                    parts.push(format!(
                        "runs as {} → {}",
                        or_none(old_run_as),
                        or_none(run_as)
                    ));
                }
                if old_author != author {
                    parts.push(format!(
                        "author {} → {}",
                        or_none(old_author),
                        or_none(author)
                    ));
                }
                if old_tool != tool {
                    parts.push(format!("AI tool {} → {}", or_none(old_tool), or_none(tool)));
                }
            }
            (
                Fact::ToolSchedule {
                    schedule: old_schedule,
                    folder: old_folder,
                    enabled: old_enabled,
                    ..
                },
                Fact::ToolSchedule {
                    schedule,
                    folder,
                    enabled,
                    ..
                },
            ) => {
                if old_enabled != enabled {
                    parts.push(format!(
                        "{} → {}",
                        maybe_on_off(*old_enabled),
                        maybe_on_off(*enabled)
                    ));
                }
                if old_schedule != schedule {
                    parts.push(format!(
                        "schedule {} → {}",
                        or_none(old_schedule),
                        or_none(schedule)
                    ));
                }
                if old_folder != folder {
                    parts.push(format!(
                        "folder {} → {}",
                        or_none(old_folder),
                        or_none(folder)
                    ));
                }
            }
            _ => {}
        }
        parts.join("; ")
    }
}

/// A scheduled task in one line: path, programs and triggers.
fn task(fact: &Fact) -> String {
    let Fact::ScheduledTask {
        path,
        enabled,
        actions,
        triggers,
        ..
    } = fact
    else {
        return String::new();
    };
    let mut text = format!("{path}: {} ({})", runs(actions), when(triggers));
    if !enabled {
        text.push_str(", off");
    }
    text
}

/// A tool's schedule in one line: name, schedule and folder.
fn schedule(fact: &Fact) -> String {
    let Fact::ToolSchedule {
        name,
        schedule,
        folder,
        enabled,
        ..
    } = fact
    else {
        return String::new();
    };
    let mut text = format!("{name} ({}", schedule.as_deref().unwrap_or("no schedule"));
    if let Some(folder) = folder {
        text.push_str(&format!(" in {folder}"));
    }
    text.push(')');
    if *enabled == Some(false) {
        text.push_str(", off");
    }
    text
}

fn runs(actions: &[TaskAction]) -> String {
    if actions.is_empty() {
        "nothing".to_owned()
    } else {
        let programs: Vec<&str> = actions
            .iter()
            .map(|action| action.program.as_str())
            .collect();
        programs.join(", ")
    }
}

fn when(triggers: &[String]) -> String {
    if triggers.is_empty() {
        "no triggers".to_owned()
    } else {
        triggers.join("; ")
    }
}

fn on_off(enabled: bool) -> &'static str {
    if enabled { "on" } else { "off" }
}

fn maybe_on_off(enabled: Option<bool>) -> &'static str {
    enabled.map_or("unknown", on_off)
}

fn with_trust(path: &str, trust: Option<&str>) -> String {
    match trust {
        Some(trust) => format!("{path} ({trust})"),
        None => path.to_owned(),
    }
}

fn server(fact: &Fact) -> String {
    let Fact::McpServer {
        scope,
        name,
        command,
        origin,
        env_names,
        ..
    } = fact
    else {
        return String::new();
    };
    let target = command
        .as_deref()
        .or(origin.as_deref())
        .unwrap_or("no command or URL");
    let mut text = format!("{name} ({target}");
    if !env_names.is_empty() {
        text.push_str(&format!("; env {}", env_names.join(", ")));
    }
    text.push(')');
    if let Some(scope) = scope {
        text.push_str(&format!(" in {scope}"));
    }
    text
}

fn or_none(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("none")
}

fn names(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}
