# Developer guide

![Calendar color example](@calendar-event-colors)

This page covers the Windows Settings app, local build steps, release notes, and the GitHub workflow.

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
scripts/Build-App.ps1 -s -c
```

The script builds the frontend, packages the local Anyblob plugin, and compiles the Rust Settings app. To build only the frontend while working on its UI, run these commands from `settings-app`:

```powershell
npm ci
npm run build
```

Use `scripts/Build-Installer.ps1` to package the app as a Windows installer after building it.

## Release notes and Markdown

`Changelogs.md` contains release notes. `Changelogs.yaml` declares the local images and videos used by those notes. Add release entries when preparing a release, and use Markdown for headings, lists, links, code blocks, and images.

The Dev Guide also uses Markdown. Local guide images use aliases registered in `settings-app/ui/markdown-media.js`, for example `![BlobUI mark](@blobui-mark)`. The app loads these files through its sanitized Markdown renderer.

## GitHub workflow

- Create feature and bugfix branches from `dev`. Use branch names such as `feat/calendar-showcase`, `fix/scrollbar`, `refactor/build-scripts`, or `chore/dependencies`.
- Open feature and bugfix pull requests against `dev`.
- Release `main` from `dev`. Keep `dev` up to date when a fix lands directly on `main`.
- Use commit subjects such as `feat: add calendar showcase` or `fix: keep tabs aligned` and include a short commit description.
