---
category: getting-started
index: 6
icon: features
name: API connection and calendar colors
---

# API connection and calendar colors

The optional local Anytype API enables property-based calendar colors and Kanban calculations. Fonts, accents, banners, board layouts, and text effects work without a key.

## Configure the connection

1. Enable Anytype's Local API and create a read-only API v2 key.
2. Open Advanced Settings > API key in BlobUI.
3. Enter the key, IP address, and port. The default address is 127.0.0.1 and the default port is 31009.
4. Press Save. Keep Anytype running while BlobUI validates the connection.

![API connection fields with a masked saved key](@api-connection-settings)

Leave the masked saved value unchanged to keep your key. The eye button reveals or hides it. Clear the field and confirm removal to disable API access. A failed replacement keeps the previous saved connection. The key is encrypted for your Windows account and should never be included in screenshots, logs, or issue reports.

If validation fails, check that Anytype is running, Local API is enabled, and the key, address, and port match its configuration. A calendar refresh failure does not by itself mean authentication failed.

## Assign calendar colors

Create a property named Calendar Color in the relevant Anytype space and set it on the objects you want to color. Avoid multiple properties with that exact name in one space.

Supported color values include Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, Green, and Blob Pink. Three- or six-digit hex colors and validated rgb/rgba colors are also accepted.

![Calendar view with events colored by their Calendar Color property](@calendar-event-colors)

## Enable and refresh colors

Calendar Colors in Preferences > Calendar starts on, including after upgrades. BlobUI locks this switch while checking API availability or when the API cannot be used. Configure the connection first, then choose whether the generated stylesheet should include calendar colors.

Press Save in BlobUI to refresh the data and rebuild CSS, then press Ctrl+R in Anytype. While Settings is running, Alt+R in Anytype also requests a rebuild; follow it with Ctrl+R to apply the result.

Turning Calendar Colors off preserves its saved cache but removes those colors from generated CSS. A temporary API outage preserves your saved choice and the last cached colors. Calendar colors are generated snapshots; editing an object's property does not update the stylesheet until the next successful rebuild.

For installation, key removal, and folder recovery, read [Installation and removal](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/installation-and-removal.md).
