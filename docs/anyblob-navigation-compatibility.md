# Anyblob chat navigation compatibility

Anyblob 0.2.6 adds canonical object resource links to eligible successful tool responses. The requested individual Sources entries and linked task integration are blocked on Codex host interfaces. This release does not provide those navigation features.

## Completed result links

Successful `read_object` and `read_document` results include `anyblob://objects/{space_id}/{object_id}` links. Confirmed object, chat, collection, query and template creation uses the returned ID. Confirmed object patches and discussion creation link the object. Adding or editing a chat message, toggling its reaction or marking its chat read links the parent chat. Child receipts never supply the parent's title.

Links use the returned title when available, followed by the returned space name or explicit space ID. Missing titles fall back to the object ID. The description includes Anyblob identity and the read access requirement. Creation IDs must pass the existing canonical resource validation. No extra reads or permission grants occur.

Searches, member listings, schema discovery, property/type/widget operations, deletions, dry runs, errors and unknown outcomes produce no object links. Text, structured results, receipts, warnings, etags and error classification remain intact. Resource reads retain their existing contents envelope and permission checks.

## Installed host evidence

The read-only probe targets Codex package `OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0`. It extracts code from `app.asar` into isolated VM contexts. It never registers plugins, creates chats, reads personal chat state, patches the installed app or contacts Anytype.

- The actual local external resource collector `L` ignores ordinary MCP resource links. Repeated synthetic results and serialized/reloaded fixture turns produce no object entries.
- The actual shared `openai/resourceActivities` URL refinement `Wut` rejects `anyblob://` and accepts an HTTPS control. The schema requires version 1, an array of resources with ID, URL and activities, and optional title and MIME type. Its URL field uses this refinement. The external resource constructor `K` independently rejects non-HTTP(S) URLs.
- The actual local tool collector `K` collapses calls to `mcp-server:anyblob`. It does not interpret Anyblob's confirmed/unknown outcome values. Separate fixture chats do not share collected results.
- The actual navigation functions `fk`, `pk`, `mk` and `gk` render one generic Anyblob row. That row has no resource-open callback. A plugin app ID can open an app, which does not establish object resource opening.

Discovery, metadata envelope parsing, React's element factory, hooks and transport are isolated dependencies. The resourceActivities schema wiring and its actual URL predicate are checked independently; the complete schema parser is not executed. Synthetic JSON reloads establish deterministic collection only, not durable host persistence. No full desktop transport or dark UI session was exercised. Existing resource-reader fixtures check permission-checked opening separately from navigation.

The [official extension documentation](https://developers.openai.com/plugins/build/extensions), checked on 2026-11-02, describes conversation panels, composer mentions and extended context. It does not establish a supported registration API for this Codex navigation task row, durable associations keyed by host chat identity or selected-task goal injection. A separate panel would not satisfy this integration.

## Required Codex host interfaces

Individual Sources require host support for canonical MCP resource URIs in persisted activity metadata, a server-bound `resources/read` open action with trusted parent chat identity, and a collector that deduplicates by space/object while preserving first-use order and authorized title updates. It must replace the generic connector row only for represented activity and preserve unrelated sources. The current HTTP-only metadata cannot meet this requirement. No fabricated HTTPS resource URLs or provider identities are emitted as substitutes.

The linked task requires a supported row registration above Sources, a searchable picker and unavailable state, a trusted host chat identity for picker and MCP actions, atomic versioned persistence of one association per chat, and an explicit goal-context update interface that does not submit a message or begin work. The host must preserve identity across reloads and expose project/permission changes for revalidation. The existing read and approval path must remain authoritative, including when opening from navigation.

Until these contracts are available and verified, task operations `get_chat_task`, `set_chat_task` and `clear_chat_task` are deliberately not advertised. There is no alternate association store, model-supplied identity, task selection UI or goal injection. No task status changes occur. Task selection, replacement, removal, custom task layouts, association concurrency, persistence, permission revocation and picker keyboard/focus/dark-layout tests remain blocked together with the feature.

## Reproduce

Validation on 2026-11-02 passed 17 MCP tests and 35 isolated native Anyblob tests. The live test remained ignored. The executable-to-pipe fixture verified read and creation links, search exclusions, denied reads and permission-checked resource resolution. The combined installed-host probe and icon checks passed. `scripts/Build-App.ps1 -s -c` completed once with `BLOBUI_BUILD_ONLY=1`, including plugin packaging. No running app was automated or restarted. No UI was added while its host requirements remain blocked.

```powershell
node codex-plugin/tests/navigation-host-compatibility.cjs 'C:/Program Files/WindowsApps/OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0/app/resources/app.asar'
```

The existing `mentions-host-compatibility.cjs` also invokes this probe. A successful process exit with `decision: BLOCKED` means the current incompatibility was reproduced. It does not mean navigation support passed. Changed host function or schema signatures fail the probe and require inspection.
