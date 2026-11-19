# Anyblob setup guide

Anyblob is a codex integration that allows codex to access your Anytype through a MCP server where each request goes to the BlobUI app, which allows you to configure additional context restrictions & permissions, which then forwards the requests to Anytype's API.
Important to note, if you allow the agent to edit, delete, create and upload files, then we are not responsible for any damage.
Even though the agent could bypass the guard rails(*It's AI*) they are good to limit any delusional AIs from deleting your entire database(*if ykyk*). 

## Local Method (Windows Only)

### 1. Add BlobUI repo as a marketplace in codex.

1. Go to the `Plugins` tab on the left side.
2. Press `Add` -> `Add a marketplace`.
3. Fill in the form:
  - `https://github.com/PainterFlow/Anytype-BlobUI`
  - `main`
  - `codex-plugin`

You can also follow the video guide below.

@anyblob-installation