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

The `Build-App` script will also build the MCP server for Anyblob and Anyblob itself.
You can also use: `scripts/Build-Codex` to build Anyblob and `scripts/Build-MCP` to build the MCP server.

The script builds the frontend, packages the local Anyblob plugin, and compiles the Rust Settings app. To build only the frontend while working on its UI, run these commands from `settings-app`:

```powershell
npm ci
npm run build
```

`scripts/Build-Installer` will package the app as a Windows installer after building it.

## Version Formatting

- For the app: `full-recode.update.hotfix`
- For Anyblob: `massive-new-feat.new-feat.fix`

## Markdown formatting

Both Changelogs and the dev guide both use BlobUIs Markdown reader.
You can basically do everything you can normally do in markdown, tables, bold text, code text, hyperlinks, etc.
But there are a few additional features, you can use `[ ]` and `[x]` to make checkmarks.
You can also mention images that are included in `md-images.yaml` by `@`-mentioning them.
Another feature the BlobUI markdown reader supports is `Issue#ID`, `PR#ID` and `User#username` to quickly mention issues, prs and users.

## Custom Themes

You can create custom built in themes using css.
To create a theme, create a new css file in `app-installation-folder/Settings/themes/built-in-themes/`.

Once you created a file paste this at the top of the file:
*Developers Note: The header always has to be at the top of the css file, the order of fields does not matter.*

```
/*
author: {username}
name: {name that is displayed in the app}
*/
```

And from there you can go ahead customizing Anytype to your liking.
See existing built-in themes and `app-installation-folder/Settings/generated.css` for reference.
*Developers Note: Yes I will admit, I'm too lazy to document the css variables.*

One thing I'm not too lazy to admit are the available fields you can put in the commented header in your custom theme.
The available fields are:

- **(required)** `author: {text}` - currently unused but might get used in the future, also useful to know who created the theme.
- **(required)** `name: {text}` - the name that is displayed in the app.
- `based-theme: Dark Mode / Light Mode` - the based theme, controls things like text color, icon color and so on. 
- `accent-color: {color code}` - Override the users accent color.
- `accent-color-selections: {bool}` - Override whether selections are effected by the accent color. (*requires `accent-color` field*)

*Developers Note: Errors & warnings are logged in logs.*

You can use `ctrl+r` to reload available custom preset themes.

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