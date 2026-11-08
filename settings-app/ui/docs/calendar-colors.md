---
category: getting-started
parent: features
index: 9
icon: features
name: Calendar Colors
---

# Calendar Colors

BlobUI can color calendar events from Anytype properties. Set a default rule for each object type and add overrides for individual calendar views. The older `Calendar Color` property remains available as a fallback. Both features use the optional Anytype Local API v2 connection. See the [API Guide](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/api-guide.md) to connect it.

## Color by property

Open Calendar Events > Color by property. Search, reload, or filter the object-type list by space. Select a type to edit its default rule or add an override for a calendar view in the same space. Types without eligible properties still appear.

Choose a single-select property to use its selected option's Anytype color. A checkbox property uses green when checked and gray when unchecked. A multi-select property uses the first selected tag's color in the order returned by Anytype. Reordering selections can change the color. Empty selections or an unresolved first tag fall through to the next rule.

For an event with a view override, BlobUI uses that rule first, then the type default, then the legacy `Calendar Color` value. Missing values and unavailable properties fall through to the next choice. A missing checkbox value is different from an unchecked value.

Choose **No color** to suppress BlobUI coloring in that scope. Choose **Use Calendar Color** as the default to restore the legacy behavior. Remove an override or select **Use type default** to restore inheritance. A new override starts by inheriting the type default.

![Calendar view with events colored by a Calendar Color property](@calendar-event-colors)

## Use the legacy Calendar Color property

Create a property named `Calendar Color` in the relevant Anytype space and set it on the objects you want to color. Avoid creating more than one property with that name in a space.

Supported color names include Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, Green, and Blob Pink. Three- or six-digit hex colors and validated `rgb()` or `rgba()` colors are also accepted.

## Save and refresh

The Calendar Colors switch is in Calendar Events > Appearance. It starts on, including after upgrades. BlobUI locks the switch while it checks API availability or when the API cannot be used.

Use the sidebar Save to publish pending rules and rebuild CSS, then press Ctrl+R in Anytype. While Settings is running, Alt+R in Anytype also requests a rebuild; follow it with Ctrl+R to apply the result. Calendar colors are snapshots. Editing an object's property does not change the stylesheet until the next successful rebuild.

Turning Calendar Colors off preserves its saved cache but removes those colors from generated CSS. A temporary API outage preserves the last successful snapshot and saved choice. Calendar day overflow popups use the type default or Calendar Color when their markup does not identify the calendar view. View overrides do not apply to an unidentified popup.

Closing a dialog keeps pending edits. Save publishes them, and Undo restores saved rules. Reloading a list keeps edits, including unavailable saved types, properties, and calendars. Unavailable entries can be removed. Rules are stored with BlobUI settings and included in backup and import.

Duplicate option names make the entire property unavailable for calendar coloring, including otherwise unique choices. Saved rules stay in place, and objects fall through to inheritance or their legacy Calendar Color. Multi-select colors always use the first selected tag in API order; an unresolved first tag falls through without choosing a later tag.

Ctrl+R in BlobUI reloads saved rules and settings before refreshing Anytype lists. A successful Codex publication updates the open BlobUI window, including an open color-rule dialog. Unrelated pending edits stay intact. If saved rules conflict with a local edit, the saved rule replaces it and Logs records a warning. Undo restores the latest saved rules. These reloads do not rebuild CSS or refresh color snapshots. Calendar discovery reads API v2 document blocks that already contain the calendar views.
