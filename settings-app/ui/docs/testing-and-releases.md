---
category: development
index: 3
icon: testing
name: Testing and releases
---

# Testing and releases

## Targeted checks

Run commands from the repository root using Node.js, Python, and Rust.

```powershell
node --test settings-app/tests/docs-data.test.cjs settings-app/tests/docs-ui.test.cjs settings-app/tests/markdown.test.cjs
python settings-app/tests/native-runtime.test.py
cargo test --manifest-path settings-app/Cargo.toml
```

Other JavaScript suites cover UI state, hover previews, dialogs, visibility fields, and release notes. Run affected suites when changing those behaviors. Browser tests require Playwright and Chrome; set `PLAYWRIGHT_MODULE` and `CHROME_PATH` if absent from default locations. A skipped browser test is not a visual check.

Native tests cover CSS generation, API behavior with a local mock server, connection replacement, and rollback. Use fake keys and workspace fixtures. DPAPI, filesystem ACLs, startup integration, and native shortcuts require Windows validation.

## Isolated installer tests

1. Build Settings with the required command.
2. Run installer checks.

```powershell
.\scripts\Build-Settings.ps1 -s -c
powershell -NoProfile -File scripts/tests/Test-Installer.ps1
```

Checks build both native installers and use folders beneath `.test-run`. They cover destinations, backups, repeated-install preservation, packaged executables, release selection, archive rejection, and failed-install cleanup. They do not launch Settings.

For a manual development install, build with `-d`. It confines installation to `BlobUI-Installer-Test` beside the executable, uses a separate Anytype data folder, and skips Start menu shortcuts. Repeated installs retain test settings and use new app folders.

```powershell
.\scripts\Build-Installer.ps1 -d -PackageRelease
.\dist\blobui-installer-dev.exe --package .\dist\blobui-windows.zip
```

Build Settings first if stale. Packaging does not rebuild or launch Settings. The development installer accepts local test ZIPs; production accepts GitHub downloads only.

## Windows smoke checks

With an intentionally selected test installation, verify launch, onboarding with and without API access, Save and Undo, generated CSS, Ctrl+R reload, paths containing spaces, background reopen, shortcuts, and exit. Check light and dark appearance and reduced motion. For docs, visit all seven articles, inspect tables and code blocks, check scrolling and indexed navigation, and confirm Introduction opens first. Bundled content must load without external requests.

F8 opens onboarding and F9 simulates an update by default. A simulation does not validate installer handoff, download, rollback, or restart.

## Prepare releases

1. Target feature and bugfix pull requests at `dev`. If a fix lands on `main`, merge `main` into `dev`, then merge `dev` into active feature branches.
2. Before handoff, verify `origin/main` is an ancestor of `dev` and `dev` is an ancestor of the active feature branch. Preserve both branches' features when resolving conflicts.
3. Update `version` in `blobui-release.json`. Write `Changelogs.md` and declare local images in `Changelogs.yaml`. Follow the [release notes authoring guide](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/docs/RELEASE-NOTES.md) for formats and limits.
4. Build Settings and the release package.

```powershell
.\scripts\Build-Settings.ps1 -s -c
.\scripts\Build-Installer.ps1 -PackageRelease
```

Packaging validates changelog sources and referenced images before writing `dist/blobui-windows.zip`. Preserve copyright and license attribution in distributed material.

## Publish assets

Release `main` from `dev`. Tag the release commit on `main` with a semantic version matching `blobui-release.json` and prefixed with `v`, such as `v1.2.0`. Commit `.github/workflows/windows-release.yml` to `main` before tagging. The workflow rejects tags outside `main` and mismatched versions.

The workflow builds Settings once, then builds the production installer and release package once. The installer validates the package before it is zipped, and the workflow checks that the ZIP contains both changelog files and the compiled Settings app. It uploads `blobui-installer.exe` and `blobui-windows.zip`, then publishes the release. The broader isolated installer checks remain available separately under `scripts/tests/Test-Installer.ps1`. For manual publishing, attach both files from `dist`. Automatic source archives do not contain the compiled app. The repository and releases must be public for unauthenticated updates.

Retry a tag with Actions > Release Windows installer > Run workflow and supply its tag. It uses GitHub's built-in token and creates a draft when needed. Retries can replace assets on mutable releases; immutable published releases require a new version tag. The updater pins the selected tag during handoff.
