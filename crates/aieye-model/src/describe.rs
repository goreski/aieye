//! Plain-language wording for changes, shared by the command line and the app.

use crate::{Change, ChangeKind, Fact};

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
            _ => {}
        }
        parts.join("; ")
    }
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
