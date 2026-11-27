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

The `Build-App` script builds the Settings frontend and Rust app. It does not package the Anyblob plugin or build the headless CLI.

Run `scripts/Build-Codex.ps1` to validate and package the Anyblob plugin from `codex-plugin/src`. Run `scripts/Build-BlobUI-Headless.ps1` to compile the no-UI `blob-ui` CLI. Add `-s` to install it in the per-user programs folder and add it to PATH. Add `-c` to stop a running copy built from this workspace before compiling.

The script builds the frontend and compiles the Rust Settings app. To build only the frontend while working on its UI, run these commands from `settings-app`:

```powershell
npm ci
npm run build
```

`scripts/Build-Installer` will package the app as a Windows installer after building it.

## Version Formatting

- For the app: `full-recode.update.hotfix`
- For Anyblob: `full-recode.update.hotfix`
- For the Headless CLI: `full-recode.update.hotfix`

## Markdown formatting

Both Changelogs and the dev guide both use BlobUIs Markdown reader.
You can basically do everything you can normally do in markdown, tables, bold text, code text, hyperlinks, etc.
But there are a few additional features, you can use `[ ]` and `[x]` to make checkmarks.
You can also mention images that are included in `md-images.yaml` by `@`-mentioning them.
Another feature the BlobUI markdown reader supports is `Issue#ID`, `PR#ID` and `User#username` to quickly mention issues, prs and users.

## BlobUI Headless CLI

For a full list of all commands use `blob-ui help`.
There is an additional article on how to setup BlobUI in the Anyblob settings.
Some useful ones are: 

- `blob-ui spaces ls` - lists all spaces BlobUI has access to with the API key.
- `blob-ui api status` - API status.
- `blob-ui api --host-ip <IP> --host-port <PORT>` - change the IP & Port, also makes BlobUI not run `anytype serve` in the background for qol.
- `blob-ui api login` - add an API key.
- `blob-ui --toggle-anyblob <bool>` - toggle anyblob.

## GitHub Guidelines

- When making changes, always target your PRs to `dev` unless it's a hotfix for the current version.
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
    - `chore/{chore}` - chores.
