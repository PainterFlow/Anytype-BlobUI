---
category: getting-started
index: 4
icon: anyblob
name: Anyblob (Codex Integration)
---

# Anyblob (Codex Integration)

Anyblob connects Codex to Anytype through the running BlobUI Settings app. The optional Codex plugin uses Anytype Local API v2. BlobUI checks its saved permissions before Anyblob reads or changes content. Appearance settings and generated CSS do not depend on Anyblob.

![Codex listing properties it can access in a private space through Anyblob](@anyblob-codex-integration)

## What Codex can do

Codex can discover permitted spaces, search and read objects, and use supported tools to manage content when both the Anytype API key and BlobUI permissions allow the action. The key's own access rights remain in force. BlobUI cannot grant rights that Anytype did not give the key.

Anyblob can also show object suggestions in the Codex composer. A suggestion may come from a space the current project cannot read. Selecting it does not grant access. Anyblob still checks the project's permissions and approval choice when Codex reads the object.

## Guides

- [Setup](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/anyblob-setup.md) explains how to connect the Local API, register the plugin, and enable the integration.
- [Configuration](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/anyblob-configuration.md) explains access levels, approval choices, project rules, and object suggestions.
