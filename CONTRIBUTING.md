# Contributing to AIEye

Thanks for helping. The most useful contribution is also the easiest: a **definition** for an AI tool you use. AI tools change every month, and nobody can keep up with all of them alone, but everyone can check the tools on their own machine.

## Add or fix a definition

A definition is one TOML file in [`definitions/`](definitions/) that tells AIEye how to recognise a tool and where its config names folders and MCP servers. The format is in [`definitions/README.md`](definitions/README.md). You need Rust (stable) and, on Windows, the Visual Studio C++ Build Tools to run the scanner.

1. **Pick a tool** you have installed. Check the open issues labelled `definition` and the files in `definitions/` first, so two people don't do the same one.
2. **Start from a similar file:** `codex-cli.toml` for a command-line tool, `claude-desktop.toml` for a desktop app. Name the copy `<id>.toml`, where `id` is lowercase letters, digits and dashes.
3. **Fill in `detect`.** `executables` are the tool's program files: a running process started from one is reported as the tool running. `paths` are files or folders whose existence shows the tool is installed or has been used. Use the path patterns from `definitions/README.md` (`~/`, `%LOCALAPPDATA%`, `*`), never your own user name.
4. **Add `folders` and `mcp`** if the tool's config names trusted folders or MCP servers. Point only at the fields that hold names, commands and URLs.
5. **Check it on your machine** with the tool open:

   ```sh
   cargo run -p aieye -- scan --no-save
   ```

   Look for records with your tool's `id`: `tool_present` for each path found, `tool_running` for each process, and the folders and MCP servers from its config. Check `executables` with this scan, not Task Manager: Windows reports a process by its real file, which differs from the path it was started from when that path is a link or an update renamed the file.
6. **Mark what you checked.** Add a `[verified.<section>.<platform>]` table with the tool's version and today's date, but only for sections where you checked every entry on a real install. Unchecked entries are welcome too; leave their `verified` table out.
7. **Open a pull request** that says the tool's version, how it was installed (installer, Microsoft Store, winget, npm), your Windows version, and which records the scan produced.

AIEye is developed on Windows first. Sections for macOS and Linux are welcome; say in the pull request how you checked them.

### Rules for definitions

- **Only the tool's own files.** A definition must never point AIEye at secrets or other programs' data: SSH keys, browser profiles, password vaults, `.env` files, token stores. Such a pull request is closed.
- **No secret values.** AIEye keeps names, commands and the scheme, host and port of URLs, and drops every value. Don't add fields to capture more.
- **Redact before you paste.** Replace your user name in paths with `you` (`C:\Users\you\…`). Never paste tokens, environment values or whole config files into an issue or pull request; describe a config by its key structure.

## Report a missing tool or a wrong definition

If you can't write the definition yourself, open an issue with the **New AI tool** template and tell us where the tool lives on your machine. For a crash, a tool AIEye misses or a wrong record, use the **Bug report** template.

## Change the code

For anything bigger than a small fix, open an issue first. The design, roadmap and limits are in [`docs/concept.md`](docs/concept.md), and new features follow the roadmap there.

Every change has to keep AIEye's principles (see the README). In particular:

- **Deterministic evidence only.** Facts come from the OS or from the service being accessed. A change that treats an LLM, or an AI tool's description of itself, as a source of truth won't be merged.
- **Every fact carries an evidence label:** `verified`, `declared` or `inferred`.
- **Read-only and local-first.** AIEye changes nothing on the machine unless the user configured it, and sends nothing off the device.
- **Definitions are data.** Knowledge of a specific tool belongs in `definitions/`, not in code.

Test only with temporary folders and dummy files, never real secrets. To look at the desktop app while you work on it, point `AIEYE_DATA_DIR` at a scratch folder so it doesn't touch your real data.

Before you open a pull request, run the same checks as CI. The app crate embeds `app/dist`, so build the frontend first:

```sh
npm --prefix app ci
npm --prefix app run check
npm --prefix app run build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Keep a pull request to one topic. Write the commit subject in the imperative ("Add Cursor definition") and use the body for why.

## Security problems

Don't open a public issue for a vulnerability. See [`SECURITY.md`](SECURITY.md).

## Licence

AIEye is licensed under the [Apache License 2.0](LICENSE). By contributing, you agree that your contribution is licensed under it too, as section 5 of the licence describes. There is no separate contributor agreement.

Everyone taking part follows the [code of conduct](CODE_OF_CONDUCT.md).
