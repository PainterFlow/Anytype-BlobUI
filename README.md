# Anytype BlobUI

Anytype Blob UI is a custom css theme for Anytype which refreshes a variety of things.
It gives Anytype a more modern and elegant touch, coloring buttons, changing fonts, fixing bugs
and just improving the overall UI.
For more documentation, [visit our Anytype page](https://any.coop/A9drcF6fXs9QvXiR114HjF7Lx5ZzrysS1PLZcWFRYYW9Jiob/anytype-blobui)

On Windows, double-click `Install.cmd` to install BlobUI. Use **Save** in BlobUI Settings to rebuild and apply the theme; runtime CSS compilation, Anytype API access, and settings saves are handled by the Rust app. Setup details are below.


#### *Made by @PainterFlow11 - All rights reserved, no license*


## Installing BlobUI

1. Extract the download outside OneDrive and open Anytype at least once.
2. Double-click **Install.cmd**. Windows PowerShell 5.1 or later is required; administrator access is not needed for the default locations.
3. Choose the **Anytype data folder**, normally `%APPDATA%\anytype`. This is the folder containing Anytype's profile and `custom.css`, rather than the folder containing `Anytype.exe`.
4. Choose a separate, empty folder for the **BlobUI app**, normally `%LOCALAPPDATA%\Programs\BlobUI`. Press Enter to accept either default.
5. Open **BlobUI** from the Start menu, or run `BlobUI.cmd` in the app folder. Enable **File > Apply Custom CSS** in Anytype and press **Ctrl+R**.

Setup installs the default theme, its assets, the Rust Settings executable, and the PowerShell launch wrapper. Build the Settings executable first with `scripts/Build-Settings.ps1 -s -c`; setup refuses to install without it. It does not need an API key or settings-space import. The installed app can run after the extracted download is removed.

Existing theme settings and credentials are preserved. Existing `custom.css` is backed up in the Anytype data folder under `blob/backups`. Setup refuses non-empty app folders. To relocate the app, install into a new empty folder with `-NoShortcut`, then remove the old app folder and shortcut yourself.

You can also run setup from PowerShell with explicit paths:

```powershell
.\scripts\Setup.ps1 -AnytypeDirectory "C:\path\to\Anytype data" -AppDirectory "C:\path\to\BlobUI"
```

Pass `-NoShortcut` to skip the Start menu entry. Use the Settings app to update the theme. `Uninstall.ps1` remains temporarily outdated for the Rust runtime. Remove the app folder and BlobUI Start menu shortcut separately if you need to remove the launcher. Theme uninstallation also deletes `blob/backups`; copy out any CSS backup you want to keep first.

### Optional: enable calendar color data from the Anytype API

Configure the Anytype API in BlobUI Settings onboarding or **Advanced Settings**. Create a read-only Anytype API v2 key and enter it in the masked field; BlobUI validates it, stores it using Windows DPAPI, and uses it only to retrieve Calendar Color data. No settings-space import or settings-object selection is required.


> [!NOTE]
> - BlobUI will not collect your data and send it to a server.
> - BlobUI will not modify your Anytype objects.
> - BlobUI reads Calendar Color data through the local Anytype API to build calendar CSS.
> - BlobUI will modify the CSS of your Anytype application, it will not affect saved data.

Enable **File > Apply Custom CSS** in Anytype. The API is optional; theme appearance settings work with API disabled.

## Updating BlobUI

Open **Advanced Settings** to check for updates, choose the Anytype data folder, or change automatic update preferences. Automatic checks run every five minutes while BlobUI Settings is running, including in the background. Automatic installation starts disabled. An available update also appears in Blob Pink below Undo and Save, above Logs. The manual check button changes to **Update now** or **You're up to date** after a successful check.

Updates run in Rust and use the latest published GitHub release. They preserve saved settings, encrypted credentials, and local CSS files outside BlobUI's managed files. No installer or onboarding reset is required. Press **Ctrl+R** in Anytype after updating to reload CSS. **F9** creates a simulated update so you can test the controls without downloading or installing a release.

The update source is `PainterFlow/Anytype-BlobUI`. The repository and its releases must be publicly accessible. Private or inaccessible release sources show an error; the app does not store GitHub credentials.

When publishing a theme release, use a semantic version tag matching `BlobUiReleaseVersion` in `scripts/BlobUI.Core.psm1`, such as `v1.1.0`. The updater validates that version against GitHub's generated source archive before installing it.

This updater installs theme sources and assets. It does not replace the running Settings executable. App executable upgrades still require a separately built app. `scripts/Update.ps1` opens Settings and directs you to the Rust updater.

## Settings app

BlobUI Settings provides a desktop Appearance window for fonts and accent colors. Build it from the source folder with Rust and the Windows C++ build tools installed.

```powershell
.\scripts\Build-Settings.ps1 -s -c
.\scripts\Settings.ps1
```

The build places the executable in `settings-app/dist/blobui-settings.exe`; `-s` starts it after build and `-c` closes existing instances first. `-n` records a one-time onboarding request for the next launch without starting the app. Use `-s -n` to start the app in onboarding immediately. `Setup.ps1` packages the executable and `scripts/Settings.ps1` remains the supported PowerShell launch wrapper. The installer is responsible for installation; `Update.ps1` and `Uninstall.ps1` are temporarily outdated.

Start Settings once per Windows session to enable **Numpad 1** while Anytype is active. In **Advanced > App Settings**, enable **Launch at startup** to open BlobUI Settings when you sign in. Closing the window keeps the app and shortcut running in the background. Press **Ctrl+Q** in Settings to exit completely, including before replacing the executable with a newer build. Opening the launcher again brings back the existing window. The shortcut supports Num Lock on or off and leaves keys in other apps unchanged.

The Settings sidebar has **Preferences** followed by **Accessibility**. Reduced Motion starts off, so BlobUI’s animated text effects run by default. Under **Hover Effects**, tab previews, link previews, and @ mention previews start off. `blob://` links always stay outside the hover preview settings. When link or @ mention previews are off, mouse clicks on those editor links or mentions are disabled to prevent Anytype’s delayed preview from appearing after the pointer leaves; existing keyboard behavior is unchanged. Change these switches, press **Save**, then reload Anytype with **Ctrl+R** to apply them.

On first launch, onboarding shows “Welcome to BlobUI”, then asks whether to enable the Anytype API. **No API Key** rebuilds the theme with API features disabled and opens Settings. **Use API Key** opens a setup guide with a masked API key field and optional IP/port (port defaults to `31009`). Create a read-only API v2 key in Anytype’s **Local API** settings. Press **Confirm** to validate API access, encrypt the key for the current Windows account, compile CSS, and open Settings. Calendar refresh runs separately: a calendar-data failure does not reject a valid key. Failed authentication or local save errors stay on the setup page for retry, with separate diagnostics. Rust handles the API request and key protection; the key stays out of command arguments and `data.json` and is stored using Windows DPAPI.

Credential saves prepare an encrypted temporary file and apply its private Windows permissions before changing existing configuration. The prepared file replaces the old key atomically as the final commit step. Save targets are checked first, and rollback avoids rewriting unchanged files.

Settings saves write `custom.css` and `blob/data.json`; credential replacement is the final transactional step. Managed assets and the install manifest are preserved. Save errors identify the failing file operation without forwarding keys or raw process output.

The choice, IP, and port are saved locally in `blob/data.json`. No API mode makes no Anytype API calls, removes the encrypted key and API-derived calendar colors, while custom accents, fonts, banners, text effects and UI fixes keep working. Settings saves respect the saved mode and connection address.

Use **Advanced Settings** to replace your key, change the port, or disable API features. **F8** restarts the full onboarding flow. Run `.\scripts\Settings.ps1 -n` to open onboarding without changing saved settings. Press **Escape** to leave without submitting; the Settings window uses the same entrance animation as when onboarding finishes. Reduced-motion preferences skip animations. Future pages can be added to the `onboardingSteps` list in `settings-app/ui/app.js`; conditional steps use `when`, and all pages share `advanceOnboarding()` and its fallback to Settings.

To start only the background listener, run `.\scripts\Settings.ps1 -Background`. This does not change the Windows login startup setting.

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

On Windows, run `powershell -NoProfile -File scripts/tests/Test-Setup.ps1`. The checks use isolated folders beneath `scripts/tests` and cover installation, backups, launcher packaging, saved-setting preservation, rejected destinations, and failed-install cleanup. The runtime’s Rust tests cover CSS output, API behavior, transactional save rollback, and connection replacement using workspace-only fixtures. `settings-app/tests/native-runtime.test.py` checks runtime ownership and supported launcher/build entry points.

Windows smoke check: build with `scripts/Build-Settings.ps1 -s -c`, run `Install.cmd`, accept the paths, launch BlobUI from the Start menu, save a setting, and reload Anytype. Verify custom folder paths containing spaces as well. Start menu integration, filesystem ACL preservation, and DPAPI require Windows for full validation.

Connection regression coverage is part of the native Rust tests: fake keys and a local mock Anytype server exercise replacement keys, optional calendar failures, custom IPs/ports, failed-authentication rollback, CSS failure, and API opt-out. The previous PowerShell process test has been replaced because the Settings runtime no longer starts PowerShell for API or save operations.
