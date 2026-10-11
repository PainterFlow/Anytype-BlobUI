# Installing BlobUI

Run Install.ps1 from this folder in PowerShell. The first console line is Anytype Instillation.

The default choice uses %APPDATA%\anytype (normally C:\Users\{user}\AppData\Roaming\anytype). Choose M to enter a different existing Anytype data directory. If the detected directory is missing, the installer lets you enter a path manually.

The installer asks for an existing Anytype local API key using a hidden prompt. It checks the key against the local API, then lets you choose the space and one or more objects with the custom Setting type. When you select more than one object, later selections override values from earlier selections. The exported BlobUI settings use the text property keys font, code_font, accent_color, and banner.

The installer uses individual CSS files under Dev as source modules. It loads every CSS file below Dev in ordinal path order, validates declared setting placeholders, and writes the compiled custom.css outside Dev. The generated stylesheet uses the selected fonts and accent color. Its banner URL is an absolute file URI to the installed image, encoded by .NET so spaces and Unicode characters in the path work.

The installer copies packaged assets into the blob\assets folder under the selected Anytype data directory, including spacebanner\banner.png. It keeps the settings source, compiler, rebuild script, and assets under blob\runtime so rebuilds continue after the downloaded source folder moves. The normalized settings and successful install directory are saved atomically in blob\data.json. The API key is stored separately in blob\credentials\anytype-api-key.dpapi, encrypted for the current Windows user and protected by a user-only file ACL.

Before replacing an existing custom.css, the installer saves a timestamped copy under blob\backups. Repeated installations create another CSS backup and merge packaged assets into place without removing unrelated files. Releases\custom.css is not used.

To update BlobUI settings after installation, run blob\runtime\Rebuild.ps1. It uses the saved space and object IDs, fetches current values, compiles the CSS, and installs it without prompting for a directory. If fetching, validation, or compilation fails, the previous data.json and custom.css remain in place.

Anytype's documented refresh shortcut is Ctrl+R on Windows. If custom CSS is not active, select Menu > File > Apply Custom CSS in Anytype, then press Ctrl+R to load the installed stylesheet.
