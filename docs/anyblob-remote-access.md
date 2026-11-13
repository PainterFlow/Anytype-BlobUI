# Remote Anyblob access with Cloudflare Tunnel

BlobUI can expose only its authenticated Anyblob bridge through a Cloudflare Tunnel. The bridge listens on the PC running Anytype at `127.0.0.1:31420`. A separate HTTP adapter listens at `127.0.0.1:31421` for Cloudflare Tunnel traffic. Both listeners bind to loopback. The Anytype Local API, its API key, and other BlobUI services are not routed through the tunnel.

Remote access is off by default. Enabling it creates a secret 64-character key. The bridge rejects requests without that key, and disabling then enabling remote access rotates it. Treat both the tunnel URL and key as secrets. Remote Codex requests use saved global space permissions. BlobUI cannot verify remote project membership, so project-specific permissions are unavailable.

## Quick Tunnel without an account

Quick Tunnels provide a temporary `trycloudflare.com` URL and do not require a Cloudflare account or domain. Cloudflare describes them as intended for testing and development, not production. The URL changes each time the tunnel starts.

1. Install `cloudflared` on the Windows PC from [Cloudflare's install instructions](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/).
2. Open BlobUI Settings on the PC, enable Anyblob, then enable **Allow Cloudflare Tunnel remote access**.
3. Select **Start tunnel**. BlobUI runs `cloudflared tunnel --url http://127.0.0.1:31421 --no-autoupdate` and shows the generated HTTPS URL. Keep BlobUI running while remote Codex needs access. Select **Stop tunnel** when finished.
4. On the remote server, build the MCP server:

   ```sh
   cargo build --locked --release --manifest-path mcp-server/Cargo.toml
   ```

5. Configure Codex's local stdio MCP server with the URL and key from BlobUI. For example, in `~/.codex/config.toml`:

   ```toml
   [mcp_servers.anyblob]
   command = "/absolute/path/to/Anytype-BlobUI/mcp-server/target/release/blobui-mcp"
   args = []

   [mcp_servers.anyblob.env]
   ANYBLOB_REMOTE_ADDRESS = "https://example.trycloudflare.com"
   ANYBLOB_REMOTE_TOKEN = "paste-the-key-from-BlobUI-here"
   ```

   The MCP client sends authenticated HTTPS requests to the tunnel's `/anyblob` endpoint. Protect the config file and do not put the key in a public repository.

The random Quick Tunnel URL is not the authentication mechanism. The bridge checks the bearer key on every request and rejects missing or invalid keys. Quick Tunnel URLs can change and do not have an uptime guarantee. Use a named tunnel for a stable hostname.

## Named Tunnel with a stable hostname

A named tunnel requires a Cloudflare account, a domain managed in Cloudflare, and a tunnel configured for that hostname. Follow [Cloudflare's tunnel setup guide](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/get-started/) and install `cloudflared` on the Anytype PC. Configure the tunnel ingress to send only the chosen hostname to `http://127.0.0.1:31421`, with a final catch-all rule that does not route other hostnames to BlobUI. Run the tunnel with `cloudflared tunnel run <TUNNEL-NAME>`.

Enable remote access in BlobUI as above, keep the Anyblob key requirement in place, and set `ANYBLOB_REMOTE_ADDRESS` on the Codex server to the stable `https://` hostname. Set `ANYBLOB_REMOTE_TOKEN` to the key from BlobUI. The named tunnel process is managed by the user, not by BlobUI's Quick Tunnel controls. Do not configure an ingress rule for port `31420`, the Anytype Local API, or any other BlobUI service.

## Security notes

- Never expose Anytype's Local API or its API key through Cloudflare Tunnel. The only origin used by this feature is the Anyblob HTTP adapter on loopback port `31421`.
- Keep the key private. Disable remote access in BlobUI to invalidate it; enabling access again creates a new key.
- Remote chats use saved global space permissions. Review those permissions before enabling remote access.
- Quick Tunnels are for testing and development. Use a named tunnel when you need a stable URL, and keep the key requirement enabled in either case.
