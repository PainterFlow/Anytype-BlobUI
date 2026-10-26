---
category: getting-started
parent: features
index: 6
icon: features
name: Calendar Colors
---

# Calendar Colors

Calendar Colors reads the `Calendar Color` property through the Anytype Local API. Configure the connection in the [API Guide](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/api-guide.md), then choose whether BlobUI should include generated calendar colors.

## Assign colors to events

Create a property named `Calendar Color` in the relevant Anytype space and set it on the objects you want to color. Avoid multiple properties with that exact name in one space.

Supported color values include Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, Green, and Blob Pink. Three- or six-digit hex colors and validated `rgb()` or `rgba()` colors are also accepted.

![Calendar view with events colored by their Calendar Color property](@calendar-event-colors)

## Enable and refresh colors

The Calendar Colors switch is in Preferences > Calendar. It starts on, including after upgrades. BlobUI locks the switch while checking API availability or when the API cannot be used.

Press Save in BlobUI to refresh calendar data and rebuild CSS, then press Ctrl+R in Anytype. While Settings is running, Alt+R in Anytype also requests a rebuild; follow it with Ctrl+R to apply the result.

Turning Calendar Colors off preserves its saved cache but removes those colors from generated CSS. A temporary API outage preserves your saved choice and the last cached colors. Calendar colors are generated snapshots: editing an object's property does not update the stylesheet until the next successful rebuild.
