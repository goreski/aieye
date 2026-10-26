# Security policy

AIEye sees a lot about the machine it runs on, so its own security matters as much as what it reports.

## Supported versions

AIEye is before 1.0. Security fixes go into `main` and the next release; older releases are not patched.

## Report a vulnerability

Please don't open a public issue. Report it privately through GitHub: on the repository's **Security** tab, choose **Report a vulnerability** ([direct link](https://github.com/goreski/aieye/security/advisories/new)).

Include the AIEye version or commit, your Windows version, the steps to reproduce, and what an attacker could do with it. We aim to reply within 7 days and will keep you updated until the problem is fixed. Tell us if you'd like to be credited in the advisory.

## What counts

- AIEye storing, showing or exporting a secret value it should drop: environment values, tokens, headers, or credentials, paths and queries in URLs.
- A definition or config file that makes AIEye read files it should not, run code, or crash or hang while parsing.
- A way for web content or another process to call the desktop app's commands, or anything that makes the app open a network port.
- A way to tamper with release builds or the definitions shipped with them.

## Known limits

These are known and documented, so they aren't vulnerabilities on their own:

- Version 0.1 runs as the signed-in user, so any process running as that user, AI tools included, can read and change AIEye's data folder (`%LOCALAPPDATA%\AIEye`). Protecting it comes with Guard (see `docs/concept.md`, §13).
- AIEye watches AI tools. It is not built to resist an attacker deliberately hiding from it.
- Installers are not code-signed yet.
