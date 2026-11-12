# Developer guide

## Set up

Install Git for Windows, Node.js LTS, Rust stable for `x86_64-pc-windows-msvc`, Visual Studio Build Tools with the Desktop development with C++ workload, and a Windows SDK.

The Settings frontend uses the versions in `settings-app/package-lock.json`. You do not need a global React install.

## Build and test

Run these commands from the repository root to build the Settings app and installer.

```powershell
scripts/Build-App.ps1 -s -c
scripts/Build-Installer.ps1
```

`Build-App.ps1` also builds Anyblob and its MCP server. Use `-s` to start the app after a build, `-b` to start it in the background next time, `-n` to show onboarding next time, and `-c` to close earlier Settings app instances.

To run the frontend tests, use `npm test` from `settings-app`. Build the frontend first with `npm ci` and `npm run build` when needed.

## Custom themes

Open Preferences, then Custom Themes. Choose Auto to select light or dark mode from the main color. Light Mode and Dark Mode force a palette. Don't Override follows Anytype's selected mode.

Put theme CSS files in the folder shown in Custom Themes. Each file needs a commented name and author header.

```css
/*
name: My Theme
author: Your Name
*/
html, html.themeDark {
  --color-bg-primary: #191918;
}
```

Saving previews the selected palette in BlobUI Settings and writes it to Anytype's `custom.css`. Press Ctrl+R in Anytype to apply it. Custom CSS cannot change Anytype's native Windows titlebar or its separate app tab bar.

## Project rules

Target feature and bug fix pull requests at `dev`. Release `main` from `dev`. Do not release or bump a version from a feature branch. Keep fixes within the GPL-v3 license and do not modify the license.
