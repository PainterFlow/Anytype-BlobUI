# Anytype BlobUI

Anytype Blob UI is a custom css theme for Anytype which refreshes a variety of things.
It gives Anytype a more modern and elegant touch, coloring buttons, changing fonts, fixing bugs
and just improving the overall UI.
For more documentation, [visit our Anytype page](https://any.coop/A9drcF6fXs9QvXiR114HjF7Lx5ZzrysS1PLZcWFRYYW9Jiob/anytype-blobui)

Run `.\scripts\Install.ps1` to install BlobUI or `.\scripts\Rebuild.ps1` to refresh its settings. Setup details are below.


#### *Made by @PainterFlow11 - All rights reserved, no license*


## Installing BlobUI


> [!NOTE]
> - BlobUI will not collect your data and send it to a server.
> - BlobUI will not modify your Anytype objects.
> - BlobUI will read your Anytype data and use it to dynamically build the CSS.
> - BlobUI will modify the CSS of your Anytype application, it will not affect saved data.

1. Download the correct BlobUI version for your Anytype.
2. Move the folder somewhere that is not your Onedrive and somewhere where you can find it again.
3. Create a space in Anytype for BlobUI's global settings.
4. Import the global settings archive from **BlobUI Settings Space** into that space. It provides the reusable `Setting` type and the global settings object.
5. Open your Anytype settings and create a new API key.
	- Make sure it's a V2 API key.
	- Make sure it doesn't expire.
	- Give it access to the global settings space and every space whose settings BlobUI should read.
	- Make sure the API key only has read only access.
	- Make sure the PORT is 31009.
	- Copy the newly created API key.
6. Enable "Apply Custom CSS" in File(Menu Bar) -> Apply Custom CSS
7. Open the Terminal and go to your directory where you put the download.
8. Run .\scripts\install.ps1
	- If you didn't pick a custom instillation, just press A and Enter.
	- Paste your Anytype API key.
9. In Anytype, press Ctrl/Cmd + R to reload.

BlobUI finds the global settings object by the exact text in its `BlobID` property, not by object name. Set it to `BlobUI-global-settings` in the settings space. Duplicate marked settings objects stop the rebuild with a diagnostic. If no global object exists, BlobUI uses its built-in font and color settings. The banner always uses the fixed `assets/spacebanner/banner.png` file. Older imported banner properties are ignored, and BlobUI preserves an installed `banner.png` during updates and uninstall.

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

The updater reads the saved API key from its encrypted local file and rediscovers settings by their `BlobID` markers. You do not need to import the settings again unless you removed them. The updater does not download releases or change objects in Anytype. Older installations without `BlobID` markers continue to refresh their saved object IDs.

To apply changes made in your BlobUI settings objects without installing a newer release, run `.\scripts\Rebuild.ps1` from your current BlobUI source folder, then reload Anytype with `Ctrl+R`. You can also pass `-InstallDirectory` to the rebuild script for a custom data directory.

If the updater reports a missing installation or settings space, check the data directory passed to `-InstallDirectory` and rerun `Install.ps1` to choose the space and save a valid API key.

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
