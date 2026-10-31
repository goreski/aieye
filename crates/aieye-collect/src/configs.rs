//! Reads the folders, MCP servers and schedules that AI tools' own config files
//! declare.
//!
//! Config files hold secrets, such as API keys in MCP server settings, and private
//! text, such as the prompts of scheduled tasks. This collector keeps only what the
//! facts allow: folder paths, trust values, server names and commands, URL origins,
//! environment variable names, and task names, schedules, folders and on/off states.
//! The rest of a file is dropped once the scan is done with it, and error messages
//! never quote it.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use aieye_defs::{PathFrom, Segment, Selector};
use aieye_model::{Evidence, Fact, Record};
use anyhow::{Result, anyhow, ensure};
use jiff::Timestamp;
use serde_json::{Map, Value};

use crate::{Collector, Context};

/// Config files larger than this are skipped.
const MAX_FILE_SIZE: u64 = 32 * 1024 * 1024;

/// Reports the folders, MCP servers and schedules in the config files that
/// definitions name. A tool's own config is a claim, so every fact is `declared`.
///
/// A file that cannot be read or parsed is logged and skipped, so one broken config
/// does not hide the rest of the scan.
pub struct Configs;

impl Collector for Configs {
    fn source(&self) -> &'static str {
        "config"
    }

    fn collect(&self, ctx: &Context) -> Result<Vec<Record>> {
        let observed_at = Timestamp::now();
        let mut documents = Documents::default();
        let mut facts = Vec::new();
        for definition in &ctx.definitions {
            let tool = definition.id.as_str();
            for source in definition.folders.get(&ctx.platform).into_iter().flatten() {
                for file in ctx.existing(tool, &source.file) {
                    let Some(document) = documents.get(&file) else {
                        continue;
                    };
                    for entry in select(document, &source.select) {
                        let path = match source.path {
                            PathFrom::Key => entry.keys.last().cloned(),
                            PathFrom::Value => entry.value.as_str().map(str::to_owned),
                        };
                        let Some(path) = path else {
                            continue;
                        };
                        let trust = source
                            .trust
                            .as_ref()
                            .and_then(|field| entry.value.get(field.as_str()))
                            .and_then(scalar_text);
                        facts.push(Fact::ConfiguredFolder {
                            tool: tool.to_owned(),
                            file: file.clone(),
                            path,
                            trust,
                            boundary: source.boundary,
                        });
                    }
                }
            }
            for source in definition.mcp.get(&ctx.platform).into_iter().flatten() {
                for file in ctx.existing(tool, &source.file) {
                    let Some(document) = documents.get(&file) else {
                        continue;
                    };
                    for entry in select(document, &source.select) {
                        let (Some((name, scope)), Value::Object(server)) =
                            (entry.keys.split_last(), entry.value)
                        else {
                            continue;
                        };
                        facts.push(Fact::McpServer {
                            tool: tool.to_owned(),
                            file: file.clone(),
                            scope: (!scope.is_empty()).then(|| scope.join("/")),
                            name: name.clone(),
                            command: server
                                .get(&source.command)
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            origin: server
                                .get(&source.url)
                                .and_then(Value::as_str)
                                .and_then(origin),
                            env_names: env_names(server, &source.env),
                        });
                    }
                }
            }
            for source in definition
                .schedules
                .get(&ctx.platform)
                .into_iter()
                .flatten()
            {
                for file in ctx.existing(tool, &source.file) {
                    let Some(document) = documents.get(&file) else {
                        continue;
                    };
                    for entry in select(document, &source.select) {
                        if !entry.value.is_object() {
                            continue;
                        }
                        let name = match &source.name {
                            Some(name) => field(entry.value, name).and_then(scalar_text),
                            None => entry.keys.last().cloned(),
                        };
                        let Some(name) = name else {
                            continue;
                        };
                        facts.push(Fact::ToolSchedule {
                            tool: tool.to_owned(),
                            file: file.clone(),
                            name,
                            schedule: source
                                .schedule
                                .iter()
                                .find_map(|path| field(entry.value, path).and_then(scalar_text))
                                .or_else(|| {
                                    let path = source.at.as_ref()?;
                                    field(entry.value, path).and_then(one_time)
                                }),
                            folder: source
                                .folder
                                .as_ref()
                                .and_then(|path| field(entry.value, path))
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            enabled: source
                                .enabled
                                .as_ref()
                                .and_then(|path| field(entry.value, path))
                                .and_then(Value::as_bool),
                        });
                    }
                }
            }
        }
        Ok(facts
            .into_iter()
            .map(|fact| ctx.record(self.source(), Evidence::Declared, observed_at, fact))
            .collect())
    }
}

/// Parsed config files, so a file that several entries name is read once per scan.
#[derive(Default)]
struct Documents(HashMap<PathBuf, Option<Value>>);

impl Documents {
    fn get(&mut self, file: &Path) -> Option<&Value> {
        self.0
            .entry(file.to_owned())
            .or_insert_with(|| match read(file) {
                Ok(document) => Some(document),
                Err(error) => {
                    tracing::warn!(file = %file.display(), "skipping config: {error:#}");
                    None
                }
            })
            .as_ref()
    }
}

fn read(file: &Path) -> Result<Value> {
    let size = fs::metadata(file)?.len();
    ensure!(size <= MAX_FILE_SIZE, "larger than {MAX_FILE_SIZE} bytes");
    let text = fs::read_to_string(file)?;
    parse(file, &text)
}

/// Parses a JSON or TOML config. Errors give only a line number, never the text.
/// An empty file has no entries; tools create some configs empty.
fn parse(file: &Path, text: &str) -> Result<Value> {
    let text = text.trim_start_matches('\u{feff}');
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    if file
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
    {
        toml::from_str(text).map_err(|error| {
            let line = error
                .span()
                .and_then(|span| text.get(..span.start))
                .map_or(0, |before| before.matches('\n').count() + 1);
            anyhow!("invalid TOML at line {line}")
        })
    } else {
        // Some tools write whole-line `//` comments into their JSON. A JSON string
        // cannot span lines, so such a line is never inside a value. Blank lines keep
        // the line numbers right.
        let text: Vec<&str> = text
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("//") {
                    ""
                } else {
                    line
                }
            })
            .collect();
        serde_json::from_str(&text.join("\n"))
            .map_err(|error| anyhow!("invalid JSON at line {}", error.line()))
    }
}

/// One entry a selector matched: its value, and the keys the `*` steps matched.
struct Entry<'a> {
    keys: Vec<String>,
    value: &'a Value,
}

fn select<'a>(document: &'a Value, selector: &Selector) -> Vec<Entry<'a>> {
    let mut entries = vec![Entry {
        keys: Vec::new(),
        value: document,
    }];
    for segment in selector.segments() {
        entries = entries
            .into_iter()
            .flat_map(|entry| step(entry, segment))
            .collect();
    }
    entries
}

fn step<'a>(entry: Entry<'a>, segment: &Segment) -> Vec<Entry<'a>> {
    let child = |key: Option<String>, value: &'a Value| {
        let mut keys = entry.keys.clone();
        keys.extend(key);
        Entry { keys, value }
    };
    match (segment, entry.value) {
        (Segment::Key(key), Value::Object(map)) => map
            .get(key)
            .map(|value| child(None, value))
            .into_iter()
            .collect(),
        (Segment::Any, Value::Object(map)) => map
            .iter()
            .map(|(key, value)| child(Some(key.clone()), value))
            .collect(),
        (Segment::Any, Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, value)| child(Some(index.to_string()), value))
            .collect(),
        _ => Vec::new(),
    }
}

/// The value at a field of an entry: a key, or a dotted path such as `trigger.cron`.
fn field<'a>(entry: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(entry, |value, key| value.as_object()?.get(key))
}

/// A one-time run as a schedule, such as "once at 2027-06-01T07:00:00Z". The config
/// gives the time as Unix milliseconds or as an ISO 8601 date and time with an
/// offset; text that is neither is kept as written.
fn one_time(value: &Value) -> Option<String> {
    let time = match value {
        Value::Number(number) => Timestamp::from_millisecond(number.as_i64()?).ok()?,
        Value::String(text) => match text.parse::<Timestamp>() {
            Ok(time) => time,
            Err(_) => return Some(format!("once at {text}")),
        },
        _ => return None,
    };
    Some(format!("once at {time}"))
}

/// A trust value as text: strings as they are, booleans and numbers written out.
fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Bool(_) | Value::Number(_) => Some(value.to_string()),
        _ => None,
    }
}

/// The names of a server's environment variables, never their values.
fn env_names(server: &Map<String, Value>, fields: &[String]) -> Vec<String> {
    let mut names = BTreeSet::new();
    for field in fields {
        match server.get(field) {
            Some(Value::Object(variables)) => names.extend(variables.keys().cloned()),
            Some(Value::Array(items)) => names.extend(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(|item| item.split('=').next())
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned),
            ),
            _ => {}
        }
    }
    names.into_iter().collect()
}

/// The scheme, host and port of a URL. Credentials, path, query and fragment are
/// dropped, because remote MCP servers often carry tokens in them.
fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    (!scheme.is_empty() && !host.is_empty()).then(|| format!("{scheme}://{host}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{context_with, platform};

    fn facts(records: &[Record]) -> Vec<&Fact> {
        records.iter().map(|record| &record.fact).collect()
    }

    fn assert_no_secrets(records: &[Record]) {
        let json = serde_json::to_string(records).unwrap();
        assert!(!json.contains("secret"), "a secret leaked: {json}");
    }

    #[test]
    fn reads_folders_and_servers_from_toml() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join("config.toml");
        fs::write(
            &file,
            r#"
model = "x"

[projects.'C:\work\one']
trust_level = "trusted"

[projects.'C:\work\two']
trust_level = "untrusted"

[mcp_servers.files]
command = "files-server"
args = ["--token", "secret-arg"]
env = { FILES_TOKEN = "secret-value" }
env_vars = ["HOME_DIR", "LANG=secret-lang"]
"#,
        )
        .unwrap();
        let p = platform();
        let ctx = context_with(
            home.path(),
            &format!(
                r#"
[[folders.{p}]]
file = "~/config.toml"
select = "projects.*"
trust = "trust_level"
boundary = false

[[mcp.{p}]]
file = "~/config.toml"
select = "mcp_servers.*"
env = ["env", "env_vars"]
"#
            ),
        );

        let records = Configs.collect(&ctx).unwrap();

        let folder = |path: &str, trust: &str| Fact::ConfiguredFolder {
            tool: "test-tool".into(),
            file: file.clone(),
            path: path.into(),
            trust: Some(trust.into()),
            boundary: false,
        };
        assert_eq!(
            facts(&records),
            [
                &folder(r"C:\work\one", "trusted"),
                &folder(r"C:\work\two", "untrusted"),
                &Fact::McpServer {
                    tool: "test-tool".into(),
                    file: file.clone(),
                    scope: None,
                    name: "files".into(),
                    command: Some("files-server".into()),
                    origin: None,
                    env_names: vec!["FILES_TOKEN".into(), "HOME_DIR".into(), "LANG".into()],
                },
            ]
        );
        assert!(records.iter().all(|record| {
            record.evidence == Evidence::Declared && record.source.as_str() == "config"
        }));
        assert_no_secrets(&records);
    }

    #[test]
    fn reads_nested_servers_from_json_and_keeps_only_url_origins() {
        let home = tempfile::tempdir().unwrap();
        let file = home.path().join("tool.json");
        fs::write(
            &file,
            r#"// Written by the tool.
{
  "projects": {
    "C:/work/one": {
      "hasTrustDialogAccepted": true,
      "mcpServers": {
        "remote": {
          "type": "http",
          "url": "https://user:secret-pass@mcp.example.com:8443/secret-path/mcp?key=secret-query#secret-fragment",
          "headers": { "Authorization": "Bearer secret-header" }
        }
      }
    }
  },
  "mcpServers": {
    "local": { "command": "npx", "args": ["secret-arg"], "env": { "API_KEY": "secret-value" } }
  }
}"#,
        )
        .unwrap();
        let p = platform();
        let ctx = context_with(
            home.path(),
            &format!(
                r#"
[[folders.{p}]]
file = "~/tool.json"
select = "projects.*"
trust = "hasTrustDialogAccepted"
boundary = false

[[mcp.{p}]]
file = "~/tool.json"
select = "mcpServers.*"

[[mcp.{p}]]
file = "~/tool.json"
select = "projects.*.mcpServers.*"
"#
            ),
        );

        let records = Configs.collect(&ctx).unwrap();

        let server = |scope: Option<&str>,
                      name: &str,
                      command: Option<&str>,
                      origin: Option<&str>,
                      env: &[&str]| {
            Fact::McpServer {
                tool: "test-tool".into(),
                file: file.clone(),
                scope: scope.map(str::to_owned),
                name: name.into(),
                command: command.map(str::to_owned),
                origin: origin.map(str::to_owned),
                env_names: env.iter().map(|name| (*name).to_owned()).collect(),
            }
        };
        assert_eq!(
            facts(&records),
            [
                &Fact::ConfiguredFolder {
                    tool: "test-tool".into(),
                    file: file.clone(),
                    path: "C:/work/one".into(),
                    trust: Some("true".into()),
                    boundary: false,
                },
                &server(None, "local", Some("npx"), None, &["API_KEY"]),
                &server(
                    Some("C:/work/one"),
                    "remote",
                    None,
                    Some("https://mcp.example.com:8443"),
                    &[]
                ),
            ]
        );
        assert_no_secrets(&records);
    }

    #[test]
    fn reads_folder_lists_and_skips_broken_files() {
        let home = tempfile::tempdir().unwrap();
        fs::write(
            home.path().join("good.json"),
            r#"{ "permissions": { "additionalDirectories": ["D:/data", 7] } }"#,
        )
        .unwrap();
        fs::write(
            home.path().join("broken.json"),
            "{ \"token\": \"secret-value\",",
        )
        .unwrap();
        let p = platform();
        let ctx = context_with(
            home.path(),
            &format!(
                r#"
[[folders.{p}]]
file = "~/good.json"
select = "permissions.additionalDirectories.*"
path = "value"
boundary = true

[[folders.{p}]]
file = "~/broken.json"
select = "projects.*"
boundary = false
"#
            ),
        );

        let records = Configs.collect(&ctx).unwrap();

        assert_eq!(
            facts(&records),
            [&Fact::ConfiguredFolder {
                tool: "test-tool".into(),
                file: home.path().join("good.json"),
                path: "D:/data".into(),
                trust: None,
                boundary: true,
            }]
        );
    }

    #[test]
    fn reads_schedules_without_their_prompts() {
        let home = tempfile::tempdir().unwrap();
        let list = home.path().join("tasks.json");
        fs::write(
            &list,
            r#"{
  "scheduledTasks": [
    { "id": "digest", "cronExpression": "0 7 * * 1-5", "enabled": true,
      "cwd": "C:/work", "prompt": "secret-prompt" },
    { "id": "reminder", "fireAt": 1811833200000, "enabled": false },
    { "id": "call", "fireAt": "2026-11-02T09:00:00+01:00" },
    { "cronExpression": "0 * * * *" },
    "not a task"
  ]
}"#,
        )
        .unwrap();
        let keyed = home.path().join("jobs.toml");
        fs::write(
            &keyed,
            "[jobs.backup]\ntrigger = { cron = \"0 2 * * *\" }\nprompt = \"secret-prompt\"\n",
        )
        .unwrap();
        let p = platform();
        let ctx = context_with(
            home.path(),
            &format!(
                r#"
[[schedules.{p}]]
file = "~/tasks.json"
select = "scheduledTasks.*"
name = "id"
schedule = ["cronExpression"]
at = "fireAt"
folder = "cwd"
enabled = "enabled"

[[schedules.{p}]]
file = "~/jobs.toml"
select = "jobs.*"
schedule = ["trigger.cron"]
"#
            ),
        );

        let records = Configs.collect(&ctx).unwrap();

        let schedule = |file: &PathBuf,
                        name: &str,
                        schedule: &str,
                        folder: Option<&str>,
                        enabled: Option<bool>| Fact::ToolSchedule {
            tool: "test-tool".into(),
            file: file.clone(),
            name: name.into(),
            schedule: Some(schedule.into()),
            folder: folder.map(str::to_owned),
            enabled,
        };
        assert_eq!(
            facts(&records),
            [
                &schedule(&list, "digest", "0 7 * * 1-5", Some("C:/work"), Some(true)),
                &schedule(
                    &list,
                    "reminder",
                    "once at 2027-06-01T07:00:00Z",
                    None,
                    Some(false)
                ),
                &schedule(&list, "call", "once at 2026-11-02T08:00:00Z", None, None),
                &schedule(&keyed, "backup", "0 2 * * *", None, None),
            ]
        );
        assert!(
            records
                .iter()
                .all(|record| record.evidence == Evidence::Declared)
        );
        assert_no_secrets(&records);
    }

    #[test]
    fn parse_errors_do_not_quote_the_file() {
        let errors = [
            parse(Path::new("c.toml"), "token = \"secret-value\"\nbroken = ").unwrap_err(),
            parse(Path::new("c.json"), "{\"token\": \"secret-value\",}").unwrap_err(),
        ];
        for error in errors {
            let message = format!("{error:#}");
            assert!(message.contains("line"), "{message}");
            assert!(!message.contains("secret"), "{message}");
        }
    }

    #[test]
    fn empty_files_have_no_entries() {
        for (name, text) in [("c.json", ""), ("c.toml", "  \n"), ("c.json", "\u{feff}\n")] {
            let document = parse(Path::new(name), text).unwrap();
            let selector = "mcpServers.*".parse().unwrap();
            assert!(select(&document, &selector).is_empty(), "{name}");
        }
    }

    #[test]
    fn origins_drop_credentials_paths_and_queries() {
        assert_eq!(
            origin("https://u:p@host.example:8443/a/b?c=d#e").as_deref(),
            Some("https://host.example:8443")
        );
        assert_eq!(
            origin("http://localhost:3000").as_deref(),
            Some("http://localhost:3000")
        );
        assert_eq!(
            origin("https://host?token=x").as_deref(),
            Some("https://host")
        );
        assert_eq!(origin("not a url"), None);
    }
}
