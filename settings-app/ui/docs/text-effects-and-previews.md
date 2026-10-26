---
category: getting-started
index: 8
icon: features
name: Text effects and hover previews
---

# Text effects and hover previews

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

## Video playback

Embedded videos use the same player in Documentation and What's New. Move your pointer over a video or focus its controls to pause, seek, skip ten seconds, change sound, or enter fullscreen. Autoplay starts muted and stops at the end. Change Autoplay videos in Advanced Settings > App settings, then Save. A manual pause remains in effect when you leave and return to that video.
