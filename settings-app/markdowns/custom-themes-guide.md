Custom themes change Anytype with CSS. BlobUI reads theme files from `<BlobUI app folder>/Settings/themes/built-in-themes/`. It adds the selected preset to the CSS it writes for Anytype. On most Windows installs the folder is `%LOCALAPPDATA%\Programs\BlobUI\Settings\themes\built-in-themes\`.

## Create a theme file

1. Open the theme folder.
2. Create a `.css` file. Use ASCII letters, numbers, spaces, hyphens, underscores, and periods in its name. The name cannot start with a dot.
3. Start the file with this header.

```css
/*
author: Your name
name: My theme
*/
```

4. Add your CSS after the header and save the file as UTF-8.
5. Press Ctrl+R in BlobUI Settings. The theme appears in Preset Themes.
6. Enable Custom Themes, then select the theme in Preset Themes.
7. Adjust Based Theme and Main color if needed, then select Save.
8. Press Ctrl+R in Anytype.

`author` and `name` are required. `name` is the label shown in the app. Field order does not matter, but the header must be at the start of the file. If the theme does not appear, open Logs and look for theme errors.

## Write the CSS

BlobUI first creates a palette from Based Theme and Main color. It appends your preset after that palette, so your file can override generated variables.

After reloading custom themes, inspect `<BlobUI app folder>/Settings/themes/generated.css` for the generated variable names. Do not edit `generated.css`. BlobUI rewrites it.

Open `dark-mode-plus.css` and `oled.css` in the theme folder for working examples. Here is a small dark theme.

```css
/*
author: Your name
name: Soft dark
*/

html, html.themeDark {
  --color-bg-primary: #191918;
  --color-bg-primary-90: rgba(25, 25, 24, 0.9);
  --color-bg-secondary: #1c1c1b;
}
```

If `--color-bg-primary` is a literal three or six digit hex color, selecting the preset copies it into Main color. You can still change Main color before saving.

## Metadata fields

| Field | What it does |
| --- | --- |
| `author` | Required. Credits the theme author. |
| `name` | Required. Sets the label shown in Preset Themes. |
| `based-theme` | Optional. Accepts `Dark Mode` or `Light Mode`. BlobUI validates this field, but the Based Theme option in Settings chooses the generated palette. |
| `accent-color` | Optional. Overrides the user's accent color while this preset is selected. It accepts a hex or `rgb()`/`rgba()` color, an Anytype color name, Blob Pink, or `default`. |
| `accent-color-selection` | Optional. Use `true` or `false` to override whether selections use the accent color. The plural `accent-color-selections` also works in older themes. |

Unknown fields are ignored. Field values cannot be empty and cannot be longer than 128 bytes.

## Reload and troubleshoot

After editing an already selected theme, press Ctrl+R in BlobUI Settings. BlobUI rereads the file and rebuilds the saved CSS. Then press Ctrl+R in Anytype.

Open Logs if a theme does not appear or does not look right. BlobUI reports header and CSS problems with the file, line, and column. Files larger than 1 MB, links, and invalid filenames are not loaded. If the selected file is deleted, BlobUI falls back to the generated base theme.
