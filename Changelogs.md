# BlobUI v1.5.0 - What's new?

BlobUI v1.5.0 is coming with the long awaited chats update, implementing chat themes and finally fixing text effects not working in chats.
Additionally there are also some other UI changes and some additional backend changes as BlobUI is preparing for a bunch of new crazy features that are coming soon.

## General

- The "Preferences" tab is now split up, kanban view & calendar event color settings have been moved out into their own tabs.

*Developers Note: I wanted to remove the requirement to press that stupid random button at the bottom of the tab to open kanban calculations / calendar coloring by property.*
- You can now toggle whether you want to enable the old legacy "Calendar Color" property to color calendars (*default is on*).

## Chat Themes

- You can now choose how chats should look in a new settings tab called: "Chat Themes".
- Available options are:
    - "Anytype" - Anytypes default look.
    - "Discord" - Chats now look like they are in discord.
    - "IRC" - IRC theme.

*Developers Note: IRC colors are generated on compilation depending on the User who sent the message.*

## Advanced Features

### Streamer Mode

- You can now hide Anytype from screen capture software in advanced settings.
- You can also toggle whether you want BlobUI to automatically toggle streamer mode, depending on whether you're recording/streaming.

## Bugfixes

- Issue#97: Validate Anyblob tool response before asking for permission.
- Issue#16: None of the text effects or hyperlink modifications work in chats at the moment.

## Developer Changelogs

- Settings are now saved at the location of where the app was installed at (*old settings migrate, don't worry*).

### Logs refresh

- The copy button now changes to a checkmark for a short amount of time after clicked.
- Added a clear button.
- The logs button now changes state depending on whether there are errors or warnings in the logs.

---
*Developers Note: All changes without explicitly denoted credits are made by User#PainterFlow*
