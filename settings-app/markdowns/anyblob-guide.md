# Anyblob setup guide

Codex starts the repository's Anyblob plugin through the local `blob-ui mcp` process. On Windows, that process uses the running BlobUI Settings app's local bridge and saved Anytype connection. On Linux, it connects directly to Anytype Headless and keeps a separate profile.

## 1. Connect Anytype on Windows

In BlobUI Settings, configure the Anytype API v2 connection and enable Anyblob. Keep BlobUI Settings running while Codex uses Anyblob. The local bridge reads the saved Settings connection and permissions.

## 2. Start Anytype Headless on Linux

Install the official Anytype CLI and configure its headless service using the [Anytype CLI documentation](https://developers.anytype.io/docs/anytype-cli). Create an API key for the service and keep the service running on the same computer as BlobUI Headless. The default local API endpoint is `127.0.0.1:31012`.

## 3. Install the BlobUI MCP executable

Download the Windows or Linux executable from the [BlobUI Headless releases](https://github.com/PainterFlow/Anytype-BlobUI/releases). Place it in a directory on `PATH`. Open a terminal on the computer that runs Codex and check that `blob-ui help` prints the command list.

On Linux, use `blob-ui api login` to enter the Anytype API v2 key. BlobUI checks the key against the local API before it saves it. Use `blob-ui api status` to check the saved connection and `blob-ui api logout` to remove the saved key. Linux settings default to `$XDG_CONFIG_HOME/blobui-headless` or `$HOME/.config/blobui-headless`. If that directory is not writable, set `BLOBUI_HEADLESS_HOME` to a writable absolute path.

## 4. Configure Anyblob

On Windows, configure Anyblob permissions and the project folder in BlobUI Settings. On Linux, run `blob-ui --toggle-anyblob true` to enable it in the headless profile. Choose a local folder that contains one subfolder per Anyblob project:

```text
Projects/
  Research/
    anyblob.config.toml
  Writing/
    anyblob.config.toml
```

On Linux, set the folder with `blob-ui project config --folder "$HOME/Projects"`. Run `blob-ui project ls` to list discovered projects. The CLI reads only this local folder. It does not read remote project folders from Codex configuration.

## 5. Set permissions on Linux

Global space permissions apply when a chat has no project-specific source permissions. A project with source permissions can use only the selected sources. Anything not covered by its sources has no access.

Set a global space permission with `blob-ui perms <space-id> --approval auto --permission read --file-uploads false`. Approval values are `always`, `never`, and `auto`. Permission values are `read`, `read-write`, and `create-delete`.

Set a project source permission with `blob-ui project "Research" perms query <query-id> --approval always --permission read --file-uploads false`. Supported source kinds are `space`, `object`, `object_type`, `query`, and `collection`. For an object ID that exists in more than one space, add `--space-id <space-id>`.

Use `blob-ui project "Research" perms ls` and `blob-ui perms ls` to review saved permissions. Use `blob-ui project "Research" info --full` to include the chats that authenticated to that project.

Use `blob-ui object-mention spaces ls` to list every available space and its ID. Each row includes a `selected` field for the current mention filter. Object mention filters only control suggestions. They do not grant permission to read objects. Use `blob-ui object-mention ls` to review filters, `blob-ui object-mention add <object-type-id>` or `remove <object-type-id>` to change object type filters, and `blob-ui object-mention spaces add <space-id>` or `remove <space-id>` to change space filters. Use `blob-ui object-mention --filter-done true|false` to filter completed task mentions.

## 6. Install the repository plugin

Add the Anytype BlobUI repository as a Codex plugin marketplace, then install Anyblob from the Plugins Directory:

```sh
codex plugin marketplace add PainterFlow/Anytype-BlobUI@feat/remote-support
```

The bare repository shorthand checks GitHub's default branch. Use the ref above while the marketplace manifest is only on `feat/remote-support`. After the manifest reaches the default branch, you can omit `@feat/remote-support`.

The repository supplies the Anyblob plugin. Codex starts `blob-ui mcp` from `PATH` when it starts the plugin. Check that `blob-ui help` works in the same environment where Codex starts the MCP process. On Windows, the MCP process uses the BlobUI Settings bridge, so the app must be running with Anyblob enabled. On Linux, it uses the headless profile. If Anyblob tools or object mentions are missing, install and enable Anyblob from the Plugins Directory, then start a new chat. Subagents can use the MCP tools connected to their parent chat. The CLI's management commands are separate from the MCP process, and the plugin does not expose commands that change CLI settings or permissions.

Start a new chat after installing the plugin. For a project chat, keep that project's `anyblob.config.toml` in its local project folder so Anyblob can authenticate the chat to the matching project.

## 6. Back up or update BlobUI Headless

Run `blob-ui export "settings.zip"` to create a CLI settings archive. Use `blob-ui import "settings.zip"` to import a settings archive. Imports from the Settings app never import API credentials. BlobUI Headless marks its own archives in `blob.meta.json` so the Settings app can recognize them.

Run `blob-ui info` to check for a newer version. Run `blob-ui update` to confirm and install it, or `blob-ui update -f` to install without a confirmation prompt.
