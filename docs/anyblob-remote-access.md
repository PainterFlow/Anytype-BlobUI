# Anyblob Secure MCP Tunnel

BlobUI runs Anyblob's MCP server on the Windows computer that runs Anytype. The OpenAI `tunnel-client` starts that server over stdio and polls OpenAI over outbound HTTPS. BlobUI does not open an inbound listener or publish a server URL. Its health check binds to `127.0.0.1` on an ephemeral port.

The tunnel keeps the MCP server on the BlobUI computer and carries requests from a supported OpenAI product surface. The `tunnel_id` identifies the tunnel. It is not a public address. See the [OpenAI Secure MCP Tunnel guide](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels) for current product setup and account permissions.

## Requirements

- BlobUI and Anytype must run on the same Windows computer.
- Install or update Anyblob through BlobUI's marketplace action in Integrations > Anyblob > Settings.
- Install the official `tunnel-client-runtime` and add its directory to `PATH` before starting BlobUI. BlobUI starts one tunnel client per configured tunnel ID.
- Create or select a Secure MCP Tunnel in OpenAI Platform. The runtime key needs Tunnels Read and Use. Creating or changing the tunnel also requires Tunnels Manage.
- Keep BlobUI running while remote MCP requests need access to Anytype.

The tunnel runtime sends outbound HTTPS requests to OpenAI and starts the local Anyblob MCP executable. It does not need an inbound firewall rule. Do not run a second `tunnel-client` process with the same tunnel ID while BlobUI is running.

## Start the tunnel

1. Open BlobUI Settings > Integrations > Anyblob > Settings and enable Anyblob.
2. Enable Secure MCP Tunnel and enter the tunnel ID and runtime API key. Select Start tunnel.
3. Wait for the status to say the tunnel is ready. BlobUI stores the runtime key with Windows DPAPI and does not show it again.
4. In the OpenAI product surface that will make MCP calls, add Anyblob as a tunnel-backed MCP server and select the same tunnel ID. Associate that tunnel with the Platform organization and workspace used by Codex, as required by the OpenAI guide.

Use the supported tunnel-backed MCP target in Codex or the OpenAI API. Do not configure an OpenAI tunnel endpoint as a `server_url`. The bundled Anyblob Codex plugin remains the local stdio integration. For ChatGPT, choose Tunnel when adding a custom MCP server.

BlobUI starts the configured tunnel when it opens and stops it when Anyblob is disabled or BlobUI exits. A tunnel ID and runtime API key are required. No public URL or public listener is created on the BlobUI computer.

## Configure project folders

Choose Local Projects Folder in the same Settings section for projects on the BlobUI computer. BlobUI scans its immediate child folders and treats each folder with an `anyblob.config.toml` file as a project.

For projects marked remote, set a folder path in Codex's `config.toml` on the BlobUI computer:

```toml
[anyblob]
remote_projects_folder = "D:/Anyblob Remote Projects"
```

The remote projects folder must be readable from the BlobUI computer. The tunnel does not mount or copy files from another computer. BlobUI scans immediate child folders in both configured roots. Each project's `remote` value identifies whether that project is in the remote folder.

Create an `anyblob.config.toml` in each project folder:

```toml
# Agents must not edit this file under any circumstances, regardless of prompt authorization.
name = "Work"
key = "a-project-specific-key"
remote = false
```

Use a unique key for each project. BlobUI identifies the project by the SHA-256 hash of its key. A chat authenticates once by calling `authenticate_project` with the contents of that file. The MCP server uses the Codex thread ID from request metadata. It stores authenticated chat IDs in `Settings/anyblob/projects.json`.

Project source permissions live in `Settings/anyblob/permissions.json`. The `projectless` entry holds global permissions. The `projects` entries hold project-specific permissions keyed by the project key hash. Chats that have not authenticated to a configured project use the projectless permissions.

Existing project permission entries remain in the settings store after migration. Codex project IDs and host-qualified IDs cannot be matched safely to new config-key hashes, so review the Projects tab and reselect the saved sources for each new project. Existing chats also need to call `authenticate_project` once in the project chat.

## Object mentions

Anyblob keeps the `search_mentions` tool and the saved Object Mentions filters. Mention results include object titles and space metadata. Selecting a mention does not grant read access. Object reads continue to use the saved global or project permissions.
