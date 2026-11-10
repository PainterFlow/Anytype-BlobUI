# Anytype BlobUI

BlobUI is a custom CSS theme for Anytype with a Windows Settings app. Customize accents, fonts, space banners, calendar colors, text effects, and hover previews without editing CSS by hand.

Made by @PainterFlow11.

## Install on Windows

1. Install Anytype and open it once.
2. Download and run `blobui-installer.exe` from the [latest published release](https://github.com/PainterFlow/Anytype-BlobUI/releases/latest).
3. Select the Anytype data folder and a separate empty folder for the BlobUI app.
4. Open BlobUI from the Start menu and complete onboarding. The API key is optional.
5. In Anytype, enable File > Apply Custom CSS. Save your appearance choices in BlobUI, then reload Anytype with Ctrl+R.

## Developer guide

The in-app [Dev Guide](dev-guide.md) covers local setup, build commands, release notes, and the GitHub workflow.

## Build from source

On Windows with Rust and the Windows C++ build tools installed, run these commands from the repository root.

```powershell
.\scripts\Build-App.ps1 -s -c
.\scripts\Build-Installer.ps1
```

See the [Dev Guide](dev-guide.md) for local setup, build commands, and the GitHub workflow.

## License

Copyright (C) 2026 Adrian Orlik. Original BlobUI material is licensed under
GNU GPL v3 only with an additional author attribution term under section 7(b).
See [LICENSE](LICENSE) for the grant and attribution term and [COPYING](COPYING)
for the full GPL v3 text. Preserve the credit "BlobUI - original author Adrian Orlik"
in the covered material or its displayed legal notices when redistributing it.
Redistribution and commercial use are allowed under these terms; distributed
derivative works must comply with GPL v3, including its corresponding-source
requirements. Third-party material retains its own license terms.

Unless explicitly agreed otherwise, contributions submitted for inclusion in
BlobUI must be licensed under these same terms. Contributors retain copyright
in their contributions and must have permission to submit them.

## Anyblob integration

Anyblob connects Codex to Anytype through the running BlobUI Settings app. Its access and approval rules are configured per space and can be overridden for selected Codex project sources. Read the [plugin reference](codex-plugin/src/README.md) for the complete tool list.

## Background development launches

Build with `scripts/Build-App.ps1 -s -c -b` to launch Settings in the background after compilation. Without `-s`, `-b` queues background mode for the next launch without starting the app. The `--silent` spelling is also accepted. Add `-n` to show onboarding on the first foreground reveal. A normal second launch restores the existing window. A second launch with `--background` preserves its current visibility.
