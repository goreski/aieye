# AIEye

**Antivirus for the age of AI agents.**

AIEye is a service for any computer that shows exactly what AI tools can reach and what they actually did, and keeps them out of places they must never touch.

- **Watch:** inventory of AI tools, the folders and accounts they can reach, what they opened and contacted, and what runs on a schedule, with alerts when anything changes.
- **Guard:** no-go zones that AI tools cannot touch under any terms, enforced by the operating system.

AIEye relies only on records kept by the operating system and by the services being accessed. It never treats what an AI says about itself as evidence.

## Status

Early development. Slice 1 of v0.1 is in place: detecting AI tools on Windows from their install paths and running processes. See [docs/concept.md](docs/concept.md) for the full design, roadmap and open questions.

## Build and run

Needs Rust (stable) and, on Windows, the Visual Studio C++ Build Tools. From the repository root:

```sh
cargo run -p aieye -- scan
```

`scan` prints a JSON snapshot of what it found. It reads no file contents in this slice, only whether files exist and which processes run. AIEye keeps its own data in `%LOCALAPPDATA%\AIEye` (for now just the device ID); `--data-dir` points it elsewhere.

Checks run by CI:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Repository layout

| Path | Contents |
|---|---|
| `crates/aieye-model` | Records, evidence labels, snapshots |
| `crates/aieye-defs` | Loads and validates definitions |
| `crates/aieye-collect` | Collectors that ask the OS for facts |
| `crates/aieye` | The `aieye` command |
| `definitions/` | AI tool definitions (format in `definitions/README.md`) |
| `docs/concept.md` | Vision, architecture, Guard design, limits, roadmap |
| `CLAUDE.md` | Principles and decisions for working on the project with Claude Code |
