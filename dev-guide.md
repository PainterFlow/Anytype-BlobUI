This page will guide you through the development of BlobUI.

## Required tools

- Git for Windows.
- Node.js LTS. npm comes with Node.js. React and the other frontend packages install locally through `npm ci`.
- Rust stable for the `x86_64-pc-windows-msvc` target.
- Visual Studio Build Tools with the Desktop development with C++ workload and a Windows SDK.

## Install the tools

1. Install [Node.js LTS](https://nodejs.org/en/download).
2. Install [Rust with rustup](https://www.rust-lang.org/tools/install), then add the `stable-x86_64-pc-windows-msvc` target if rustup did not select it.
3. In Visual Studio Installer, add the Desktop development with C++ workload and a Windows SDK.
4. Install [Git for Windows](https://git-scm.com/download/win) if it is not already available.
5. Open a new PowerShell window and check `node --version`, `npm --version`, `rustc --version`, `cargo --version`, and `git --version`.

You do not need a global React install. The Settings build installs the project's pinned JavaScript packages with `npm ci`.

## Build BlobUI

From the repository root, build the Settings app with:

```powershell
scripts/Build-App.ps1
```

You can use the arguments: `-s` for autostarting the app, `-b` for launching it in the background next time the app launches, `-n` to make onboarding appear next time the app launches and `-c` to close all previously running instances.

The `Build-App.ps1` script will also build the MCP server for Anyblob and Anyblob itself.
You can also use: `scripts/Build-Codex.ps1` to build Anyblob and `scripts/Build-MCP.ps1` to build the MCP server.

The script builds the frontend, packages the local Anyblob plugin, and compiles the Rust Settings app. To build only the frontend while working on its UI, run these commands from `settings-app`:

```powershell
npm ci
npm run build
```

`scripts/Build-Installer.ps1` will package the app as a Windows installer after building it.

## Version Formatting

- For the app: `full-recode.update.hotfix`
- For Anyblob: `massive-new-feat.new-feat.fix`

## Markdown formatting

Both Changelogs and the dev guide both use BlobUIs Markdown reader.
You can basically do everything you can normally do in markdown, tables, bold text, code text, hyperlinks, etc.
But there are a few additional features, you can use `[ ]` and `[x]` to make checkmarks.
You can also mention images that are included in `md-images.yaml` by `@`-mentioning them.
Another feature the BlobUI markdown reader supports is `Issue#ID`, `PR#ID` and `User#username` to quickly mention issues, prs and users.

## GitHub Guidelines

- When making changes, always target your PRs to `dev` unless it's for a hotfix.
- Never attempt to release versions.
- Do not bump the version.
- Make sure your changes actually work.
- Please follow Anytypes and the current apps UI design philosophy.
- Do not attempt to backdoor or put malware in the app.
- Modifications you make to BlobUI are licensed under GPL-v3.
- Do not attempt to modify the license.
- Branch naming guidelines:
    - `version/{version-id}` - backups of released versions of BlobUI.
    - `fix/{issue-id(s)}` - bug fixes, use `fix/mass-fix` for multiple issues - always mention all fixed issues in the description.
    - `feat/{feature-name}` - new feature requests.
    - `migrate/{migration to ...}` - migration to, for example, native rust or something like that.