# Anytype BlobUI

Anytype Blob UI is a custom css theme for Anytype which refreshes a variety of things.
It gives Anytype a more modern and elegant touch, coloring buttons, changing fonts, fixing bugs
and just improving the overall UI.
For more documentation, [visit our Anytype page](https://any.coop/A9drcF6fXs9QvXiR114HjF7Lx5ZzrysS1PLZcWFRYYW9Jiob/anytype-blobui)

On Windows, double-click `Install.cmd` to install the BlobUI app and CSS. Use `.\scripts\Rebuild.ps1` to refresh the theme. Setup details are below.


#### *Made by @PainterFlow11 - All rights reserved, no license*


## Installing BlobUI

1. Extract the download outside OneDrive and open Anytype at least once.
2. Double-click **Install.cmd**. Windows PowerShell 5.1 or later is required; administrator access is not needed for the default locations.
3. Choose the **Anytype data folder**, normally `%APPDATA%\anytype`. This is the folder containing Anytype's profile and `custom.css`, rather than the folder containing `Anytype.exe`.
4. Choose a separate, empty folder for the **BlobUI app**, normally `%LOCALAPPDATA%\Programs\BlobUI`. Press Enter to accept either default.
5. Open **BlobUI** from the Start menu, or run `BlobUI.cmd` in the app folder. Enable **File > Apply Custom CSS** in Anytype and press **Ctrl+R**.

Setup installs the default theme, its assets, a local Windows app launcher, and the maintenance scripts. It does not need an API key or settings-space import. The app currently provides a **Rebuild CSS** button; onboarding will be added separately. The installed app can run after the extracted download is removed.

Existing theme settings and credentials are preserved. Existing `custom.css` is backed up in the Anytype data folder under `blob/backups`. Setup refuses non-empty app folders; use the theme updater below for theme updates. To relocate the app, install into a new empty folder with `-NoShortcut`, then remove the old app folder and shortcut yourself.

You can also run setup from PowerShell with explicit paths:

```powershell
.\scripts\Setup.ps1 -AnytypeDirectory "C:\path\to\Anytype data" -AppDirectory "C:\path\to\BlobUI"
```

Pass `-NoShortcut` to skip the Start menu entry. Run `scripts\Uninstall.ps1 -InstallDirectory "C:\path\to\Anytype data"` to remove the theme. Remove the app folder and the BlobUI Start menu shortcut separately to remove the launcher. Theme uninstallation also deletes `blob/backups`; copy out any CSS backup you want to keep first.

### Optional: configure settings through the Anytype API

The original API configuration flow is still available through `scripts\Install.ps1` inside the installed app. This replaces default mode with your selected settings objects. After completing it, **Rebuild CSS** refreshes settings through the API. The following steps apply only to this optional configuration:


> [!NOTE]
> - BlobUI will not collect your data and send it to a server.
> - BlobUI will not modify your Anytype objects.
> - BlobUI will read your Anytype data and use it to dynamically build the CSS.
> - BlobUI will modify the CSS of your Anytype application, it will not affect saved data.

1. Download the correct BlobUI version for your Anytype.
2. Move the folder somewhere that is not your Onedrive and somewhere where you can find it again.
3. Create a new space in Anytype called: "BlobUI - Settings".
4. Import the Anytype Objects from BlobUI Settings Space -> (the zip file should be here) (It should be included in your download) in that new space.
	- Select Anytype for importing
5. Open your Anytype settings and create a new API key.
	- Make sure it's a V2 API key.
	- Make sure it doesn't expire.
	- Give it access to at least the newly created space, if you want all features to work everywhere, then give it access to all spaces.
	- Make sure the API key only has read only access.
	- Make sure the PORT is 31009.
	- Copy the newly created API key.
6. Enable "Apply Custom CSS" in File(Menu Bar) -> Apply Custom CSS
7. Open the Terminal and go to your directory where you put the download.
8. Run .\scripts\install.ps1
	- If you didn't pick a custom instillation, just press A and Enter.
	- Paste your Anytype API key.
9. In Anytype, press Ctrl/Cmd + R to reload and it should be done.

## Updating BlobUI

1. Download the newer BlobUI release for your Anytype version and extract it to a folder outside OneDrive.
2. Keep Anytype open with its API enabled on port `31009`. The API key you used during installation must still have read access to your BlobUI settings space.
3. Open PowerShell in the newly extracted BlobUI folder and run the updater.

   ```powershell
   .\scripts\Update.ps1
   ```

   If you use a custom Anytype data directory and the updater cannot find your installation, specify the folder that contains `custom.css` and `blob`.

   ```powershell
   .\scripts\Update.ps1 -InstallDirectory "C:\path\to\Anytype"
   ```

   The default Anytype data directory on Windows is `%APPDATA%\anytype`.
4. Wait for the update to complete, then switch to Anytype and press `Ctrl+R` to reload the stylesheet. If the theme does not appear, enable **Apply Custom CSS** in the **File** menu and reload again.

The updater reads the saved API key from its encrypted local file and refreshes the settings objects you selected during installation. You do not need to import the BlobUI Settings Space again. The updater does not download releases or change objects in Anytype.

To apply changes made in your BlobUI settings objects without installing a newer release, run `.\scripts\Rebuild.ps1` from your current BlobUI source folder, then reload Anytype with `Ctrl+R`. You can also pass `-InstallDirectory` to the rebuild script for a custom data directory.

If the updater reports a missing installation or saved settings, check the data directory passed to `-InstallDirectory`. If it reports an invalid API key or missing settings objects, run `.\scripts\Install.ps1` again to save a valid key and select your settings objects.

## Settings app

BlobUI Settings provides a desktop Appearance window for fonts and accent colors. Build it from the source folder with Rust and the Windows C++ build tools installed.

```powershell
.\scripts\Build-Settings.ps1
.\scripts\Settings.ps1
```

The build places the executable in `settings-app/dist/blobui-settings.exe`. `Install.ps1` and `Update.ps1` include that executable when it is present, placing it at `blob/runtime/settings-app.exe`; the installed launcher is `blob/runtime/scripts/Settings.ps1`. A release that ships the app should include the built executable at the same `settings-app/dist` path before packaging.

Start Settings once per Windows session to enable **Numpad 1** while Anytype is active. Closing the window keeps the app and shortcut running in the background. Press **Ctrl+Q** in Settings to exit completely, including before replacing the executable with a newer build. Opening the launcher again brings back the existing window. The shortcut supports Num Lock on or off and leaves keys in other apps unchanged.

While the Settings window is focused, press **F8** to preview the onboarding welcome screen. “Hello” and “Press anywhere to continue” fade in; click anywhere on the welcome screen or press Enter/Space to return to Settings. F8 can reopen the preview without changing your settings. Reduced-motion preferences skip the animation.

To start only the background listener, run `.\scripts\Settings.ps1 -Background`. This does not register the app to start automatically with Windows.

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

On Windows, run `powershell -NoProfile -File scripts/tests/Test-Setup.ps1`. The checks use isolated folders beneath `scripts/tests` and cover default installation, backups, launcher packaging, rebuild without API access, saved-setting preservation, rejected destinations, and failed-install cleanup. Run `scripts/tests/Test-BlobUI.ps1` for the existing API and maintenance workflow checks.

Windows smoke check: run `Install.cmd`, accept the paths, launch BlobUI from the Start menu, click **Rebuild CSS**, and reload Anytype. Verify custom folder paths containing spaces as well. Windows Forms, Start menu COM integration, filesystem ACL preservation, and DPAPI require Windows for full validation.
