---
category: getting-started
parent: anyblob-codex-integration
index: 6
icon: settings-ui
name: Configuration
---

# Anyblob configuration

Open Integrations > Anyblob in BlobUI. Use Settings to enable the bridge, Permissions to set global space rules and object suggestion filters, and Projects to configure Codex project sources. Permission edits stay as drafts until you select Save. Undo restores the saved settings.

## Set global space access

The Permissions tab lists spaces available through the saved Anytype API connection. Each space has an access level, an approval choice, and a separate Upload/delete files switch.

| Access level | What it allows |
| --- | --- |
| No Access | Blocks Anyblob access. This is the default. |
| Read Only | Discovers, searches, and reads allowed content. |
| Read-Write | Edits objects and views, sends chat messages, and reacts. |
| Create-Delete | Creates and deletes supported content, including objects, properties, types, and views. |

Anytype's API key still limits every operation. The Upload/delete files switch is an extra requirement for uploads, file deletion, and operations that use attachments. Access level alone does not allow file actions.

## Choose when Codex asks

| Approval choice | Behavior |
| --- | --- |
| Always Ask | Requests approval for each content read or change. |
| Approve for me | Allows routine reads and edits. Asks before creating, deleting, or uploading files. |
| Never Ask | Skips routine prompts. Irreversible actions may still need approval. |

Space discovery and mention suggestions do not grant access to content. An approval request does not override a denied BlobUI permission or a limit on the Anytype API key.

## Limit a Codex project

Open the Projects tab, select a local Codex project, then choose the sources it may use. Sources can be spaces, object types, collections, or queries. Set an access level and approval choice for each selected source, then save.

![Anyblob global and project-specific access controls for spaces, queries, and object types](@anyblob-project-permissions)

A space source covers the space. An object type source covers objects of that type. A collection source covers its current members, and a query source covers its current results. A project source rule overrides the global space rule for that source in that project only. Projects without configured sources use the global space permissions. Project rules do not change other projects or projectless Codex chats.

Each selected project source has its own Upload/delete files switch. File operations still need that switch as well as the source's access level and the rights on the Anytype API key.

## Filter object suggestions

The Object Mentions section on the Settings tab controls suggestions in the Codex composer. Filtered spaces and Filtered object types start on All, which includes newly available spaces and types. Choose explicit entries to narrow suggestions. Filter Incomplete Tasks hides tasks whose Done checkbox is checked.

These filters change suggestions only. They do not grant read permission. A suggestion can refer to an object outside the current project's read access; selecting it does not make the object readable. Codex must still pass the project's or global permission checks and the configured approval choice.
