---
category: getting-started
parent: features
index: 6
icon: features
name: Calendar Colors
---

# Calendar Colors

Calendar Colors reads property values through the Anytype Local API v2. Configure the connection in the [API Guide](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/api-guide.md), then choose whether BlobUI should include generated calendar colors.

## Assign colors to events

Create a property named `Calendar Color` in the relevant Anytype space and set it on the objects you want to color. Avoid multiple properties with that exact name in one space.

Supported color values include Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, Green, and Blob Pink. Three- or six-digit hex colors and validated `rgb()` or `rgba()` colors are also accepted.

![Calendar view with events colored by their Calendar Color property](@calendar-event-colors)

## Enable and refresh colors

The Calendar Colors switch is in Preferences > Calendar. It starts on, including after upgrades. BlobUI locks the switch while checking API availability or when the API cannot be used.

Press Save in BlobUI to refresh calendar data and rebuild CSS, then press Ctrl+R in Anytype. While Settings is running, Alt+R in Anytype also requests a rebuild; follow it with Ctrl+R to apply the result.

Turning Calendar Colors off preserves its saved cache but removes those colors from generated CSS. A temporary API outage preserves your saved choice and the last cached colors. Calendar colors are generated snapshots: editing an object's property does not update the stylesheet until the next successful rebuild.

## Color by property

Open Preferences > Calendar and choose **Color by Property**. The object-type list supports search, reload, and filtering by space. Select a type to edit its default or add an override for a calendar view in the same space. Types without eligible properties still appear.

Choose a single-select property to use its selected option's Anytype color. Choose a checkbox property to use green for checked and gray for unchecked. Multi-select properties are unavailable because API v2 does not expose the option identities required by this feature.

An object's calendar override takes priority, followed by its type default, then its existing Calendar Color value. Missing values and unavailable properties fall through to the next choice. A missing checkbox value is different from an unchecked checkbox.

Choose **No color** to suppress all BlobUI coloring in that scope. Choose **Use Calendar Color** on the default to restore the existing behavior. Remove an override, or select **Use type default**, to restore inheritance. Adding an override initially inherits the type default.

Closing a dialog preserves pending edits. Use the sidebar Save to publish them or Undo to restore saved rules. Reloading the list preserves edits, including unavailable saved types, properties, and calendars. Unavailable entries can be removed. Rules are stored with BlobUI settings and included in backup and import.

Rules and colors are snapshots. Press Save or Alt+R to rebuild, then Ctrl+R in Anytype to apply. A temporary API failure preserves the last successful snapshot. Calendar day overflow popups use the type default or Calendar Color when their markup does not identify the calendar view. View overrides never apply to an unidentified popup.
