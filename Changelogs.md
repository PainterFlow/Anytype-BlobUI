# BlobUI v1.4.0 - What's new?

## Anyblob 0.2.2

- Use Codex's Accept button without a duplicate confirmation control.
- Show concise approval descriptions with object names, types and spaces instead of raw metadata.

## Anyblob 0.2.1

- Detect stale versioned Codex plugin copies and show when a refresh is required.
- Recover the bridge listener after pipe errors and wait briefly for a restarting bridge before reporting it unavailable.
- Report incompatible bridge responses as an update requirement.

## Anyblob 0.2.0

- Added explicit v2 tools for objects, properties, types, templates, collections, queries, views, messages, reactions, widgets and file uploads.
- Existing permissions retain their read behavior until Content changes is enabled and saved. Create-Delete then includes reading and editing.
- Added ETag checks, dry runs, idempotency keys and explicit uncertain-outcome reporting. Irreversible deletions always require approval.
- Local uploads use immutable snapshots, a 25 MiB limit and the saved file permission. URL uploads and separate attachment operations are supported.


*Insert random ah text.*

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

Anyblob adds global object mention suggestions to the desktop composer. Object Mentions in Settings filters suggestions by space and object type, with dynamic All defaults and Save/Undo. Suggestions can include objects outside a project's read access. Selecting one does not grant access or bypass Always Ask.
You can also use it to manage your Anytype space as there are quite a few skills you can use to do so.

## General
- the sidebar is now resizable.

## Color Calendar Events by property
- Added the ability to color calendar events by property.
    - You can color each object type individually.
    - Per-calendar view overrides.
    - Automatically grabs the color of your property option directly from Anytype.
    - The old "Calendar Color" property is still supported, but might get deprecated soon.
    - Single Select & Checkmark property types are supported.
*Developers Note: due to Anytype API v2 limitations, multi-selection is not possible.*

## Advanced Settings -> App Settings
- organized and modernized the UI of "App Settings".


## Bugfixes
- Issue#71 - In-Object searched text results are unaffected by the accent color setting.
- Issue#72 - The `/` command preview of checkmarks is unaffected by the accent color setting.

---
*Developers Note: All changes without explicitly denoted credits are made by User#PainterFlow*
