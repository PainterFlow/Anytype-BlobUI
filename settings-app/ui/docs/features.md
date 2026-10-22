---
category: getting-started
index: 3
icon: features
name: Features
---

# Features

## Apply appearance changes

1. Open BlobUI Settings from the Start menu, or press Numpad 1 while Anytype is active and Settings is running.
2. Change Preferences or Accessibility controls. Changes remain a draft until Save.
3. Press Save to write local settings and rebuild `custom.css`.
4. Press Ctrl+R in Anytype to reload the stylesheet.

Undo changes discards the unsaved draft after confirmation. Save errors appear in Settings; check Logs for diagnostics. API connection changes and app controls have their own persistence behavior.

## Accents and fonts

In Preferences, use the accent picker, hue control, or Hex or CSS color field. The native accent option follows Anytype's accent. BlobUI applies accents to controls including dates, properties, dataview hover states, toggles, publishing, and membership controls.

Include selections is the first switch inside the Accent color box, above Anytype Accent Color. It starts on, including after upgrades. It applies the chosen accent to text and code selections, table ranges, selected blocks and records, drag selection and drop indicators, selected choices, and blue focus outlines. Ordinary text-colored typing carets keep their current color. Turn it off to restore Anytype selection styling while keeping other BlobUI accents. Save and reload Anytype to apply the change.

Set the interface and code fonts separately using fonts available on your computer. Saved fonts also apply to Settings. The app follows the root theme setting in Anytype's `devconfig.json` for light, dark, and system appearance.

## Kanban views

In Preferences, Board layout applies to inline and full-page boards. Default keeps Anytype's layout. Balanced reduces card and column spacing. Compact also narrows columns to 220px and places properties on shared rows that wrap inside the card, while keeping each title on its own row.

Collapsed Categories works with every style. It shows rounded category headers until you hover over a column or move keyboard focus into it. Cards expand and collapse with a short animation, disabled by system or BlobUI Reduced Motion settings. Devices without hover keep the cards visible. The initial settings are Default with Collapsed Categories off. Save and reload Anytype to apply changes, or use Undo changes to discard a draft.

## Space banner

1. Under Space banner, select Choose image.
2. Choose a PNG, JPEG, or WebP file no larger than 2 MB and inspect the sidebar preview.
3. Press Save and reload Anytype.

Use default restores the bundled banner after saving. The preview is static; verify the actual crop in Anytype. A rejected image shows an error without replacing your saved choice.

## Calendar colors

Calendar colors require the optional Anytype API connection. In Anytype, give relevant objects a property named Calendar Color. Supported values include Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, Green, Blob Pink, three- or six-digit hex colors, and validated rgb/rgba colors. A space must not contain multiple properties with that name.

Save in Settings to refresh calendar data and rebuild CSS, then reload Anytype. Alt+R in Anytype also requests a rebuild while Settings is running. If refresh fails temporarily, BlobUI keeps the saved calendar cache and reports the failure. Disabling API access removes API-derived colors while other appearance settings remain available. API setup is covered in Installation and removal.

## Text effects

1. Select text in an Anytype object.
2. Set its link to one of the URLs below.
3. View it with BlobUI custom CSS enabled.

| URL | Effect |
| --- | --- |
| `blob://text.sfx/spoiler` | Conceals text until its editable block is hovered. |
| `blob://text.sfx/glitchy` | Applies a glitch animation. |
| `blob://text.sfx/jumping` | Moves text up and down. |
| `blob://text.sfx/rgb` | Animates a rainbow gradient. |
| `blob://text.sfx/neon` | Animates a neon gradient. |

Hovering within the same editable block reveals its spoilers together. Keyboard focus also reveals an individual spoiler. Fragments use inline blocks, so wrapping can differ from ordinary text. These are visual effects, not access controls; concealed text remains in the object.

## Accessibility and previews

Reduced Motion starts off. Turn it on to suppress BlobUI's animated text effects. Settings also supports reduced-motion transitions.

Under Hover Effects, tab previews, link previews, and @ mention previews each start off. Enable the previews you want, then Save and reload Anytype. Disabling link or mention previews keeps the source clickable. Marker links beginning with `blob://` are excluded from these preview switches.

## Background operation and shortcuts

Closing the window keeps Settings running in the background. Launching it again brings back the existing window. Under Advanced Settings > App Settings, enable Launch at startup and press Save to start it when you sign in. Ctrl+Q in Settings exits completely.

| Default shortcut | Action |
| --- | --- |
| Numpad 1 in Anytype | Opens Settings while its background listener runs. |
| Alt+R in Anytype | Rebuilds CSS through the running Settings app. |
| Ctrl+Q in Settings | Quits Settings and its listener. |
| Ctrl+R in Anytype | Reloads Anytype to apply generated CSS. |

Numpad 1 works with Num Lock on or off. The listener leaves other applications' keys unchanged. Open Help > Keybinds, click a binding, and press a replacement combination. Valid changes save automatically; duplicate or unsupported bindings show an error. Ctrl+R belongs to Anytype and is not a BlobUI binding.

## Logs, Help, and maintenance

Open Logs to inspect activity and errors. Copy all logs copies the log text. Help provides Documentation, Keybinds, What's New?, version information, and author and repository links. Release notes appear when Settings reopens after an update.

Advanced Settings contains API configuration and App Settings for the Anytype data folder, startup, updates, and uninstall. See [Updating](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/updating.md) and [Installation and removal](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/installation-and-removal.md) for procedures. Setup and update-preview diagnostic bindings are covered in Development.

## Calendar colors

In Preferences, Calendar Colors controls API-derived object colors in calendar views and defaults to on. Changes apply after Save and reloading Anytype with Ctrl+R. The switch is locked while the API is being checked or cannot be used. Configure the connection in Advanced Settings. An API outage preserves your saved choice and cached colors. Turning Calendar Colors off keeps the cache but removes its colors from the generated CSS.
