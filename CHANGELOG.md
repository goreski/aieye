# Changelog

All notable changes to AIEye. Versions follow [semantic versioning](https://semver.org/); before 1.0, a minor version may change the definitions format or the data AIEye stores.

## 0.1.0 (2026-11-01)

The first release: an inventory of what AI tools can reach on a Windows computer, and what changed since the last look.

### Added

- **Tool detection** from definitions: install paths, and running processes matched by their real executable, including binaries renamed by an update while still running.
- **Configs:** the folders and MCP servers that AI tools' own config files name, labelled `declared`. MCP servers keep only their name, command or URL origin, and the names of their environment variables; values, arguments, headers and tokens are dropped.
- **Schedules:** every Windows Task Scheduler task outside Windows' own folders, labelled `verified` and linked to an AI tool when it runs one of the tool's programs or something in its folders; and AI apps' own scheduled tasks from their config files, labelled `declared`. Arguments and prompts are never kept.
- **Snapshots and changes:** every scan is saved in a local SQLite file. `aieye diff` and the app compare scans with fixed rules: new tools, new or wider access, and new or changed schedules are alerts.
- **Desktop app** (Tauri 2): totals, changes, a card per AI tool with its evidence, every scheduled task on the device, and export of a snapshot as JSON. It opens no network port.
- **Command line:** `aieye scan`, `aieye diff` and `aieye history`, with JSON output.
- **Definitions** for Antigravity, Claude Code, Claude desktop, Codex CLI, Gemini desktop and GitHub Copilot CLI, with the sections checked on a real install marked `verified`.
- **Installers** for Windows, built by GitHub Actions with SHA-256 checksums: a per-user `setup.exe` and a per-machine `.msi`.

### Known limits

- Windows only. The installers are not code-signed yet.
- AIEye runs as the signed-in user, so AI tools running as that user can also reach its data folder. Guard, which keeps AI tools out of places, is not built yet.
- Claude desktop's agent-mode schedules are not checked on a real install yet.
