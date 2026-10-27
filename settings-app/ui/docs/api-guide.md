---
category: getting-started
index: 8
icon: features
name: API Guide
---

# API Guide

The optional Anytype Local API connection gives BlobUI access to data needed by Calendar Colors and Kanban formula calculations. Appearance settings, board layouts, banners, and text effects work without a key. Anytype must be running with its Local API enabled when BlobUI validates a connection.

## Configure the connection

1. In Anytype, enable Local API and create a read-only API v2 key.
2. Open Advanced Settings > API key in BlobUI.
3. Enter the key, IP address, and port. The defaults are `127.0.0.1` and `31009`.
4. Press Save. Keep Anytype running while BlobUI validates the connection.

![API connection fields with a masked saved key](@api-connection-settings)

Leave the masked saved value unchanged to keep your key. The eye button reveals or hides it. To remove API access, clear the field and confirm removal. If a replacement key fails validation, BlobUI keeps the previous saved connection.

The key is encrypted for your Windows account. Do not include it in screenshots, logs, or issue reports.

If validation fails, check that Anytype is running, Local API is enabled, and the key, address, and port match its configuration. A calendar refresh failure does not by itself mean authentication failed. For property setup, color values, and refresh behavior, read [Calendar Colors](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/calendar-colors.md).
