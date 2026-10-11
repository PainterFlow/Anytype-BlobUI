# Anytype BlobUI

Anytype Blob UI is a custom css theme for Anytype which refreshes a variety of things.
It gives Anytype a more modern and elegant touch, coloring buttons, changing fonts, fixing bugs
and just improving the overall UI.
For more documentation, [visit our Anytype page](https://any.coop/A9drcF6fXs9QvXiR114HjF7Lx5ZzrysS1PLZcWFRYYW9Jiob/anytype-blobui)

Run `.\scripts\Install.ps1` to install BlobUI or `.\scripts\Rebuild.ps1` to refresh its settings. Setup details are below.


#### *Made by @PainterFlow11 - All rights reserved, no license*

## Installing BlobUI

Run `.\scripts\Install.ps1` from this folder in PowerShell. The first console line is Anytype Instillation.

BlobUI reads settings through the [Anytype API v2](https://developers.anytype.io/docs/reference/v2/anytype-api/), which Anytype currently marks as pre-release. Anytype must be running a build that exposes v2 through its local API at 127.0.0.1:31009. V2 routes start with /v2 and do not use the Anytype-Version header. If Anytype returns HTTP 404 for a v2 route, check that the installed build supports API v2 and that the selected space and settings objects still exist.

Use an API key with read access to the space that contains the BlobUI settings. The installer lists spaces and reads selected objects with GET requests. It discovers Setting objects with POST /v2/spaces/{space_id}/search, a read-only search request, only in the designated settings space. Calendar Color discovery checks every space this API key can access.

The default choice uses %APPDATA%\anytype (normally C:\Users\{user}\AppData\Roaming\anytype). Choose M to enter a different existing Anytype data directory. If the detected directory is missing, the installer lets you enter a path manually.

The installer asks for an existing Anytype local API key using a hidden prompt. It checks the key against the local API, then lets you choose the space and one or more objects with the Setting type key. When you select more than one object, later selections override values from earlier selections. API v2 returns an [AnyBlock v2 document](https://pkg.go.dev/github.com/anyproto/any-block/format/v2) whose properties are a name-to-value map. BlobUI maps the Font, Code Font, and Accent Color properties to its internal font, code_font, and accent_color settings. Accent Color accepts three or six digit hex colors, valid `rgb()` or `rgba()` values, and Anytype palette names. The names are White, Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, Green, and Blob Pink. Blob Pink selects `#DE50DE`. To choose by name in Anytype, make Accent Color a Select property and add the names you want as options; a text property also works.

The installer uses individual CSS files under Dev as source modules. It loads every CSS file below Dev in ordinal path order, validates declared setting placeholders, and writes the compiled custom.css outside Dev. The generated stylesheet uses the selected fonts and accent color. Its banner URL is an absolute file URI to the installed image, encoded by .NET so spaces and Unicode characters in the path work. To customize the banner, replace `assets/spacebanner/banner.png` and run `.\scripts\Rebuild.ps1`.

BlobUI also reads the `Calendar Color` property from objects across every space accessible to the API key. Each space may use its own property key for the `Calendar Color` display name. It accepts three or six digit hex colors, valid `rgb()` or `rgba()` values, and the Anytype palette names Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, Green, or Blob Pink. `Default` leaves the event unchanged. Use a Select property for palette names or a text property for hex and RGB values. Each install or rebuild refreshes the object-ID-to-color map and compiles calendar record selectors into custom.css; removing the property or choosing Default clears the corresponding override. After rebuilding, press Ctrl+R in Anytype to reload the stylesheet.

The installer copies packaged assets into the blob\assets folder under the selected Anytype data directory, including spacebanner\banner.png. It keeps the scripts under blob\runtime\scripts and CSS and assets beside that folder so rebuilds continue after the downloaded source folder moves. The normalized settings and successful install directory are saved atomically in blob\data.json. The API key is stored separately in blob\credentials\anytype-api-key.dpapi, encrypted for the current Windows user and protected by a user-only file ACL. A source-folder install also saves the selected Anytype directory in the ignored .blobui-install.json file so the source rebuild script can find a manually selected location.

Each successful install replaces the existing custom.css and saves no backup files. The installer keeps the previous CSS and data in memory during the operation so it can restore them if a later install step fails. Packaged assets merge into place without removing unrelated files. Releases\custom.css is not used.

To update BlobUI settings after installing from this source folder, run `.\scripts\Rebuild.ps1` there. It uses the saved source-folder install location, fetches current values through API v2, compiles the current Dev CSS and assets, and installs without prompting. You can pass `-InstallDirectory 'C:\path\to\Anytype data'` to select a different existing installation. If the source-folder pointer is missing, Rebuild checks the standard `%APPDATA%\anytype` location for a valid older installation. For an installation at a custom path without a pointer, pass `-InstallDirectory` once or run `.\scripts\Install.ps1` again.

The copied `blob\runtime\scripts\Rebuild.ps1` remains available after the downloaded source folder moves. It uses the data and encrypted credential beside that runtime copy, along with the CSS and assets copied during installation. Both entry points use saved full space and object IDs, fetch current values through API v2, and install without prompting for a directory. Existing data.json files from the previous API version keep their schema and IDs and can be refreshed through v2. If fetching, validation, or compilation fails, the previous data.json and custom.css remain in place.

To remove BlobUI from a source checkout, run `.\scripts\Uninstall.ps1`. It uses `.blobui-install.json`, or checks `%APPDATA%\anytype` when no pointer exists and its `blob\data.json` confirms that directory. Pass `-InstallDirectory 'C:\path\to\Anytype data'` to select a custom directory without a source pointer. The installed copy at `blob\runtime\scripts\Uninstall.ps1` selects the Anytype directory that contains its runtime. The uninstaller deletes that directory's root `custom.css` and entire `blob` folder, including credentials, runtime scripts, assets, staging files, and any old backups. It does not need the API key or saved settings data when you select a directory explicitly. Other files in the Anytype directory remain in place, and custom.css files in a `Releases` folder are not removed. A source uninstall removes `.blobui-install.json` only when it points to the directory it just uninstalled.

Anytype's documented refresh shortcut is Ctrl+R on Windows. If custom CSS is not active, select Menu > File > Apply Custom CSS in Anytype, then press Ctrl+R to load the installed stylesheet.
