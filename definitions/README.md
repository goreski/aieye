# AI definitions

Definitions tell AIEye how to recognise an AI tool and where to read what it is configured to reach. Like antivirus signatures, they are data files updated independently of the app, so a new AI tool needs a new definition, not a new AIEye release.

One YAML file per tool, named `<id>.yaml`.

## Format (draft)

| Field | Meaning |
|---|---|
| `id` | Stable identifier, e.g. `codex-cli` |
| `name`, `vendor` | Display name and vendor |
| `kind` | `desktop-app`, `cli`, `ide-extension`, `browser-extension` or `mcp-server` |
| `platforms` | `windows`, `macos`, `linux` |
| `detect` | How to recognise it: process names, install paths, registry keys, extension IDs |
| `config` | Files to read, each with a parser and the fields that matter |
| `folders` | Where trusted or granted folders are recorded, and whether they are a real boundary |
| `mcp` | Where configured MCP servers are listed |
| `schedules` | Where the tool keeps local scheduled tasks, if any |
| `network` | Domains the tool is expected to contact |
| `verified` | Tool version and date each entry was checked on a real install; unchecked entries stay `false` |

## Example (illustrative — not yet verified)

```yaml
id: codex-cli
name: Codex CLI
vendor: OpenAI
kind: cli
platforms: [windows, macos, linux]
detect:
  process: [codex, codex.exe]
config:
  - path: ~/.codex/config.toml
    parser: toml
folders:
  source: ~/.codex/config.toml
  key: projects            # [projects."<path>"] trust_level = "trusted" | "untrusted"
  boundary: false          # trust loads project config; it does not restrict file access
mcp:
  source: ~/.codex/config.toml
schedules: null
network: []                # to fill in from observed traffic
verified: false
```

Definitions are data. Each path and key must be checked on a real install, with the tool version and date recorded in `verified`, before AIEye treats it as reliable.
