# Remote Anyblob access

Remote access lets a Codex process on another PC or server call Anyblob while BlobUI and Anytype stay on the Windows PC. The MCP process connects to a loopback TCP port on its own host. An SSH reverse tunnel carries that connection to BlobUI's loopback-only listener.

## Limits and security

- Keep BlobUI and Anytype running on the Windows PC. Enable Anyblob and then enable **Allow SSH-tunneled remote access** in the Codex integration settings.
- Remote requests use saved global space permissions. BlobUI cannot verify a remote chat's project identity, so project-specific permissions are unavailable. Do not enable remote access if the saved global permissions are broader than intended.
- Treat the remote access key as a secret. Disabling and re-enabling remote access rotates it and invalidates the previous key.
- The listener binds only to `127.0.0.1:31420`. Keep the SSH forward bound to loopback and do not expose this port to the network.

## Set up the tunnel

On the Windows PC, open an SSH reverse tunnel to the server that runs Codex. Replace `user@server` with the SSH account and host:

```powershell
ssh -N -o ExitOnForwardFailure=yes -R 127.0.0.1:31420:127.0.0.1:31420 user@server
```

Keep this SSH session open while Codex uses Anyblob. The SSH server must permit remote TCP forwarding. If the connection fails, check the SSH server's forwarding policy and confirm that neither side exposes port `31420` beyond loopback.

## Configure Codex on the server

Build the MCP executable on the Codex host from the repository:

```sh
cargo build --locked --release --manifest-path mcp-server/Cargo.toml
```

Add the built `blobui-mcp` executable as a local stdio MCP server in `~/.codex/config.toml`. Replace the executable path and key with their real values:

```toml
[mcp_servers.anyblob]
command = "/absolute/path/to/Anytype-BlobUI/mcp-server/target/release/blobui-mcp"
args = []

[mcp_servers.anyblob.env]
ANYBLOB_REMOTE_ADDRESS = "127.0.0.1:31420"
ANYBLOB_REMOTE_TOKEN = "paste-the-key-from-BlobUI-here"
```

Protect this file because it contains the Anyblob key. Start or restart Codex after saving it, and leave the SSH tunnel running. The key is shown when remote access is enabled. If you no longer have it, disable and enable remote access to create a new key, then update Codex's configuration.
