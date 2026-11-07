---
category: getting-started
index: 11
icon: features
name: API Guide
---

# API Guide

The optional Anytype Local API v2 connection supplies data for Calendar Colors and Kanban formulas. A read-only key is enough for those features. Anyblob also uses the saved connection for Codex requests. BlobUI permissions cannot grant more access than the Anytype key allows. Appearance settings, board layouts, banners, and text effects work without a key. Anytype must be running with its Local API enabled when BlobUI validates a connection.

## Configure the connection

1. In Anytype, enable Local API and create an API v2 key. Choose rights that match the features and Anyblob operations you plan to allow. A read-only key can read content but cannot authorize edits or creation.
2. Open Advanced Settings > API key in BlobUI.
3. Enter the key, IP address, and port. The defaults are `127.0.0.1` and `31009`.
4. Press Save. Keep Anytype running while BlobUI validates the connection.

![API connection fields with a masked saved key](@api-connection-settings)

Leave the masked saved value unchanged to keep your key. The eye button reveals or hides it. To remove API access, clear the field and confirm removal. If a replacement key fails validation, BlobUI keeps the previous saved connection.

The key is encrypted for your Windows account. Do not include it in screenshots, logs, or issue reports.

If validation fails, check that Anytype is running, Local API is enabled, and the key, address, and port match its configuration. A calendar refresh failure does not by itself mean authentication failed. For Codex setup and permissions, read [Anyblob Setup](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/anyblob-setup.md). For property setup, color values, and refresh behavior, read [Calendar Colors](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/calendar-colors.md).
