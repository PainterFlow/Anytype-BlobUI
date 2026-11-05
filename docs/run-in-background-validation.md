# Run in background validation

Validation ran on Windows on 2026-11-05. All app profiles, installation data, credentials and debugger files used workspace `.test-run` fixtures. The user's Anytype instance was not automated or restarted.

## Implemented behavior

The title-bar background action, Close and background launch create one tray icon and hide the existing window. Open Settings restores that window and removes the tray. Quit BlobUI uses the existing guarded exit path. Window and tray transitions run together on the UI thread to prevent shortcut restore races.

Advanced Settings > Experiments contains a separate Run in background experiment with Disabled plus the Normal and Autostart Background variants. The shared dropdown follows the older Resizable Sidebar experiment, with Disabled, Variant 1 and Variant 2. Gray text below each menu option explains its behavior. Disabled is the default. Normal enables the title-bar action. Autostart Background also enables the startup picker in App settings. The startup setting stays in App settings, with its standard switch for the other variants. Save and Undo include the experiment choice. The startup picker uses the Windows Run entry and supports off, foreground and background modes. Save captures the registry value's original type and bytes for rollback. Ownership checks accept only supported executable paths and command forms. Installer cleanup preserves unrelated or malformed values.

Build-App accepts `-Silent` and `--silent`, requires `-s`, and forwards `--background`. Explicit onboarding waits for initial frontend loading, event subscription and a foreground reveal. Background second launches preserve visibility.

## Checks

The final experiment correction passed four focused frontend test files and the native three-mode persistence test. These cover previews, experiment and startup Save/Undo, retained startup modes across variant changes, and startup controls staying in App settings. The dark fixture checks the shared experiment dropdown and gray descriptions without horizontal overflow.

- The isolated native desktop fixture used the separate identifier `io.blobui.settings.background-fixture`, a private WebView profile, private APPDATA and LOCALAPPDATA, an isolated Anyblob pipe and API-disabled data.
- Background startup remained hidden after initialization. Three hide and restore cycles retained the same native window and draft input. A normal second launch restored Settings. Background second launches preserved its visibility.
- Explicit onboarding stayed pending while hidden, including a request delivered by a second launch. It appeared once on the next foreground reveal.
- The real Windows tray menu contained Open Settings and Quit BlobUI. Open restored the same window and removed the sole tray icon. Quit exited the fixture. Native menu messages targeted only the fixture process.
- Seven startup mode and ownership tests passed. Installer ownership tests and installer compilation passed. Registry parsing tests include malformed strings, hidden trailing commands and unrelated executable paths. Tests did not change the user's Run entry or exercise a real Windows sign-in.
- The isolated Rust suite passed 228 tests with two ignored and one filtered. The filtered, unchanged `release_metadata_validates_schema_and_semantic_version` test expects version 1.2.0 while repository metadata contains 1.3.0. The initial sandboxed run lacked localhost access and failed local API fixtures. Those passed with localhost access and isolated environment variables.
- The frontend suite passed 93 tests and the shared controls suite passed two tests. A final targeted regression confirms native background errors appear in Logs.
- Build argument fixtures passed for normal launch, both silent spellings, silent onboarding and rejection of silent without `-s`.
- Both branch ancestry checks passed. `origin/main` is an ancestor of `dev`, and `dev` is an ancestor of the active branch.

The native Anytype foreground shortcut was covered by its existing unit tests and shared restore function. It was not exercised against the user's running Anytype app. Tray creation failure recovery was reviewed and its frontend reporting was tested, without forcing a shell failure.

## Performance sample

The same isolated release fixture ran all three measurements with a 3.5 second settling period and roughly 8.5 seconds of sampling. The previous hidden behavior was represented by hiding the window directly through Win32, without a tray. This is an equivalent hide operation in the new binary, rather than a separately built historical version. Values include BlobUI and the five WebView processes reported by the browser debugger.

| Mode | CPU, one core | Working set MiB | Private memory MiB |
| --- | --- | --- | --- |
| Visible | 0.37% | 413.30 | 221.21 |
| Hidden without tray | 0.55% | 418.18 | 226.17 |
| Background with tray | 0.52% | 308.96 | 235.78 |

This single short sample does not establish a CPU or memory improvement. The background mode keeps the WebView alive to preserve drafts and shortcut behavior. Working-set trimming and sampling order can affect memory readings.

## Build handoff

The required build command ran with `BLOBUI_BUILD_ONLY=1` and `scripts/Build-App.ps1 -s -c`. Initial distribution copies encountered a file lock, without stopping any process. Later builds compiled successfully and updated `settings-app/dist/blobui-settings.exe`. The separate fixture executable stays under `.test-run/background-native`.

The final dropdown build compiled successfully. Its distribution copy was blocked again because settings-app/dist/blobui-settings.exe was in use. The final compiled executable is settings-app/target/release/blobui-settings.exe. No process was stopped.
