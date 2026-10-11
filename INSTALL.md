# Installing BlobUI

Run `Install.ps1` from this folder in PowerShell. The first console line is `Anytype Instillation`.

The default choice uses `%APPDATA%\anytype` (normally `C:\Users\{user}\AppData\Roaming\anytype`). Choose `M` to enter a different existing Anytype data directory. If the detected directory is missing, the installer returns to the location prompt so a manual path can be entered.

The installer uses `Dev\custom.css` and copies the `assets` directory into `<Anytype data directory>\blob\assets`, preserving its folders. The space banner is installed at `blob\assets\spacebanner\banner.png`; the installed CSS uses a relative URL to that file, so it does not contain a user-specific path. `Releases\custom.css` is not used.

Before replacing an existing `custom.css`, the installer saves a timestamped copy under `blob\backups`. Repeated installations create a separate CSS backup each time and merge the packaged assets into place without removing unrelated files.
