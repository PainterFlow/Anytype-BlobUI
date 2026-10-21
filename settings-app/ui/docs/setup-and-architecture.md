---
category: development
index: 1
icon: setup
name: Setup and architecture
---

# Setup and architecture

## Development requirements

Use Windows with Rust and the Windows C++ build tools installed. Settings uses Tauri 2 and the Windows WebView2 runtime. Node.js runs JavaScript tests, Python runs native-runtime checks, and PowerShell runs the build and installer test scripts. These developer tools are not runtime dependencies of the native installer.

## Build and launch

Run commands from the repository root.

```powershell
.\scripts\Build-Settings.ps1 -s -c
```

Always use both switches when building Settings. The script closes verified instances from this workspace, builds the app, places it at `settings-app/dist/blobui-settings.exe`, and starts it. It can use the workspace Rust toolchain under `.test-run/toolchain` when Cargo is not on PATH.

```powershell
.\scripts\Build-Installer.ps1
```

The installer output is `dist/blobui-installer.exe`. Read Testing and releases before running installation tests or packaging.

## Launch modes and diagnostics

Run the existing executable with `--background` to start its listener without changing login startup. Use `--onboarding` to open setup without first changing saved settings. The build script's `-n` switch requests onboarding on the next launch; combine it with the required build switches when building for an onboarding check.

Default diagnostic shortcuts are F8 to reopen full onboarding and F9 to simulate an available update without downloading or installing a release. They are configurable under Help > Keybinds. They help test the app and do not publish a release.

## Repository layout

| Area | Responsibility |
| --- | --- |
| `Dev` | Ordered source CSS modules. |
| `assets` | Bundled theme assets and default space banner. |
| `settings-app/ui` | Frontend, bundled articles, and Markdown renderer. |
| `settings-app/src` | Rust runtime, API, compilation, persistence, shortcuts, startup, and updates. |
| `installer/src` | Downloads, archive validation, installation, and transactional replacement. |
| `scripts` | Supported build, packaging, and installer-test entry points. |
| `docs` | Repository maintenance references. |

The frontend invokes Rust through Tauri. Rust performs API requests and saves directly without starting PowerShell. The compiler collects CSS under Dev, sorts it by relative path, substitutes validated values, and generates `custom.css` with bundled assets and local settings.

## Settings and credentials

The default data root is `%APPDATA%\anytype`. Local state lives beneath `blob` and generated `custom.css` lives in the data root. Appearance choices use `settingsV2` in `blob/data.json`. The first save migrates legacy font and accent choices; later saves use local settings rather than legacy Anytype Settings objects.

Credentials use Windows DPAPI for the current account. Replacement prepares a private encrypted temporary file and commits it after configuration and CSS operations succeed. Saves use transactional rollback and preserve managed assets and the installation manifest. Calendar refresh can fail independently of a valid connection; a saved cache supports later saves when the API is unavailable.

The runtime reads the root `theme` in Anytype's `devconfig.json` for light, dark, or system appearance. It does not read unrelated preferences.

## Workspace isolation

Keep source changes and fixtures inside the workspace. Do not modify release CSS or use a real Anytype installation as a fixture. A normal Settings build starts the app, so confirm its data folder before saving. Use the development installer and isolated folders for installation testing. Tests use fake keys and mock API responses rather than personal credentials.

Continue with [Theme & docs dev](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/theme-and-documentation-development.md) or [Testing and releases](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/testing-and-releases.md).
