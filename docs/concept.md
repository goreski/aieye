# AIEye — concept

*Status: concept · October 2026 · Goran Oreški*

## 1. The idea

**AIEye is antivirus for the age of AI agents.** It is a service installed on any computer that shows exactly what AI tools can reach and what they actually did, and keeps them out of places they must never touch.

One rule defines it:

> AIEye relies only on records kept by the operating system and by the services being accessed. It never treats what an AI says about itself as evidence.

## 2. Why

- AI assistants now hold access to mail, calendars and drives, read local folders, and run scheduled tasks while nobody is watching.
- Each vendor shows only its own slice (connectors, projects, scheduled tasks), and only as self-reported lists.
- Existing tools that cover this (MCP gateways, SaaS security platforms) are built for enterprise security teams and only see traffic routed through them.
- Nobody gives an ordinary user, or a small organisation, one independent view of "what can AI touch on this machine, and what did it do."

## 3. Two pillars

| Pillar | What it does | Default |
|---|---|---|
| **Watch** | Inventory, activity, schedules, change alerts | Always on |
| **Guard** | No-go zones that AI tools cannot touch under any terms | Opt-in per zone |

## 4. The antivirus analogy

| Antivirus | AIEye |
|---|---|
| Engine installed on every device | Background service (Windows first, then macOS and Linux) |
| Virus definitions, updated daily | **AI definitions**: catalogue of known AI tools and how to detect and read each one |
| Full scan | **Inventory scan**: which AI tools are present, what they are configured to reach, which folders they can touch, what is scheduled |
| Real-time protection | **Live watch**: files AI processes open, services they contact, new recurring patterns |
| Threat alert | **Change alert**: new AI tool, new trusted folder, new MCP server, AI touching a sensitive folder, new schedule |
| Quarantine | **Guard**: no-go zones enforced by the OS. Otherwise AIEye observes and the user decides |
| Tamper protection | AIEye's own folder, policy and service are a no-go zone for AI processes |
| Business console | Optional central console for organisations; locally a desktop app on the device |

## 5. What AIEye shows

### 5.1 Inventory: what can reach what
- **AI tools present:** installed apps, command-line agents, IDE extensions, browser extensions, local MCP servers.
- **Configuration of each tool:** trusted folders, configured MCP servers and the services they point to.
- **Folder reach:** effective read/write access according to OS permissions. This is only meaningful when agents run under their own account; otherwise the honest answer is "everything the user can reach."
- **Cloud connections:** accounts reachable through middlemen whose logs AIEye can read (e.g. Composio), and through the AIEye gateway.

### 5.2 Activity: what actually happened
- **Files:** which process opened which file, and when (OS file auditing).
- **Network:** which process contacted which domain, and when. Content stays encrypted; who, where and when are exact.
- **Cloud calls:** tool calls recorded by middlemen and by the AIEye gateway.

### 5.3 Schedules
- OS schedulers (Windows Task Scheduler, launchd, cron).
- Local schedulers inside AI apps that keep their tasks in files on disk.
- Recurring patterns detected in file, network and cloud activity (e.g. "weekdays ~07:38 and 13:38").
- Vendor-reported scheduled tasks, shown only as a cross-check.

### 5.4 Relationship map
One map of the chains, with middlemen as their own step, for example:
`Claude → Composio → user@example.org → Gmail` or `Codex → C:\Users\…\Projects`.

### 5.5 Change alerts
Each snapshot is compared with the previous one. Alerts for:
- **New or wider access:** new AI tool, new trusted folder, new MCP server, new connected account, wider scopes.
- **New behaviour:** a new recurring pattern, a new domain contacted by an AI process.
- **Declared vs observed mismatch:** activity that matches no declared task; a declared task with no matching activity; activity from an app with no known access.
- **Guard events:** every blocked attempt on a no-go zone.

From v0.1, every scan is saved and `aieye diff` compares consecutive snapshots with fixed rules: a tool not in the older snapshot, and a configured folder or MCP server that was added or changed, are **alerts**; removals, install traces and running-process changes are **info**. Facts are matched by what they are about (tool and path, executable, config entry), not by process ID, and each change keeps the evidence label of its records.

### 5.6 History
A timeline per AI tool, per folder and per account, exportable as CSV.

### 5.7 Evidence labels
Every fact carries one label:

| Label | Meaning | Examples |
|---|---|---|
| **verified** | Recorded by the OS or by the service being accessed | NTFS permissions, Security log events, Sysmon events, Composio execution logs, gateway logs |
| **declared** | A tool's own configuration or a vendor's own list | Trusted-folder files, MCP config, vendor scheduled-task lists |
| **inferred** | Produced by AIEye's deterministic detection rules | Recurring-pattern detection |

## 6. Architecture

```
            ┌──────────────────────── device ────────────────────────┐
            │                                                        │
 AI tools ──┤  Device agent  ── reads ──  configs, OS schedulers,    │
            │       │                     permissions, audit logs,  │
            │       │                     ETW events                │
            │       ▼                                                │
            │  Local store (snapshots + events)                      │
            │       │                                                │
            │       ▼                                                │
            │  Desktop app (Tauri)                                   │
            └───────┬────────────────────────────────────────────────┘
                    │ optional
                    ▼
      Org console (central)       Gateway (always-on host)       Log connectors
      aggregates many devices     cloud AI → gateway → accounts   Composio, others
```

| Component | Role | Where it runs |
|---|---|---|
| Device agent | Scans, watches, enforces Guard rules | Each computer, as a service |
| Definitions | Knowledge of how to detect and read each AI tool | Data files, updated independently of the app |
| Local store | Snapshots, events, diffs | Device |
| Desktop app | Inventory, map, alerts, history | Tauri app running as the signed-in user, with tray icon and notifications |
| Log connectors | Pull execution logs from middlemen (Composio first) | Device agent or gateway host |
| Gateway (optional) | Holds tokens, logs every cloud AI call, enforces cloud no-go zones | Always-on host: small VPS, Raspberry Pi with tunnel, or a company server |
| Org console (optional) | Central view across devices for organisations | Server |

### 6.1 Stack

*Decided October 2026.* AIEye writes its own collector rather than building on osquery and Sysmon, and the agent is written in **Rust**.

- **Rust:** the agent runs as SYSTEM or root and parses files that AI tools write, so memory safety matters. Rust matches C++ for speed, and one Cargo workspace builds for Windows (x64 and ARM), macOS, Linux and a Raspberry Pi gateway.
- **Own collector:** on Windows it reads ETW (Event Tracing for Windows) directly, so users need neither Sysmon nor an audit policy, and AIEye bundles no third-party tools.

| Part | Choice |
|---|---|
| Language | Rust (stable), one Cargo workspace |
| Async runtime | `tokio` |
| Windows APIs | `windows` (Microsoft's Win32 bindings), `windows-service`, `ferrisetw` for ETW |
| Linux and macOS (later) | `aya` (eBPF) and fanotify; `endpoint-sec` (Endpoint Security) |
| Config parsing | `serde` with `serde_json` and `toml` |
| Local store | SQLite via `rusqlite` (bundled) |
| File watching | `notify` |
| Desktop app | Tauri 2 with a Svelte + Vite (TypeScript) frontend; opens no network port |
| CLI and logging | `clap`, `tracing` |
| Signed definitions | Ed25519 (`ed25519-dalek`) |
| Gateway (v0.5) | `rmcp` (official Rust MCP SDK) on `axum` |
| Installers | Tauri bundler: MSI on Windows, DMG on macOS, DEB and AppImage on Linux |

The v1.0 Windows minifilter driver is the exception: its language is decided when that work starts (§15).

*Decided October 2026:* the dashboard is a **Tauri 2 desktop app**, not a web app on `localhost`. A localhost server can be called by every local process, including the AI tools AIEye watches; a Tauri app opens no port. It also gives a tray icon, native notifications for change alerts, and installers for all three platforms from one Svelte frontend, using the system's web engine (WebView2 on Windows, WebKit on macOS and Linux). In v0.1 the app calls the collectors directly. From v0.2 it talks to the service over a local channel (§13).

## 7. Definitions

AI tools change every month, so knowledge of how to recognise and read them lives in updatable data files, not in code. New tools are supported by shipping a new definition, not a new app version. Definitions are open and community-maintained, like ad-block filter lists, under the same Apache-2.0 licence as the code. They are TOML files; see `definitions/README.md` for the format.

## 8. Guard: no-go zones

### 8.1 The core problem
AI tools run under the user's own account, so normal folder permissions (per user) cannot tell "the user opened this file" from "an AI tool opened it." Real enforcement must be **per process**, the way antivirus works.

| OS | Per-process enforcement | Hurdle |
|---|---|---|
| Windows | File-system minifilter driver | Microsoft driver signing |
| macOS | Endpoint Security framework (authorise each file open) | Apple entitlement |
| Linux | fanotify permission events, AppArmor profiles | Root; distro differences |

### 8.2 Three layers, soft to hard
1. **Tool settings:** AIEye writes deny rules into each tool's own config. Useful, but the tool enforces it on itself.
2. **OS enforcement:** v1 runs AI tools under a dedicated account (e.g. `ai-agent`) with explicit deny permissions on no-go folders; the kernel enforces it. Later: the driver / Endpoint Security / fanotify.
3. **Gateway enforcement:** cloud calls touching protected places (Drive folders, mail labels, calendars) are refused.

### 8.3 Holes to close
- **Child processes:** agents start `python`, `node`, `powershell`, `git`. Rules must follow the whole process tree.
- **Cloud sync:** a no-go folder synced to OneDrive, iCloud or Google Drive can be read in the cloud by an AI with a Drive connector. No-go zones must stay out of synced folders or be enforced in the gateway too.
- **The user's browser:** an AI driving the user's browser can upload a file through the browser process.
- **AIEye itself:** its folder, policy file and service are no-go by default (tamper protection).

### 8.4 Default no-go list
SSH and other key folders, browser profiles (cookies, saved passwords), password-manager vaults, `.env` files, AIEye's own folder. Users add their own (e.g. contracts, financial records).

Every blocked attempt is logged and alerted, e.g. *"Codex tried to read `D:\Finance` at 14:02."*

## 9. Covering the cloud blind spot

Anything an AI does from its vendor's servers never touches the device. Example: a cloud scheduled task that reads mail through Composio and writes to a calendar through the vendor's own Google Calendar connector.

| Approach | Coverage | Label |
|---|---|---|
| Middleman logs (Composio's execution-log API) | Every call routed through that middleman | verified |
| AIEye gateway: cloud AI connects to it instead of directly to the service | Everything routed through it; direct connectors are then removed | verified |
| Vendor lists of scheduled tasks and connectors | Cross-check only | declared |

In the example, the Composio part is visible through its logs, while the direct calendar connector stays invisible. Policy that follows: route cloud AI access through the gateway.

**Still uncovered:** direct vendor connectors the user keeps, and vendor-internal actions that never touch the user's accounts (e.g. reading files uploaded to a chat project).

An optional organisation module can add account-side evidence for managed domains (Google Admin SDK `tokens.list`; Reports API OAuth token events). It requires admin rights.

## 10. Data sources (reference, verify before use)

| Source | What it gives | Label |
|---|---|---|
| Codex `~/.codex/config.toml`, `[projects."<path>"] trust_level` | Trusted / untrusted project folders | declared |
| Gemini CLI `~/.gemini/trustedFolders.json` | Trusted folder rules | declared · to verify; `~/.gemini` is shared with Antigravity |
| Antigravity `~/.gemini/config/projects/*.json`: `projectResources.resources[].gitFolder.folderUri` | Folders each project works on, as file URIs | declared |
| Antigravity `~/.gemini/config/mcp_config.json`, `mcpServers` | MCP servers | declared · to verify: empty on the dev machine |
| Copilot CLI `~/.copilot/config.json` (JSON with `//` comments), `trustedFolders` | Trusted folders | declared · to verify: not set on the dev machine |
| Claude Code `~/.claude.json`: `projects.<path>.hasTrustDialogAccepted` | Folders used and whether the user trusted them | declared |
| Claude Code `~/.claude.json`: `mcpServers`, `projects.<path>.mcpServers` | MCP servers, user and per-project scope | declared · to verify: empty on the dev machine |
| Claude Code `~/.claude/settings.json`: `permissions` (`allow`, `deny`, `additionalDirectories`) | Permission rules, extra folders | declared · to verify: not set on the dev machine |
| Claude desktop (Store build) `%LOCALAPPDATA%\Claude-Data\claude_<publisher id>\claude_desktop_config.json`, `mcpServers` | MCP servers | declared |
| Claude desktop `AppData\Roaming\Claude\…\scheduled-tasks.json` | Local scheduled tasks and their working folders | declared · reported by users, to verify |
| Windows Task Scheduler (COM `ITaskService`) | OS scheduled jobs | verified |
| NTFS permissions via the Windows Authz API | Effective folder access per account | verified |
| ETW `Microsoft-Windows-Kernel-Process` | Process starts and stops with parent process (process tree) | verified · to verify |
| ETW `Microsoft-Windows-Kernel-File` | File opens by process | verified · to verify |
| ETW `Microsoft-Windows-DNS-Client`, `Microsoft-Windows-Kernel-Network` | DNS lookups and connections by process | verified · to verify |
| Composio `POST /api/v3.1/logs/tool_execution` | Tool calls: time, status, tool, account | verified |
| Google Admin SDK (org module, admin only) | OAuth tokens per user; API calls per app | verified |

Note: in Gemini CLI and Codex, "trusted" controls whether project settings and tools load. It is not an access boundary.

The ETW rows replace Windows Security log event 4663 (needs an audit policy) and Sysmon events 3 and 22 (needs a third-party install). Both stay available as fallbacks.

## 11. Limits

- **Phones:** iOS does not allow one app to see what other apps do, and Android allows very little. AIEye targets computers.
- **Not tamper-proof against deliberate evasion:** it watches AI tools, not attackers.
- **Personal Google accounts:** no API for apps-with-access or per-app activity.
- **Notion, Strava and similar:** no per-app activity logs for individual users.
- **Cloud internals:** see §9.

## 12. Scaling

- **Device-based by design:** install on each device. No account admin rights needed.
- **Organisations:** optional central console. In the EU, collecting employee activity needs a GDPR legal basis, transparency, very likely a DPIA, and early involvement of the data protection officer. Default to each person seeing their own data and admins seeing aggregates.
- **Data model from day one:** every record carries `device_id`, nullable `org_id`, `source`, `evidence`, `observed_at`.
- **Platforms:** Windows → macOS → Linux.

## 13. Securing AIEye itself

AIEye ends up holding the most sensitive view of the machine. It is read-only by default, local-first (nothing leaves the device unless the user enables it), unreachable by AI processes, and ships signed definitions.

The desktop app opens no network port, so AI processes cannot call it the way they could call a web server on `127.0.0.1`. From v0.2 it talks to the service over a local channel (a named pipe on Windows, a Unix socket elsewhere) on which the service checks which program is connecting, and changing Guard policy requires administrator approval.

Config parsers read only the fields a definition lists and never store secret values: for MCP servers, AIEye keeps the server name, its command or the scheme, host and port of its URL, and the names of its environment variables, never their values, arguments, headers or tokens. Credentials, paths and queries are cut from URLs because hosted MCP servers often carry tokens there.

## 14. Roadmap

| Version | Scope |
|---|---|
| **v0.1 Inventory** | Windows: detect AI tools from definitions; parse configs, trusted folders, MCP servers; Task Scheduler and local AI schedulers; snapshots and diffs; desktop app (Tauri) |
| **v0.2 Live watch** | Windows service; ETW collector (process, file, DNS, network); per-process attribution following the process tree; recurring-pattern detection |
| **v0.3 Guard v1** | Dedicated agent account with deny permissions; no-go policy file; blocked-attempt alerts; tool-settings deny rules |
| **v0.4 Cloud logs** | Composio log connector; vendor-declared lists; declared-vs-observed alerts |
| **v0.5 Gateway** | Self-hosted MCP gateway with logging and cloud no-go enforcement |
| **v1.0** | Kernel-level Guard (Windows minifilter); macOS (Endpoint Security); Linux (fanotify); org console |

v0.1 is built in slices: (1) tool detection from install paths and running processes, (2) configs: trusted folders and MCP servers, (3) store and diff, (4) desktop app, (5) schedulers. It runs as the signed-in user and installs nothing; the service arrives with ETW in v0.2, which needs it. Until then, AIEye's data folder (`%LOCALAPPDATA%\AIEye`) is reachable by AI tools running as the same user.

## 15. Open questions

- Language for the v1.0 Windows minifilter: C, or Rust via `windows-drivers-rs`?
- Where to host the gateway?
- Name: "AIEye" has no conflict in AI monitoring, but an unrelated iOS app and a tool called "AI Bot Eye" exist. Check again before going public.
