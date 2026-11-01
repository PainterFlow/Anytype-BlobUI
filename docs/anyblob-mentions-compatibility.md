# Anyblob composer mentions compatibility

Checked on 2026-11-01 against installed desktop package `OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0`.

Implementation stopped at the compatibility gate. The installed host recognizes `openai/mentions`, but its local mention search dispatcher does not supply the active chat identity to the tool caller. Anyblob cannot safely choose the current project's permitted objects for suggestions. No production capability, picker, resource reader, permission exception, or release version changed.

## Evidence

The installed archive contains `/webview/assets/app-initial-4267e52fe5b0.js`. Its capability discovery accepts `openai/mentions` with `searchTool` and checks the search tool's `readOnlyHint`.

In the same asset, `sIn` dispatches a local mention search to `cIn` with `{query:r.query}` and options `{hostToolCallMetadata:s}`. It does not supply `threadId`. The tool caller `cIn` accepts `threadId` in that options object and passes it through its transport context resolver before calling `callMcpTool`. A fallback transport context does not establish the active composer's project. This probe does not execute that resolver or create its fallback chats.

`codex-plugin/tests/mentions-host-compatibility.cjs` loads the installed dispatcher from the archive and executes it with a captured tool caller connected to a separate stdio fixture process. It uses synthetic project A and project B chat identities concurrently, including empty queries. Both searches omit chat identity and the fixture denies both. Direct identified control calls return their separate project objects. The fixture has no Anytype connection, personal credentials, or personal state access.

The probe passed and reported `extensionRecognized: true`, `activeChatForwarded: false`, `projectControlsPassed: true`, and `decision: BLOCKED`.

Run from the workspace with Node.js, passing the installed archive path.

```powershell
node codex-plugin/tests/mentions-host-compatibility.cjs 'C:/Program Files/WindowsApps/OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0/app/resources/app.asar'
```

This is a version-specific dispatcher and stdio contract probe, not an end-to-end desktop composer test. It does not establish successful capability negotiation over the full installed transport. No fixture was registered in the user's host. The missing project identity already fails the required gate, so resource resolution, subsequent host reads, cancellation, submitted selection verification, and message-scoped consent remain unverified. No claim of support follows from the host recognizing the capability string.

## Required host guarantees before implementation

The host must supply the current chat identity on mention searches, including concurrent and empty searches, and resource resolution and reads. The identity must come from host context separately from model-controlled arguments. The existing Anyblob `_meta.threadId` path can then resolve current project membership and recheck it before returning data.

The host must also provide a trustworthy record or attestation of the submitted resource selections tied to a particular message. Anyblob must be able to verify it separately from tool arguments and reject unselected objects and later messages. Search history, an object URI, and a resource read alone do not prove submission. No such mechanism was verified in this investigation. Do not add an Always Ask exception until both guarantees pass isolated host tests.

Production source filtering, permission precedence, denial behavior, and Always Ask approvals remain unchanged. Resource and consent implementation tests, release bumps, packaging, and the Settings build are deferred because no feature was implemented. The running Anytype instance was not accessed or automated.

The [official extension documentation](https://developers.openai.com/plugins/build/extensions) describes composer mentions as available in ChatGPT desktop. That statement alone does not establish the installed Codex host's local transport or consent guarantees.
