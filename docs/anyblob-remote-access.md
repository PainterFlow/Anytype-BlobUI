# Remote Anyblob access with Cloudflare Tunnel

BlobUI can expose only its authenticated Anyblob bridge through a Cloudflare Tunnel. The bridge listens on the PC running Anytype at `127.0.0.1:31420`. A separate HTTP adapter listens at `127.0.0.1:31421` for Cloudflare Tunnel traffic. Both listeners bind to loopback. The Anytype Local API, its API key, and other BlobUI services are not routed through the tunnel.

Remote access is off by default. Enabling it creates a secret 64-character key. The bridge rejects requests without that key. Disabling and enabling access rotates the key. Treat both the tunnel URL and key as secrets.

After you configure the remote MCP URL and key below, launch or reconnect the remote Anyblob MCP server. It syncs its Codex host and project catalog at startup and refreshes the catalog when the Codex project list changes. Sync sends project IDs and names, not project folder paths. BlobUI validates each request's remote project membership against that catalog and saved permissions. The MCP request carries membership separately from tool arguments, so a tool argument cannot choose a project.

In BlobUI Settings > Anyblob > Projects, synced projects appear under the cloud PC name. A globe tooltip shows the computer name. Select a project row to save source permissions for that host and project. Those project scopes override global space permissions for chats with verified membership in configured projects. Projectless chats and projects without saved sources use global permissions. If project scopes are saved, missing or invalid project identity denies requests. Creation uses the effective project or global permission.

## Quick Tunnel without an account

Quick Tunnels provide a temporary `trycloudflare.com` URL and do not require a Cloudflare account or domain. Cloudflare describes them as intended for testing and development, not production. The URL changes each time the tunnel starts.

1. Install `cloudflared` on the Windows PC from [Cloudflare's install instructions](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/).
2. Open BlobUI Settings on the PC, enable Anyblob, then enable **Allow Cloudflare Tunnel remote access**.
3. Select **Start tunnel**. BlobUI runs `cloudflared tunnel --url http://127.0.0.1:31421 --no-autoupdate` and shows the generated HTTPS URL. Keep BlobUI running while remote Codex needs access. Select **Stop tunnel** when finished.
4. On the remote server, build the MCP server with this command.

   ```sh
   cargo build --locked --release --manifest-path mcp-server/Cargo.toml
   ```

   Install the Anyblob skills from this checkout with this command.

   ```sh
   ./scripts/Install-Remote-Skills.sh
   ```

   The installer copies the skills to `~/.agents/skills` and their shared workflow reference to `~/.agents/references`. It refuses to replace an unrelated skill or a different reference file with the same name. Start a new Codex session after installation.

5. Configure Codex's local stdio MCP server with the URL and key from BlobUI. This example belongs in `~/.codex/config.toml`.

   ```toml
   [mcp_servers.anyblob]
   command = "/absolute/path/to/Anytype-BlobUI/mcp-server/target/release/blobui-mcp"
   args = []

   [mcp_servers.anyblob.env]
   ANYBLOB_REMOTE_ADDRESS = "https://example.trycloudflare.com"
   ANYBLOB_REMOTE_TOKEN = "paste-the-key-from-BlobUI-here"
   ```

   The MCP client sends authenticated HTTPS requests to the tunnel's `/anyblob` endpoint. Protect the config file and do not put the key in a public repository. On Windows, use the built `blobui-mcp.exe` path.

6. Launch or reconnect the remote Anyblob MCP server. In BlobUI Settings > Anyblob > Projects, select cloud project rows under the remote PC name to save source permissions for that host. The catalog syncs at startup and refreshes when the remote Codex project list changes.

The random Quick Tunnel URL is not the authentication mechanism. The bridge checks the bearer key on every request and rejects missing or invalid keys. Quick Tunnel URLs can change and do not have an uptime guarantee. Use a named tunnel for a stable hostname.

## Named Tunnel with a stable hostname

A named tunnel requires a Cloudflare account, a domain managed in Cloudflare, and a tunnel configured for that hostname. Follow [Cloudflare's tunnel setup guide](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/get-started/) and install `cloudflared` on the Anytype PC. Configure the tunnel ingress to send only the chosen hostname to `http://127.0.0.1:31421`, with a final catch-all rule that does not route other hostnames to BlobUI. Run the tunnel with `cloudflared tunnel run <TUNNEL-NAME>`.

Enable remote access in BlobUI as above, keep the Anyblob key requirement in place, and set `ANYBLOB_REMOTE_ADDRESS` on the Codex server to the stable `https://` hostname. Set `ANYBLOB_REMOTE_TOKEN` to the key from BlobUI. The named tunnel process is managed by the user, not by BlobUI's Quick Tunnel controls. Do not configure an ingress rule for port `31420`, the Anytype Local API, or any other BlobUI service.

## Security notes

- Never expose Anytype's Local API or its API key through Cloudflare Tunnel. The only origin used by this feature is the Anyblob HTTP adapter on loopback port `31421`.
- Keep the key private. Disable remote access in BlobUI to invalidate it; enabling access again creates a new key.
- Remote chats for configured projects use saved project source permissions. Projectless chats and projects without saved sources use global permissions. Review both before enabling remote access. If project scopes are saved, missing or invalid project identity denies requests. Creation uses the effective project or global permission.
- Quick Tunnels are for testing and development. Use a named tunnel when you need a stable URL, and keep the key requirement enabled in either case.
