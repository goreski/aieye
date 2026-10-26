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
| `[verified.<section>.<platform>]` | The `version` and `date` of the check of that section on a real install |

More sections arrive with later slices of v0.1: `config` (files to read and the fields that matter), `folders` (where trusted or granted folders are recorded, and whether they are a real boundary), `mcp`, `schedules` and `network`.

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

### `verified`

A section counts as verified for a platform only when every entry in it was checked on a real install. Record the tool version and the date as a plain TOML date:

```toml
[verified.detect.windows]
version = "0.164.0"
date = 2026-10-26
```

Leave the table out until the check is done. Adding an entry to a verified section means checking it too and updating `version` and `date`.

Check `executables` entries with `aieye scan` itself, not Task Manager or `Get-Process`. Windows reports a running program by its real file, which differs from the path it was started from when that path is a link, or when an update renamed the file while it ran. Both happen with Codex CLI and Claude Code.

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

[verified.detect.windows]
version = "0.164.0"
date = 2026-10-26
```

## Known limits

- Tools installed through npm run as `node.exe`, so the process list cannot yet tell them apart from other Node programs. Their files are still found.
- Store (MSIX) apps live in `C:\Program Files\WindowsApps`, which a standard user cannot list. They are found through their running processes and data folders; reading the package list directly comes later.
