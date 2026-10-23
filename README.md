# AIEye

**Antivirus for the age of AI agents.**

AIEye is a service for any computer that shows exactly what AI tools can reach and what they actually did, and keeps them out of places they must never touch.

- **Watch:** inventory of AI tools, the folders and accounts they can reach, what they opened and contacted, and what runs on a schedule, with alerts when anything changes.
- **Guard:** no-go zones that AI tools cannot touch under any terms, enforced by the operating system.

AIEye relies only on records kept by the operating system and by the services being accessed. It never treats what an AI says about itself as evidence.

## Status

Concept stage. See [docs/concept.md](docs/concept.md) for the full design, roadmap and open questions.

## Repository layout

| Path | Contents |
|---|---|
| `docs/concept.md` | Vision, architecture, Guard design, limits, roadmap |
| `definitions/` | AI tool definitions (format in `definitions/README.md`) |
| `CLAUDE.md` | Principles and decisions for working on the project with Claude Code |
