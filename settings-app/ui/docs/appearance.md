---
category: getting-started
parent: features
index: 4
icon: features
name: Appearance
---

# Appearance

Preferences controls for accents, fonts, and the space banner work without an API key. Settings keeps edits as a draft until you save. Save rebuilds `custom.css`; press Ctrl+R in Anytype to apply it. Undo changes discards the draft after confirmation.

## Accents and selections

In Preferences, use the accent picker, hue control, or Hex or CSS color field. Anytype Accent Color follows Anytype's accent. BlobUI applies accents to controls including dates, properties, dataview hover states, toggles, publishing, and membership controls.

Include selections is the first switch inside the Accent color box, above Anytype Accent Color. It starts on, including after upgrades. It applies the chosen accent to text and code selections, table ranges and handles, selected cell menus, selected blocks and records, drag selection and drop indicators, selected choices, and blue focus outlines, including inline and menu text-property editors. Selected text inside BlobUI Settings also follows the switch and current accent as you edit. Ordinary text-colored typing carets keep their current color. Turn it off to restore default selection styling while keeping other BlobUI accents. Save and reload Anytype to apply the change there.

## Fonts

Set the interface and code fonts separately using fonts installed on your computer. Each field accepts a font family name up to 80 letters, numbers, spaces, underscores, or hyphens, beginning with a letter or number. When no font has been saved, the defaults are Inter for the interface and Consolas for code. Saved fonts also apply to Settings. The app follows the root theme setting in Anytype's `devconfig.json` for light, dark, and system appearance.

## Space banner

1. Under Space banner, select Choose image.
2. Choose a PNG, JPEG, or WebP file no larger than 2 MB. Each dimension must be at most 8192 pixels, and the image must not exceed 20 megapixels. Inspect the sidebar preview.
3. Press Save and reload Anytype.

Use default selects the bundled banner for the next save. The preview is static; verify the actual crop in Anytype. Settings reports rejected files without replacing your saved choice. Wait for an image to finish loading before saving. Undo changes discards an unsaved banner choice after confirmation.
