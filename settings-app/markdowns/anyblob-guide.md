# Anyblob setup guide

Anyblob is a codex integration that allows codex to access your Anytype through a MCP server where each request goes to the BlobUI app, which allows you to configure additional context restrictions & permissions, which then forwards the requests to Anytype's API.
Important to note, if you allow the agent to edit, delete, create and upload files, then we are not responsible for any damage.
Even though the agent could bypass these guard rails(It's AI) they are good enough to make sure no delusional AI decides to just delete everything(*if ykyk*).
All-in-all, Anyblob allows the agent to use Anytype with API v2 and you can also @ mention objects in codex for easy access.

## Local Method (Windows Only)

This is the method you probably use, unless you have cloud servers which complicate things a bit.
Anyblob has to be updated manually, it does not update with BlobUI.

#### Add BlobUI repo as a marketplace in codex.

1. Go to the Plugins tab on the left side.
2. Press Add -> Add a marketplace.
3. Fill in the form:
	- `https://github.com/PainterFlow/Anytype-BlobUI`
	- `main`
	- `codex-plugin`

You can also follow the video guide below.

@anyblob-installation

Now just restart codex by pressing Ctrl+Q and then you’re good to go.

---

## Remote Method (Windows & Linux supported, requires BlobUI Headless)

The second method to use Anyblob is to use it on a remote pc together with BlobUI Headless CLI and Anytype Headless.

#### Part 1: Installing Anytype (if you already have a local API key then this is not needed)

1. Download & install Anytype Headless from https://github.com/anyproto/anytype-heart/releases
2. Run `anytype auth create <USERNAME>` and copy the login key to somewhere save (you can also use anytype auth login to log-in to an existing account you own).
3. Run `anytype auth apikey create <name> --all-spaces --read-write` (use `--spaces <space id or name>` to limit spaces instead of `--all-spaces` and you can use `--read-only` for read only access).

#### Part 2: Installing BlobUI headless CLI

1. Download & install BlobUI headless from https://github.com/PainterFlow/Anytype-BlobUI/releases (make sure to install it on PATH)
2. Run `blob-ui api login` and paste the API key you just got(don’t worry if you see nothing appear).

#### Part 3: Installing Anyblob to codex CLI

1. Download & install Codex CLI with `npm install -g @openai/codex` (https://github.com/openai/codex)
2. Run: `codex plugin marketplace add https://github.com/PainterFlow/Anytype-BlobUI`
3. Run: `codex plugin install anyblob@anyblob`

Now restart codex and you should be done.
An important thing to note is that Kanban view calculations and calendar coloring by property mcp tools are unavailable with BlobUI CLI.