---
category: getting-started
index: 11
icon: updating
name: Updating
---

# Updating

## Check and install

1. Open Advanced Settings > App Settings and find Updates.
2. Select Check for updates.
3. After a successful check, the button changes to Update now or You're up to date.
4. If available, select Update now and allow the installer to finish.
5. When Settings reopens, read the release notes and reload Anytype with Ctrl+R.

An available update also appears as a pink sidebar button, below Undo and Save and above Logs.

## Automatic preferences

Check for updates automatically starts enabled. Checks run every five minutes while Settings runs, including in the background. Install updates automatically starts disabled. Change these controls under Updates, then press Save. Automatic installation waits while settings have unsaved changes. Quitting Settings stops background checks.

## What an update does

Settings uses the latest published release from PainterFlow/Anytype-BlobUI. It downloads the release's `blobui-installer.exe` and checks its GitHub URL, size, and SHA-256 digest when GitHub supplies one. Settings exits only after the installer starts successfully.

The installer pins that release tag, waits for Settings to close, and replaces managed app and theme files with rollback support. A newer release appearing during installation does not change the selected package. The installer remains in the app folder for future updates and uninstall.

Updates preserve saved settings, encrypted API credentials, update preferences, custom banners, and CSS files outside BlobUI's managed files. Generated `custom.css` is rebuilt; keep a separate copy of manual edits before updating. Settings reopens afterward.

## Recover from failure

If installer handoff fails, Settings stays open and reports the problem. If installation fails afterward, the installer reopens Settings with an error state. Automatic retries of that release pause until you retry or a newer release appears.

1. Read the update status and Logs.
2. Check internet access, write access to installation folders, and whether another process holds app files open.
3. Retry manually after resolving the reported problem.
4. If necessary, download the latest published installer and follow Installation and removal.

Release sources must be public. BlobUI does not store GitHub credentials, so private or inaccessible releases produce an error. Missing `blobui-installer.exe` or `blobui-windows.zip` assets need a corrected release from the maintainer.

## Upgrade an older installation

Settings builds from before installer-based app updates need the latest `blobui-installer.exe` run once to bootstrap this update path. Follow its folder prompts and keep the existing Anytype data folder to retain settings. Afterward, use the Updates controls.

See [Installation and removal](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/installation-and-removal.md) for folder choices and reinstalling.
