# Anytype BlobUI

Anytype Blob UI is a custom css theme for Anytype which refreshes a variety of things.
It gives Anytype a more modern and elegant touch, coloring buttons, changing fonts, fixing bugs
and just improving the overall UI.
For more documentation, [visit our Anytype page](https://any.coop/A9drcF6fXs9QvXiR114HjF7Lx5ZzrysS1PLZcWFRYYW9Jiob/anytype-blobui)

On Windows, run `blobui-installer.exe` from a published release to install BlobUI. Use **Save** in BlobUI Settings to rebuild and apply the theme; runtime CSS compilation, Anytype API access, and settings saves are handled by the Rust app. Setup details are below.


#### *Made by Adrian Orlik (@PainterFlow11)*

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


## Installing BlobUI

### Windows installer

Download and run `blobui-installer.exe`. It downloads the `blobui-windows.zip` asset from the latest published GitHub release, then asks for the Anytype data folder and an empty BlobUI app folder. Existing theme settings and encrypted credentials are preserved, and existing CSS is backed up. Anytype must already have been opened once. Enable **File > Apply Custom CSS** in Anytype after installation.

The installer is a native Rust executable. It downloads and extracts the release, compiles the theme, and installs files directly without running PowerShell or setup scripts. It does not launch Settings automatically. Open BlobUI from the Start menu after installation. To reinstall while keeping settings, choose a new empty app folder.

Build the installer with Rust and the Windows C++ build tools. The build command also supports the workspace Rust toolchain used by the Settings build.

```powershell
.\scripts\Build-Installer.ps1
```

The result is `dist/blobui-installer.exe`. It fetches the newest release each time it runs, so theme releases do not require rebuilding it. PowerShell is used only to run the developer build command. The installer itself has no PowerShell or .NET runtime dependency.

Build an isolated development installer with `-d`.

```powershell
.\scripts\Build-Installer.ps1 -d
```

Run `dist/blobui-installer-dev.exe` to test the latest release. It installs only beneath `BlobUI-Installer-Test` beside the executable, uses a separate Anytype data folder, skips Start menu shortcuts, and leaves your normal installation alone. Repeated test installs retain the test settings and use a new app folder. This tests installation without applying the test theme to your running Anytype.

For an offline test of the current workspace, package the already built Settings executable and theme, then run the development installer against that ZIP.

```powershell
.\scripts\Build-Installer.ps1 -d -PackageRelease
.\dist\blobui-installer-dev.exe --package .\dist\blobui-windows.zip
```

If the Settings executable needs rebuilding, run `scripts/Build-Settings.ps1 -s -c` first. Packaging itself does not rebuild or launch Settings. The production installer accepts downloads from GitHub only.

The prepared **Release Windows installer** GitHub Actions workflow is currently ignored by Git until the release is ready. Once committed and pushed, it builds Settings, tests the installer, and publishes `blobui-installer.exe` and `blobui-windows.zip` as GitHub release assets. Merge development changes into `main`, then push a version tag on that commit matching `version` in `blobui-release.json`, prefixed with `v`. For example, version `1.2.0` uses tag `v1.2.0`. The workflow rejects tags whose commits are outside `main` or whose versions do not match.

You can retry an existing tag from **Actions > Release Windows installer > Run workflow** after the workflow reaches the default branch. Enter the existing tag in the `tag` field. The workflow creates a draft release when needed, uploads both tested files, then publishes it. A retry replaces the two assets on an existing mutable release. Immutable published releases cannot have their assets replaced; use a new version tag instead. No additional repository secret is required; uploads use GitHub's built-in token.

For manual packaging, run `scripts/Build-Installer.ps1 -PackageRelease` after building Settings and attach `dist/blobui-windows.zip` and `dist/blobui-installer.exe` to the GitHub release. GitHub's automatic source ZIP does not include the compiled app and cannot replace this asset. Until a release contains `blobui-windows.zip`, the online installer reports the missing package.

Run `scripts/tests/Test-Installer.ps1` to compile both installers and check isolated installation, repeated-install preservation, release selection, and archive validation. Tests use temporary folders inside the workspace and do not launch Settings.

Open **BlobUI** from the Start menu or run `blobui-settings.exe` from the app folder after installation. To remove BlobUI, open **Advanced Settings**, select the red **Uninstall BlobUI** button below Updates, then confirm. The uninstaller removes files recorded in BlobUI's ownership manifests and the generated `custom.css`, while keeping Anytype data, BlobUI settings, encrypted credentials, backups, custom banners, unrelated files, and the installer. If `custom.css` was edited after BlobUI generated it, uninstall stops and asks you to save a copy and restore the generated stylesheet first. The app runs directly in Rust. PowerShell is needed only for developer build and test commands.

### Optional: enable calendar color data from the Anytype API

Configure the Anytype API in BlobUI Settings onboarding or **Advanced Settings**. Create a read-only Anytype API v2 key and enter it in the masked field; BlobUI validates it, stores it using Windows DPAPI, and uses it only to retrieve Calendar Color data. No settings-space import or settings-object selection is required.


> [!NOTE]
> - BlobUI will not collect your data and send it to a server.
> - BlobUI will not modify your Anytype objects.
> - BlobUI reads Calendar Color data through the local Anytype API to build calendar CSS.
> - BlobUI will modify the CSS of your Anytype application, it will not affect saved data.

Enable **File > Apply Custom CSS** in Anytype. The API is optional; theme appearance settings work with API disabled.

## Updating BlobUI

Open **Advanced Settings** to check for updates, choose the Anytype data folder, change automatic update preferences, or uninstall BlobUI. Automatic checks run every five minutes while BlobUI Settings is running, including in the background. Automatic installation starts disabled. An available update also appears in Blob Pink below Undo and Save, above Logs. The manual check button changes to **Update now** or **You're up to date** after a successful check.

Updates use the latest published GitHub release's verified `blobui-installer.exe` asset. Settings downloads the installer, checks the release URL, size, and GitHub SHA-256 digest when available, then exits only after the installer has started. The installer pins the matching release tag, waits for Settings to close, and replaces managed app and theme files as one rollback-capable update. It preserves saved settings, encrypted credentials, updater preferences, custom banners, and local CSS files outside BlobUI's managed files. The installer executable stays in the app folder for future updates and uninstall. If handoff fails, Settings stays open; if installation fails, the installer reopens Settings with an error state and pauses automatic retries of that same release until the user retries or a newer release appears. Press **Ctrl+R** in Anytype after updating to reload CSS. **F9** creates a simulated update so you can test the controls without downloading or installing a release.

The update source is `PainterFlow/Anytype-BlobUI`. The repository and its releases must be publicly accessible. Private or inaccessible release sources show an error; the app does not store GitHub credentials.

When publishing a release, use a semantic version tag matching `version` in `blobui-release.json`, such as `v1.2.0`, and attach both `blobui-windows.zip` and `blobui-installer.exe`. The updater fetches the release for that exact tag after starting the handoff, so a newer release appearing mid-update cannot change the package being installed. Installations running an older Settings build need to run the latest `blobui-installer.exe` once to bootstrap installer-based Settings updates.

## Settings app

BlobUI Settings provides a desktop Appearance window for fonts and accent colors. Build it from the source folder with Rust and the Windows C++ build tools installed.

```powershell
.\scripts\Build-Settings.ps1 -s -c
.\settings-app\dist\blobui-settings.exe
```

The build places the executable in `settings-app/dist/blobui-settings.exe`; `-s` starts it after build and `-c` closes existing instances from this workspace first. `-n` records a one-time onboarding request for the next launch without starting the app. Use `-s -n` to start the app in onboarding immediately. Run the executable directly, or use `scripts/Build-Installer.ps1 -PackageRelease` to package it for the native installer.

Start Settings once per Windows session to enable **Numpad 1** while Anytype is active. In **Advanced > App Settings**, enable **Launch at startup** to open BlobUI Settings when you sign in. Closing the window keeps the app and shortcut running in the background. Press **Ctrl+Q** in Settings to exit completely, including before replacing the executable with a newer build. Opening the launcher again brings back the existing window. The shortcut supports Num Lock on or off and leaves keys in other apps unchanged.

The Settings sidebar has **Preferences** followed by **Accessibility**. Reduced Motion starts off, so BlobUI’s animated text effects run by default. Under **Hover Effects**, tab previews, link previews, and @ mention previews start off. `blob://` links always stay outside the hover preview settings. When link or @ mention previews are off, mouse clicks on those editor links or mentions are disabled to prevent Anytype’s delayed preview from appearing after the pointer leaves; existing keyboard behavior is unchanged. Change these switches, press **Save**, then reload Anytype with **Ctrl+R** to apply them.

On first launch, onboarding shows “Welcome to BlobUI”, then asks whether to enable the Anytype API. **No API Key** rebuilds the theme with API features disabled and opens Settings. **Use API Key** opens a setup guide with a masked API key field and optional IP/port (port defaults to `31009`). Create a read-only API v2 key in Anytype’s **Local API** settings. Press **Confirm** to validate API access, encrypt the key for the current Windows account, compile CSS, and open Settings. Calendar refresh runs separately: a calendar-data failure does not reject a valid key. Failed authentication or local save errors stay on the setup page for retry, with separate diagnostics. Rust handles the API request and key protection; the key stays out of command arguments and `data.json` and is stored using Windows DPAPI.

Credential saves prepare an encrypted temporary file and apply its private Windows permissions before changing existing configuration. The prepared file replaces the old key atomically as the final commit step. Save targets are checked first, and rollback avoids rewriting unchanged files.

Settings saves write `custom.css` and `blob/data.json`; credential replacement is the final transactional step. Managed assets and the install manifest are preserved. Save errors identify the failing file operation without forwarding keys or raw process output.

The choice, IP, and port are saved locally in `blob/data.json`. No API mode makes no Anytype API calls, removes the encrypted key and API-derived calendar colors, while custom accents, fonts, banners, text effects and UI fixes keep working. Settings saves respect the saved mode and connection address.

Use **Advanced Settings** to replace your key, change the port, or disable API features. **F8** restarts the full onboarding flow. Run `.\settings-app\dist\blobui-settings.exe --onboarding` to open onboarding without changing saved settings. Press **Escape** to leave without submitting; the Settings window uses the same entrance animation as when onboarding finishes. Reduced-motion preferences skip animations. Future pages can be added to the `onboardingSteps` list in `settings-app/ui/app.js`; conditional steps use `when`, and all pages share `advanceOnboarding()` and its fallback to Settings.

To start only the background listener, run `.\settings-app\dist\blobui-settings.exe --background`. This does not change the Windows login startup setting.

The first Settings app save migrates the current font and accent values from `data.json` into the local `settingsV2` record. After that, these local choices are authoritative, so saving no longer requires the legacy Anytype Settings objects. Anytype's Calendar Color data refreshes when its local API is available; otherwise the app keeps the saved calendar cache and reports that status. Saving rebuilds `custom.css`; press `Ctrl+R` in Anytype to apply it.

The app reads the root `theme` value in `%APPDATA%\anytype\devconfig.json` and follows Anytype's light, dark, or system mode, including the native window theme. It also applies the saved interface and code fonts from BlobUI's local settings to the settings window. The layout follows Anytype's flat preferences style and reads no other Anytype preferences.

## Dev Features

### Text Effects & Embeds

I'm currently working on implementing custom text effects and embeds with the help of the href attribute
The Base Idea is:
```css
/* All BlobUI markers inside object text */
.pageMain .editor .blockText .editable a[href^="blob://"] {
}

/* A specific text effect */
.pageMain .editor .blockText .editable a[href="blob://text.sfx/spoiler"] {
}

/* The block containing that marker */
.pageMain .editor .blockText:has(
  .editable a[href="blob://text.sfx/spoiler"]
) {
}
```
The current text effects are:
- `blob://text.sfx/spoiler`: Hides the marked text until you hover over its editable text block
- `blob://text.sfx/glitchy`: Creates a cool glitchy effect
- `blob://text.sfx/jumping`: Moves the marked text gently up and down
- `blob://text.sfx/rgb`: Displays a rainbow gradient that travels across the text
- `blob://text.sfx/neon`: Displays a neon gradient that travels across the text

Apply an effect by selecting text in Anytype and setting its link to the effect URL.
Spoilers appear as rounded blocks slightly brighter than the sidebar widgets, with a darker rounded background when revealed. Hovering anywhere in the same editable text block reveals all its spoilers; moving outside that block conceals them again. Links keep pointer events disabled to prevent Anytype hover previews. Keyboard focus also reveals an individual spoiler. Formatting fragments join seamlessly with rounded phrase ends. Each link fragment is an inline block, so line wrapping can differ from ordinary text.

## Installer validation

On Windows, build Settings with `scripts/Build-Settings.ps1 -s -c`, then run `powershell -NoProfile -File scripts/tests/Test-Installer.ps1`. The checks build both native installers and use isolated folders beneath `.test-run` to cover installation, backups, executable packaging, saved-setting preservation, rejected destinations, and failed-install cleanup. The runtime’s Rust tests cover CSS output, API behavior, transactional save rollback, and connection replacement using workspace-only fixtures. `settings-app/tests/native-runtime.test.py` checks runtime ownership and the native executable and developer build entry points.

Windows smoke check: build with `scripts/Build-Settings.ps1 -s -c`, run the production installer from a published release, choose the paths, launch BlobUI from the Start menu, save a setting, and reload Anytype. Verify custom folder paths containing spaces as well. Start menu integration, filesystem ACL preservation, and DPAPI require Windows for full validation.

Connection regression coverage is part of the native Rust tests: fake keys and a local mock Anytype server exercise replacement keys, optional calendar failures, custom IPs/ports, failed-authentication rollback, CSS failure, and API opt-out. The previous PowerShell process test has been replaced because the Settings runtime no longer starts PowerShell for API or save operations.
