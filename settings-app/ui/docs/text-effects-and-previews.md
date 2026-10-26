---
category: getting-started
parent: features
index: 7
icon: features
name: QoL Features
---

# QoL Features

BlobUI provides visual text effects through marker links and lets you choose which Anytype hover previews appear. These controls work without API access.

## Apply a text effect

Select text in an Anytype object and set its link to one of these marker URLs. Keep Apply Custom CSS enabled in Anytype.

| Marker URL | Effect |
| --- | --- |
| `blob://text.sfx/spoiler` | Conceals text until its editable block is hovered. |
| `blob://text.sfx/glitchy` | Applies a glitch animation. |
| `blob://text.sfx/jumping` | Moves text up and down. |
| `blob://text.sfx/rgb` | Animates a rainbow gradient. |
| `blob://text.sfx/neon` | Animates a neon gradient. |

The recording demonstrates text effects and the hover-preview settings.

@text-effects-and-hover-settings

Hovering inside one editable block reveals its spoilers together. Keyboard focus can reveal an individual spoiler. Spoilers are visual effects, not access controls; the object still contains the concealed text. Effects use inline blocks, so long text may wrap differently.

v1.3.0 fixes spelling underlines showing through spoilers and interrupting neon or RGB effects. Reload Anytype after upgrading if you still see the old appearance.

## Choose hover previews

Under Accessibility > Hover Effects, choose tab previews, link previews, and @ mention previews independently. All three start off. A disabled preview leaves its source clickable. BlobUI marker links are excluded from link-preview switches.

Enable only the previews you want, press Save in BlobUI, and reload Anytype with Ctrl+R. Undo changes discards unsaved choices after confirmation.

## Reduce animation

Reduced Motion in Accessibility suppresses BlobUI's animated text effects. Settings transitions and category animations also respect reduced motion. The option starts off; the app also respects the system motion preference for its interface transitions.

## Background operation and shortcuts

Closing the window keeps Settings running in the background. Launching it again brings back the existing window. Under Advanced Settings > App Settings, enable Launch at startup and press Save to start it when you sign in. Ctrl+Q in Settings exits completely.

| Default shortcut | Action |
| --- | --- |
| Numpad 1 in Anytype | Opens Settings while its background listener runs. |
| Alt+R in Anytype | Rebuilds CSS through the running Settings app. |
| Ctrl+Q in Settings | Quits Settings and its listener. |
| Ctrl+R in Anytype | Reloads Anytype to apply generated CSS. |

Numpad 1 works with Num Lock on or off. The listener leaves other applications' keys unchanged. Open Help > Keybinds, click a binding, and press a replacement combination. Valid changes save automatically; duplicate or unsupported bindings show an error. Ctrl+R belongs to Anytype and is not a BlobUI binding.

## Logs and Help

Open Logs to inspect activity and errors. Copy all logs copies the log text. Help provides Documentation, Keybinds, What's New?, version information, and author and repository links. Release notes appear when Settings reopens after an update.

Advanced Settings contains API configuration and App Settings for the Anytype data folder, startup, updates, and uninstall. See [Updating](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/updating.md) and [Installation and removal](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/installation-and-removal.md) for procedures. Setup and update-preview diagnostic bindings are covered in Development.
