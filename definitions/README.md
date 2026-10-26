# AI definitions

Definitions tell AIEye how to recognise an AI tool and where to read what it is configured to reach. Like antivirus signatures, they are data files updated independently of the app, so a new AI tool needs a new definition, not a new AIEye release.

One TOML file per tool, named `<id>.toml`. AIEye rejects a file with unknown fields, so a typo fails loudly instead of being ignored.

## Format

| Field | Meaning |
|---|---|
| `id` | Stable identifier, e.g. `codex-cli`: lowercase letters, digits and dashes, matching the file name |
| `name`, `vendor` | Display name and vendor |
| `kind` | `desktop-app`, `cli`, `ide-extension`, `browser-extension` or `mcp-server` |
| `platforms` | Where the tool exists: `windows`, `macos`, `linux` |
| `[detect.<platform>]` | How to recognise the tool on one platform |
| `[[folders.<platform>]]` | Where the tool's own config names folders, such as trusted projects |
| `[[mcp.<platform>]]` | Where the tool's own config lists MCP servers |
| `[verified.<section>.<platform>]` | The `version` and `date` of the check of that section on a real install |

More sections arrive with later slices of v0.1: `schedules`, and later `network`.

### `detect`

| Field | Meaning |
|---|---|
| `executables` | Program files of the tool. A running process started from one of them is reported as the tool running. |
| `paths` | Other files or folders whose existence shows the tool is installed or has been used |

Every entry in either list that exists is reported as the tool being present.

### Path patterns

- Use `/` as the separator on every platform.
- Start with `~/` (the home folder), `%NAME%` (an environment variable such as `%LOCALAPPDATA%`) or an absolute path (`/…` or `C:/…`). Relative paths and `..` are rejected.
- `*` matches any part of one folder or file name, for example `releases/*/bin/codex.exe` for a versioned folder.
- On Windows, matching ignores case.

### `folders` and `mcp`

Both read the tool's own config files, so every fact from them is labelled `declared`. Each section is a list of tables, one per place in a file:

| Field | Section | Meaning |
|---|---|---|
| `file` | both | The config file, a path pattern ending in `.json` or `.toml`. JSON may contain whole-line `//` comments. |
| `select` | both | Where the entries are in the file (see Selectors) |
| `path` | `folders` | `"key"` (default): the folder is the key the last `*` matched, as in `projects.<path>`. `"value"`: the folder is the selected string itself, as in a list. |
| `trust` | `folders` | The field of each entry holding the config's trust value, if any. Booleans and numbers are written out as text. |
| `boundary` | `folders` | Whether the tool restricts its file access to these folders. Trust settings usually do not. |
| `command`, `url` | `mcp` | The fields holding a local server's command and a remote server's URL. Default `command` and `url`. |
| `env` | `mcp` | The fields holding environment variables: an object of names to values, or a list of `NAME` or `NAME=value`. Default `["env"]`. |

AIEye never stores secret values from these files. For an MCP server it keeps the name, the command, the URL's scheme, host and port (credentials, path and query often carry tokens), and the names of environment variables. Arguments, headers and every value are dropped.

### Selectors

A selector is a list of keys separated by `.`, walked from the top of the file. `*` stands for each key of an object or each item of a list. `mcp` selectors end in `*`, whose key names each server; earlier `*` keys become the server's scope.

| Selector | Selects |
|---|---|
| `projects.*` | Each entry under `projects`, keyed by folder |
| `mcp_servers.*` | Each MCP server under `mcp_servers`, keyed by name |
| `projects.*.mcpServers.*` | Each MCP server of each project; the project is the scope |
| `permissions.additionalDirectories.*` | Each item of a list (use `path = "value"`) |

### `verified`

A section counts as verified for a platform only when every entry in it was checked on a real install. Record the tool version and the date as a plain TOML date:

```toml
[verified.detect.windows]
version = "0.164.0"
date = 2026-10-26
```

Leave the table out until the check is done. Adding an entry to a verified section means checking it too and updating `version` and `date`.

Check `executables` entries with `aieye scan` itself, not Task Manager or `Get-Process`. Windows reports a running program by its real file, which differs from the path it was started from when that path is a link, or when an update renamed the file while it ran. Both happen with Codex CLI and Claude Code.

Check `folders` and `mcp` entries against a real config that has such entries, looking at its key structure rather than its values. A selector that points at an empty object proves only the location, not the fields inside an entry, so it stays unverified.

## Example

```toml
id = "codex-cli"
name = "Codex CLI"
vendor = "OpenAI"
kind = "cli"
platforms = ["windows", "macos", "linux"]

[detect.windows]
executables = ["%LOCALAPPDATA%/Programs/OpenAI/Codex/bin/codex.exe"]
paths = ["~/.codex"]

[[folders.windows]]
file = "~/.codex/config.toml"
select = "projects.*"
trust = "trust_level"
boundary = false

[[mcp.windows]]
file = "~/.codex/config.toml"
select = "mcp_servers.*"
env = ["env", "env_vars"]

[verified.detect.windows]
version = "0.164.0"
date = 2026-10-26
```

## Known limits

- Tools installed through npm run as `node.exe`, so the process list cannot yet tell them apart from other Node programs. Their files are still found.
- Store (MSIX) apps live in `C:\Program Files\WindowsApps`, which a standard user cannot list. They are found through their running processes and data folders; reading the package list directly comes later.
- Only the user-level config files are read. Project-level files, such as a project's `.mcp.json`, come later.
- Copilot CLI has no `folders` or `mcp` section yet: the development machine has no trusted folders or MCP config for it to check against.
