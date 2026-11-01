# Anyblob composer mentions implementation

Updated on 2026-11-01. The global metadata backend is implemented. Native mentions are not advertised yet.

## Revised contract and implemented foundation

Suggestions search saved global space and object type filters independently of project identity and read permissions. Suggestions may identify objects the current project cannot read. Selecting a suggestion grants no access. Resource reads must use the existing authorized reader with host-provided identity and preserve Always Ask. Missing identity retains existing denial behavior. The earlier project-specific suggestion and message-scoped approval requirements are superseded.

Saved `codexIntegration.objectMentions` contains independent `spaces` and `types` selections, each with `all` and `items`. Missing settings migrate to dynamic All. Explicit empty selections match nothing. Invalid present filters fail closed. Space items preserve IDs and names. Type items preserve space IDs, type IDs, names and space names. Type selections survive changes to spaces. Settings normalization preserves these filters during permission edits and Save/Undo.

The typed bridge operation `search_mentions` accepts only `query`, including an empty string, with a 4096-byte limit. It requires the enabled integration and existing bridge authentication, independently of project lookup and read approvals. The operation is not listed as an MCP tool yet.

Search discovers current spaces and enumerates selected spaces using the v2 empty-query search endpoint. It matches titles by Unicode lowercase substring comparison. This avoids the existing full-text endpoint's capped snapshot and overlapping pages. Type matching requires full `type.id`. Filters apply before sorting and limiting. Results deduplicate by space/object ID and sort by title, space name, space ID and object ID using Rust string order. Pagination failures return errors without broader fallback. Enumeration fails at 100,000 rows per space or space list. Large spaces may be slow. There is no cache.

The result contains at most 20 metadata-only resource links. Search never requests object documents. Stable suggestion URIs use `anyblob://objects/{space_id}/{object_id}` and validated IDs. Resource resolution is not implemented. A suggestion URI never authorizes a read.

The bridge checks cancellation, authentication, integration revision and saved filters before HTTP requests and before returning results. Connection and permission saves already invalidate the revision. Concurrent searches share no result cache or mutable results.

## Remaining implementation

- Add the Object Mentions category below General with the shared multi-select pickers, independent unlocked state, inactive selections, discovery failures and Save/Undo. Add type-only discovery without loading project sources or object documents.
- Advertise `openai/mentions` and the app-visible read-only search tool. Verify the exact extension schema against official documentation and isolated host fixtures.
- Add strict resource URI parsing and `resources/read`, routing through `ReadObject` with host metadata. Reject forged URIs and verify host identity forwarding for native resource reads.
- Extend installed-host fixtures for global concurrent metadata searches, native capability discovery, structured response shape, cancellation and subsequent reads. The old probe tests the superseded requirement and is not the current release gate.
- Cover picker keyboard behavior, dark layouts, loading, failures, unavailable selections and Save/Undo. Extend resource-read tests for differing projects, Always Ask, revocation, source changes and project moves.
- Bump release versions when native mentions are ready and validate the complete package.

## Backend checkpoint validation

The backend checkpoint passed 26 focused native tests, the packaged executable-to-pipe-to-mock-Anytype fixture, two MCP unit tests and the MCP stdio test. The dark-mode permission Save/Undo fixture passed in both layouts, including filter normalization and unavailable-selection preservation. The existing UI state checks passed.

`scripts/Build-App.ps1 -s -c` passed with `BLOBUI_BUILD_ONLY=1`, including plugin package validation. No running app was restarted or automated.

The approval-dialog fixture fails its background-inert assertion at `settings-app/tests/anyblob-ui.test.cjs:44`. It reproduces against unchanged `HEAD` source in an isolated workspace copy. This checkpoint does not fix that existing failure. Native composer discovery and resource reads remain unverified.

Release versions remain unchanged because native mentions are unfinished. Bump them with the completed feature, rather than treating this backend checkpoint as a native mention release.

## Historical probe details

Checked on 2026-11-01 against installed desktop package `OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0`.

The earlier implementation stopped because mention searches lacked active chat identity. Global metadata searches now accept shared helper identity. Resource-read identity still needs verification before native mentions ship. No approval exception is planned.

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

## Historical requirements, now superseded

The old plan required active chat identity on mention searches. The revised plan only requires that identity on content reads, separately from model-controlled arguments. The existing `_meta.threadId` path resolves project membership and rechecks it before returning content.

The old plan also required submitted-selection attestation for a message-scoped approval exception. The revised plan has no such exception and requires no attestation.

Production content source filtering, permission precedence, denial behavior and Always Ask approvals remain unchanged. The running Anytype instance was not accessed or automated.

The [official extension documentation](https://developers.openai.com/plugins/build/extensions) describes composer mentions as available in ChatGPT desktop. That statement alone does not establish the installed Codex host's local transport or consent guarantees.
