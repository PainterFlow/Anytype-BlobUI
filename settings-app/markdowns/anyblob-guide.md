
Anyblob connects Codex to Anytype through BlobUI and the Anytype API v2. BlobUI applies saved access, file, and approval rules per space. The API key also limits what Anyblob can do. Object mentions suggest matches but do not grant permission to read them.

## Local method for Windows

Install the regular BlobUI Settings app and enable Anyblob under Integrations. Configure the Anytype API v2 connection in Advanced Settings. In Anytype, create an API key with access only to the spaces and actions you want Codex to use. In BlobUI, save a permission for each space and choose its access level, approval rule, and file setting.

Add the BlobUI repository as a Codex plugin marketplace:

1. Open the Plugins tab and choose Add, then Add a marketplace.
2. Enter `https://github.com/PainterFlow/Anytype-BlobUI`, select the `main` branch, and enter `codex-plugin` as the directory.
3. Install Anyblob from the Plugins Directory.

You can also follow the video guide below.

@anyblob-installation

Keep BlobUI Settings running while Codex uses Anyblob. Restart Codex after installing or updating the plugin.

## Remote method for Windows and Linux

Use BlobUI Headless with Anytype Headless when Anyblob will run on a remote computer.

### Part 1: Installing Anytype

If you already have an API v2 key, continue to Part 2. Otherwise, install [Anytype Headless](https://github.com/anyproto/anytype-heart/releases), create or sign in to an account, then create an API key with access only to the spaces and actions you need.

### Part 2: Installing BlobUI Headless

1. Download BlobUI Headless from [GitHub Releases](https://github.com/PainterFlow/Anytype-BlobUI/releases) and add `blob-ui` to PATH.
2. If Anytype API runs on another host, set its address with `blob-ui api --host-ip 192.168.1.20 --host-port 31012`. The default endpoint is `127.0.0.1:31012`. Then run `blob-ui api login` and enter the API key. If a key is already saved, use `blob-ui api login -f` to replace it. The terminal does not display the key while you type. BlobUI starts `anytype serve` only for a loopback address.
3. Run `blob-ui spaces ls` to list spaces and their IDs.
4. Run `blob-ui perms ls` to inspect saved global permissions. Set one for a space with `blob-ui perms <space-id> --permission read --approval always --file-uploads false`. Permission values are `read`, `read-write`, and `create-delete`. Approval values are `always`, `auto`, and `never`.
5. Run `blob-ui --toggle-anyblob true` to enable Anyblob.

### Part 3: Installing Anyblob in Codex CLI

1. Install [Codex CLI](https://github.com/openai/codex) with `npm install -g @openai/codex`.
2. Add the marketplace with `codex plugin marketplace add https://github.com/PainterFlow/Anytype-BlobUI`.
3. Install the plugin with `codex plugin install anyblob@anyblob`.

Restart Codex after installation. BlobUI Headless supports the Anyblob content tools. BlobUI Settings-only Kanban calculations and calendar coloring tools are unavailable in headless mode.

## Object mention filters

On Windows, configure mention spaces and object types in BlobUI Settings. On Linux, use `blob-ui object-mention ls`, `blob-ui object-mention add <object-type-id>`, or `blob-ui object-mention remove <object-type-id>`. Use `blob-ui object-mention spaces ls|add|remove <space-id>` to manage mention spaces. These filters change suggestions only. Anyblob still checks the saved permission for a space before reading an object.
