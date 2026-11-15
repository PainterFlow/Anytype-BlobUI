# Remote Anyblob access with Tailscale Funnel

BlobUI exposes only its authenticated Anyblob bridge through Tailscale Funnel. The bridge listens on the PC running Anytype at `127.0.0.1:31420`. A separate HTTP adapter listens at `127.0.0.1:31421` for Funnel traffic. Both listeners bind to loopback. The Anytype Local API, its API key, and other BlobUI services are not routed through Funnel.

Remote access is off by default. Enabling it creates a secret 64-character key. The bridge rejects requests without that key. Disabling and enabling access rotates the key. Treat the key as a secret. The `*.ts.net` address is stable while the Tailscale device identity and name stay the same.

The remote MCP server is a connector. It holds the Funnel address and remote key, then forwards each request to local BlobUI. The request carries its Codex chat ID. BlobUI verifies remote chat assignments against the local `.codex-global-state.json`, including the assigned project's host ID, before checking saved permissions. Tool arguments cannot choose a project.

BlobUI reads remote project names and host IDs from its local `.codex-global-state.json`. The remote MCP process does not verify project membership. A missing or unreadable local state file prevents BlobUI from verifying configured remote project permissions.

In BlobUI Settings > Anyblob > Projects, remote projects appear with their Codex host ID. Select a project row to save source permissions for that host and project. Those project scopes override global space permissions for chats assigned to configured projects. Projectless chats and projects without saved sources use global permissions. If project scopes are saved, missing or invalid project identity denies requests. Creation uses the effective project or global permission.

## Set up remote access

1. Install [Tailscale for Windows](https://tailscale.com/download/windows) on the PC running Anytype and sign in to your tailnet. BlobUI requires Tailscale 1.52 or later.
2. Enable Funnel for your tailnet in the Tailscale admin policy. Tailscale may ask you to approve Funnel the first time it starts.
3. Open BlobUI Settings on the PC, enable Anyblob, then enable **Allow Tailscale Funnel remote access**.
4. Select **Start tunnel**. BlobUI runs `tailscale funnel --https=443 --set-path=/anyblob http://127.0.0.1:31421` and shows the device's HTTPS `*.ts.net` address. Tailscale maps only the `/anyblob` path to the loopback adapter. The route stops when you select **Stop tunnel**, disable remote access, or close BlobUI.
5. On the remote server, build the MCP connector with this command.

   ```sh
   cargo build --locked --release --manifest-path mcp-server/Cargo.toml
   ```

   Install the Anyblob skills from this checkout with this command.

   ```sh
   ./scripts/Install-Remote-Skills.sh
   ```

   The installer copies the skills to `~/.agents/skills` and their shared workflow reference to `~/.agents/references`. It refuses to replace an unrelated skill or a different reference file with the same name. Start a new Codex session after installation.

6. Configure Codex's local stdio MCP server with the address and key from BlobUI. This example belongs in `~/.codex/config.toml`.

   ```toml
   [mcp_servers.anyblob]
   command = "/absolute/path/to/Anytype-BlobUI/mcp-server/target/release/blobui-mcp"
   args = []

   [mcp_servers.anyblob.env]
   ANYBLOB_REMOTE_ADDRESS = "https://device.tailnet.ts.net"
   ANYBLOB_REMOTE_TOKEN = "paste-the-key-from-BlobUI-here"
   ```

   The MCP client sends authenticated HTTPS requests to the address's `/anyblob` path. The server requires only the connector binary, address, and key. It does not need an Anytype API key or a copy of BlobUI's project state. Protect the config file and do not put the key in a public repository. On Windows, use the built `blobui-mcp.exe` path.

7. Launch or reconnect the remote Anyblob MCP server. Confirm that the local Codex state contains the remote project and chat assignment. In BlobUI Settings > Anyblob > Projects, select the matching remote project row to save source permissions for its host.

## Network and uptime notes

Tailscale Funnel exposes the mapped path to the public internet. The bridge still requires the Anyblob key on every request. Funnel is currently in beta, and Tailscale applies a non-configurable bandwidth limit without publishing a numeric allowance. BlobUI keeps the route on while it runs, so remote access is unavailable when the Anytype PC or BlobUI is offline.

Funnel works when the remote Codex host is outside your tailnet. If that host can join your tailnet, Tailscale Serve can keep the same service private to tailnet members, but BlobUI's remote access controls configure Funnel.

## Security notes

- Never expose Anytype's Local API or its API key through Tailscale. The only origin used by this feature is the Anyblob HTTP adapter on loopback port `31421`.
- Keep the key private. Disable remote access in BlobUI to invalidate it. Enabling access again creates a new key.
- Remote chats for configured projects use saved project source permissions. Projectless chats and projects without saved sources use global permissions. Review both before enabling remote access. If project scopes are saved, missing or invalid project identity denies requests. Creation uses the effective project or global permission.
- Funnel URLs have a stable Tailscale hostname, but Funnel's public availability and bandwidth limits are controlled by Tailscale. Keep the key requirement enabled.
