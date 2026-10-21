---
category: getting-started
index: 2
icon: installation
name: Installation and removal
---

# Installation and removal

## Before installing

Install Anytype on Windows and open it once so its data folder exists. Download BlobUI from [GitHub releases](https://github.com/PainterFlow/Anytype-BlobUI/releases/latest). The installer needs internet access to download the published Windows package. GitHub's automatic source ZIP is not an installable app package.

The usual Anytype data folder is `%APPDATA%\anytype`. It contains `custom.css` and BlobUI's `blob` folder. This differs from the empty application folder where you install `blobui-settings.exe`.

## Install BlobUI

1. Download and run `blobui-installer.exe` from a published release.
2. Select your Anytype data folder when prompted.
3. Select an empty folder for the BlobUI app.
4. Wait for installation to finish. It downloads `blobui-windows.zip`, installs the runtime, generates CSS, and creates the launcher.
5. Open BlobUI from the Start menu. The installer does not launch Settings automatically.
6. In Anytype, enable File > Apply Custom CSS and press Ctrl+R.

Existing settings and encrypted credentials are preserved, and existing CSS is backed up. If installation reports a missing package, check that the release contains `blobui-windows.zip`. If the app destination is rejected, choose a new empty folder.

## Complete onboarding without an API key

1. Open Settings and continue from Welcome to BlobUI.
2. Select No API Key.
3. Wait for BlobUI to rebuild the theme and open Settings.
4. Choose appearance settings, press Save, and reload Anytype with Ctrl+R.

Fonts, accents, banners, text effects, and UI styling work without API access. Calendar colors derived from Anytype properties require a key.

## Connect the Anytype API

1. In Anytype's Local API settings, create a read-only API v2 key.
2. In onboarding, select Use API Key and follow the setup guide.
3. Enter the key in the masked field. The default API address is `127.0.0.1` and port is `31009`.
4. Press Confirm. BlobUI validates access, encrypts the key for your Windows account, rebuilds CSS, and opens Settings.
5. Reload Anytype after saving your appearance choices.

No settings-space import is required. A calendar-data failure does not reject a valid key. Authentication and local save errors keep setup open for correction and retry. Check that Anytype is running, its Local API is enabled, and the key and connection address match.

## Change or remove the connection

1. Open Advanced Settings > API key.
2. Enter a replacement key or change API key IP and API key port.
3. Save and check the result before reloading Anytype.

Clear the masked key and confirm removal to disable API access. BlobUI removes the encrypted key and API-derived colors. Failed connection replacement leaves the saved configuration intact. Keys use Windows DPAPI and are stored separately from `blob/data.json`. Do not put keys in logs or issue reports.

## Reinstall and change folders

To reinstall while keeping settings, run the release installer and select a new empty app folder with the existing Anytype data folder. For ordinary upgrades, use [Updating](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/updating.md).

Advanced Settings > App Settings > Anytype Folder chooses the data folder containing `custom.css`. Verify the selected folder and press Save. Back up settings, custom banner files, and manually edited CSS before reorganizing an installation. Keep the distinction between the app folder and Anytype's data folder.

## Uninstall BlobUI

1. Open Advanced Settings > App Settings.
2. Select the red Uninstall BlobUI button below Updates.
3. Read the confirmation and confirm removal.
4. Reload Anytype and check that BlobUI styling is gone.

Uninstall removes files recorded in BlobUI's ownership manifests and generated `custom.css`. It retains Anytype data, BlobUI settings, encrypted credentials, backups, custom banners, unrelated files, and the installer. It does not erase your Anytype objects.

If `custom.css` changed after BlobUI generated it, uninstall stops to protect those edits. Save a separate copy first. Restore BlobUI's generated stylesheet by rebuilding with Alt+R while Anytype is active and Settings is running, or change an appearance setting and press Save. Then retry uninstall. Keep the copied stylesheet to recover manual changes. BlobUI does not automatically restore an earlier custom stylesheet; use your backup if needed.
