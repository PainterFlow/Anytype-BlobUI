# BlobUI v1.4.0 - What's new?

Welcome to v1.4.0 of BlobUI, this update is the biggest update so far, as we are adding the first integration to BlobUI.
Introducing the all new Anyblob, Anyblob allows Anytype to integrate with codex. For more information on Anyblob read the text below this section.
Besides the addition of Anyblob, v1.4.0 also brings a variety of other cool new features for those who are not interested in the AI stuff.
You can now color calendar events by their properties and we also fixed a lot of bugs related to buggy behaviour with kanban calculations.
Besides that you should also notice some performance improvements and a few other quality of live changes.

# Introducing Anyblob!

Anyblob is a optional plugin + MCP server to integrate Anytype with codex.
It uses API v2, allowing you to restrict access, but you can restrict access even further.
Anyblob allows you to configure permissions for each space, the available permissions are:
- **No Access:** default, AI can't see the space or access it.
- **Read Only:** Codex can only read the contents of the space.
- **Read-Write:** Codex can read contents of the space and edit objects, modify properties, send chat messages, edit views, react to messages, etc..
- **Create-Delete:** Codex can create and delete objects, properties and views. (*+ Read-Write permissions).

Additionally codex will also ask for approval depending on how you configure it:
- **Always Ask:** codex will always ask for every action that isn't just reading.
- **Approve for me:** codex will only ask when deleting and creating objects.
- **Never Ask:** codex will never ask.

There is also a toggle which allows you to chose whether you want to give codex access to upload/delete files.
*Developers Note: I made this a separate thing, because I know not everyone self-hosts Anytype or buys the premium plan.*

Additionally, you can also configure these things for each project, since you can also configure which codex project has access to what.
You can configure the things mentioned above for: queries, spaces and collections.
*Developers Note: these permissions act as overrides to the global per-space permissions. Unconfigured or projectless chats/projects use the global permissions.*

Anyblob adds global object mention suggestions to the desktop composer. Object Mentions in Settings filters suggestions by space and object type, with dynamic All defaults. Suggestions can include objects outside a project's read access. Selecting one does not grant access or bypass Always Ask.
You can also use it to manage your Anytype space as there are quite a few skills you can use to do so.

## General

- The sidebar is now resizable.
- You can now use `Ctrl+r` in the BlobUI app to reload BlobUI.

## Color Calendar Events by property

- Added the ability to color calendar events by property.
    - You can color each object type individually.
    - Per-calendar view overrides.
    - Automatically grabs the color of your property option directly from Anytype.
    - The old "Calendar Color" property is still supported, but might get deprecated soon.

## Kanban View Calculations

- Now support multi-selections
- If options of a single or multi-selection property contain duplicate names, the property will be excluded and a hard error will be shown.
*Developers Note: We've decided that it's better to restrict duplicate names then to restrict multi-selection properties.*

## Advanced Settings -> App Settings

- organized and modernized the UI of "App Settings".

## Advanced Settings -> Experiments

- Implement the `Settings Future Proof UI Refresh` experiment.


## Bugfixes

*Developers Note: A variety of undocumented bugs related to kanban view calculations were fixed. If you noticed
any buggy behaviour in v1.3.0, expect it to be fixed now. If not then please report the issue.*

- Issue#75 - Uncollapsed settings tabs have a thinner selected box then normal.
- Issue#72 - The `/` command preview of checkmarks is unaffected by the accent color setting.
- Issue#71 - In-Object searched text results are unaffected by the accent color setting.

## Developer Changelogs

- Added `Build-MCP.ps1` script which builds the MCP server.
- Added `Build-Codex.ps1` script which builds Anyblob.
- Added `Rust-Toolchain.ps1` which is the rust toolchain.
- The `Build-App.ps1` script now also builds the MCP server.
- Added a cache to cache Anytype stuff like requested objects, views, properties, etc..
    - This cache updates when pressing `Ctrl+r` or when restarting the App, it should have no effect on settings etc..
- The BlobUI app UI now uses React JS, I hope this improves performance and eases development.
- Updated `AGENTS.md`, the AI will now no longer edit `Changelogs.md` on its own, unless authorized in the prompt.
- Added `CLAUDE.md` which just forwards to `AGENTS.md`.

---
*Developers Note: All changes without explicitly denoted credits are made by User#PainterFlow*
