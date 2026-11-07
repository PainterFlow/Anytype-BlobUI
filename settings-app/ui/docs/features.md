---
category: getting-started
index: 3
icon: features
name: Features
---

# Features

BlobUI groups its customization guides under Features. Choose a guide for the setting you want to change.

## Appearance

[Appearance](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/appearance.md) covers accents, text selections, fonts, and the space banner.

## Kanban

[Kanban](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/kanban-layouts-and-formulas.md) explains board layouts, collapsed categories, and view calculations. Board layouts work without API access; calculations require the optional Anytype Local API.

## Calendar Colors

[Calendar Colors](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/calendar-colors.md) covers property-based event colors, calendar overrides, the legacy Calendar Color property, and refresh behavior. It requires the optional Local API connection.

## Anyblob (Codex Integration)

[Anyblob](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/anyblob-codex-integration.md) connects Codex to Anytype through BlobUI. Use its separate setup and configuration guides to install the plugin and set space or project permissions.

## QoL Features

[QoL Features](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/text-effects-and-previews.md) covers text effects, hover previews, reduced motion, shortcuts, Logs, and Help.

## Save and apply changes

Settings keeps most edits as drafts until you select Save. Save writes the changes and rebuilds `custom.css`. In Anytype, press Ctrl+R to apply the stylesheet. Select Undo changes to discard unsaved edits after confirmation. API connection setup has a separate validation flow in the [API Guide](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/api-guide.md).

Ctrl+R inside BlobUI reloads saved settings and refreshes Anytype lists and pickers. Codex publications refresh saved settings automatically, and returning to BlobUI checks for missed changes. Unrelated local edits are preserved. Saved values replace conflicting local edits, with a warning in Logs. No confirmation dialog appears, and Undo uses the newest saved settings. These reloads do not publish CSS.

## Settings layout

The [Settings UI guide](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/settings-ui-refresh.md) explains category tabs and sidebar navigation.
