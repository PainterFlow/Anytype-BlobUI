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

## Dev Features

### Text Effects & Embeds

I'm currently working on implementing custom text effects and embeds with the help of the href attribute
The Base Idea is:
```css
/* All BlobUI markers inside object text */
.pageMain .editor .blockText .editable a[href^="blob://"] {
}

/* A specific text effect */
.pageMain .editor .blockText .editable a[href="blob://animated.txt"] {
}

/* The block containing that marker */
.pageMain .editor .blockText:has(
  .editable a[href="blob://animated.txt"]
) {
}
```