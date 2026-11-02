# Native Anyblob approvals

Anyblob uses standard MCP `elicitation/create` form requests on the direct local stdio connection. BlobUI owns the saved permission checks and prepares the exact operation before asking. It sends a readable, bounded description over the authenticated pipe. Codex receives a required boolean confirmation with a false default. Only an accepted form with `confirm` set to the boolean `true` permits that operation once.

The pipe exchange uses bridge version 2. Each approval decision must match its connection, operation ID and unique approval ID. Update BlobUI and the Anyblob plugin together, then reconnect the plugin. Older bridges fail with update guidance. Clients without form elicitation may run operations that need no approval. Approval-required operations fail with guidance to update Codex or use a compatible client.

The two-minute approval deadline includes time in the serialized approval queue. The existing three-minute overall call deadline still applies. Decline, cancel, missing or malformed confirmation, disconnect, timeout and changed saved permissions deny execution. Late and duplicate MCP answers cannot resolve a later request. Permissions and project membership are checked again before execution. Irreversible actions still require approval under Never Ask. Codex tool-approval preferences are independent and remain unchanged.

Descriptions use the prepared operation, including the immutable local upload snapshot. They show requested values, IDs when names are unavailable, upload size and hash, and explicit irreversible consequences. Content and collection previews are bounded and identify truncation and original sizes or counts. Names and content remain untrusted display text. No extra metadata reads or model calls obtain names or summaries. No reusable approval tokens, tool consent arguments or saved approval choices are introduced.

Settings retains permission pickers but no approval modal, approval event listeners or Tauri approval command. Closing a Settings window has no approval role.

## Validation

Run `scripts/tests/Test-Anyblob.ps1` for isolated stdio, pipe, mock API and authorization tests. The dedicated `mcp-server/tests/elicitation.rs` fixture covers capability negotiation, explicit true and false answers, decline, cancel, malformed content, stale and duplicate responses, two simultaneous calls answered in reverse order, cancellation and unsupported clients. The mock API fixture also confirms a successful mutation receipt after a real question-and-answer exchange. Existing tests retain permission revocation, project membership, overlapping grants, irreversible actions and prepared upload coverage.

Dark Playwright fixtures retain permission-picker Save/Undo coverage and assert that Settings registers no approval listeners. Validation never controls the user's running Anytype instance.

These fixtures validate the standard stdio protocol with an isolated client, not the installed Codex desktop renderer. Actual installed-client form rendering remains unverified. Hosted plugins, embedded UI and multi-round-trip hosted transports are outside this change.

The protocol follows the [Codex elicitation flow](https://learn.chatgpt.com/docs/app-server) and [plugin server guidance](https://developers.openai.com/plugins/build/mcp-server). The app-server's `mcpServer/elicitation/request` method belongs to the host bridge and is not called by this MCP server.
