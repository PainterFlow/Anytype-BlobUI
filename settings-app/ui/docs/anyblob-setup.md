---
category: getting-started
parent: anyblob-codex-integration
index: 5
icon: setup
name: Setup
---

# Anyblob setup

Anyblob runs through the Windows desktop versions of BlobUI and Codex. Local Codex execution is required. WSL, Linux, and remote execution are not supported. Keep BlobUI Settings running while Codex uses Anyblob.

## Connect and install

1. In Anytype, enable the Local API and create an API v2 key. Choose the API rights you need. A read-only key can read content, but it cannot authorize edits or creation.
2. In BlobUI, open Advanced Settings > API key. Enter the key and connection address, then save. Anytype must be running while BlobUI checks the connection. See the [API Guide](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/api-guide.md) for key and connection details.
3. Open Integrations > Anyblob > Settings. Select Install beside Install Marketplace. This registers the bundled Anyblob plugin in your personal Codex marketplace. It does not install or enable the plugin inside Codex.
4. In Codex, open the Plugins Directory, select your personal marketplace, then install and enable Anyblob. Reopen Codex if the marketplace does not appear.
5. In BlobUI, open Integrations > Anyblob > Permissions. Choose an access level for each space Codex should use, then select Save. Spaces start at No Access. Read [Configuration](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/anyblob-configuration.md) before granting write or file access.
6. Return to Integrations > Anyblob > Settings and turn on Enable Anyblob. This control saves immediately.
7. Keep BlobUI running in its open window or its existing background mode. Ask Codex to list accessible spaces, then search for an object in a space you allowed.

Installing the marketplace package, enabling Anyblob in Codex, and enabling the BlobUI bridge are separate steps. BlobUI does not start or restart either app.

## If setup does not work

If Codex cannot find the marketplace, reopen Codex and check the personal marketplace in its Plugins Directory. If Anyblob reports that the bridge is unavailable, start BlobUI and check that Enable Anyblob is on. If a space is missing or a read is denied, check the saved API connection and the space's permissions. A read-only Anytype key cannot perform writes even when BlobUI is set to Read-Write or Create-Delete.
