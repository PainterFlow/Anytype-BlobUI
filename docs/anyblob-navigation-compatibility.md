# Anyblob Sources integration

Anyblob 0.2.8 adds readable object and space names to individual entries in the existing Codex Sources section. Entries use `result._meta["openai/resourceActivities"]`. Ordinary MCP resource links remain in tool content for MCP clients and mentions. Task-row integration is outside this change.

## Why 0.2.6 did not change the panel

The previous version emitted `content[].resource_link` entries only. The installed Sources collector does not collect arbitrary MCP resource links. It falls back to one generic Anyblob tool entry. Its resource activity URL schema rejects `anyblob://` and accepts only HTTP(S). The earlier passing compatibility probe reproduced this failure; it did not verify a working Sources feature.

## Object entries

Each eligible result now includes version 1 resource activity metadata with a stable ID containing the canonical space/object URI, a title and a real Anytype HTTPS universal link. The format `https://object.any.coop/{object_id}?spaceId={space_id}` comes from [Anytype's own link builder](https://github.com/anyproto/anytype-ts/blob/develop/src/ts/lib/util/object.ts). Its [URL parser](https://github.com/anyproto/anytype-ts/blob/develop/src/ts/lib/util/common.ts) recognizes the object and space IDs. No invite ID, invite key, sharing operation or access grant is included.

Successful object and document reads, confirmed object/chat/collection/query/template creation and confirmed edits return source references. Chat message activity links its parent chat; child receipt titles never rename the parent. Document titles also resolve from `properties.name`. Names display as `Object title · Space name`.

The bridge resolves display metadata through its saved API v2 connection after permission validation. Content operations reuse existing approval context when available; otherwise they load that context before dispatch with the existing cancellation and permission checks. This supplies names even for projected reads, outlines and Markdown responses that omit properties. Object reads resolve the space name and retain their already-read object title. Only the matching space and target object's display names leave these internal requests, never other spaces, properties, blocks or invite data. Wrong object IDs cannot supply titles. Mutations never trigger a follow-up object read. Confirmed creation and rename requests supply names when receipts omit them, while returned names take precedence. Missing metadata uses `Untitled object` and `Unknown space`, never raw IDs. Metadata lookup failures do not change a confirmed mutation receipt or cause a retry.

Source attribution uses the host's `read` activity for the returned object identity or mutation receipt. This does not claim an additional Anytype API read or change permissions. The original mutation outcome and receipt remain authoritative in structured content. Using `created` as the activity would put creation-only references outside Sources in the installed host.

Search hits, member listings, schemas, property/type/widget operations, deletions, dry runs, denied or failed requests and unknown mutation outcomes emit no activity metadata. The existing text, structured results, receipts, warnings and etags remain intact. Canonical IDs are validated before building either link.

The host deduplicates by the space/object identity, uses later titles from chronological fixture turns and suppresses the generic Anyblob tool row when individual entries represent that provider. Unrelated tool sources remain. Codex controls ordering and truncation; its collector scans recent activity first, so first-use ordering is not supplied by this integration.

## Icon and URL display limits

The installed external-source icon component takes its provider logo from connected-app discovery or falls back to the resource website favicon. Its local MCP fallback does not read `serverInfo.icons` or the plugin's manifest logo. The actual component probe confirms a favicon fallback for `object.any.coop`. Anyblob already advertises the existing custom app icon for mentions and its plugin listing, but there is no verified supported per-resource icon override for these local MCP HTTPS Sources. No fake connected-app identity, unrelated hosted icon or installed-host patch is used.

The host also renders the actual HTTPS resource URL in its source-details subtitle. The object and space IDs must stay in that link for correct opening. Its subtitle format has no verified local plugin override. Source titles and descriptions no longer use IDs; the host-controlled URL line can still contain them.

## Opening and permissions

Clicking a Sources row dispatches the HTTPS universal link through Codex's external-link handler. Anytype handles opening and its own current account access. This replaces the original proposal to open Sources through the BlobUI MCP resource reader. No fake HTTPS proxy or installed-app patch is used.

MCP reads and mutations retain all BlobUI approvals and project restrictions. The canonical `anyblob://objects/{space_id}/{object_id}` resource still resolves through the existing permission-checked reader when used through MCP. Neither kind of link grants access. Historical titles and identities already stored in a chat can remain visible after permission changes; Codex does not offer a verified callback to revoke historical source metadata.

## Host verification

The probe reads Codex `OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0` without launching or modifying it. Synthetic results come from the production Rust `tool_result` serializer through `mcp-server/examples/source-fixtures.rs`.

It executes the installed host's actual resourceActivities parser and bundled Zod schema, external resource collector, tool source collector, navigation components and source-row click handler in isolated VMs. The parser accepts the generated HTTPS metadata and rejects an `anyblob://` control. Tests cover repeated reads, duplicate titles in separate spaces, later authorized titles, confirmed creation, search/dry/unknown exclusions, failed and denied calls, separate chats, JSON reloads, generic-row replacement, connected-app identity, unrelated sources and external-link dispatch.

App discovery, React hooks and element creation, title queries and the final external-open operation are isolated. No network, personal chat state, host filesystem, timers or actual app transport is supplied to those VMs. The shared bundle's import declarations and exports are removed, import metadata is replaced with an empty object, and only its schema initializer is called. No installed files are patched. JSON reloads verify collector reconstruction, not durable desktop persistence. A live Sources panel and OS link handoff have not been automated or verified.

## Reproduce and install

After packaging the current executable, run `scripts/tests/Test-Anyblob.ps1 -SkipBuild` to generate `.test-run/navigation/source-fixtures.json` through the production serializer and run the isolated native tests. Without `-SkipBuild`, the test script packages the current executable itself. The native transport fixture intentionally launches that package, so a stale package tests stale behavior. Then run the host checks.

```powershell
node codex-plugin/tests/navigation-host-compatibility.cjs 'C:/Program Files/WindowsApps/OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0/app/resources/app.asar'
node codex-plugin/tests/mentions-host-compatibility.cjs 'C:/Program Files/WindowsApps/OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0/app/resources/app.asar'
```

The navigation probe must report `decision: PASS` with individual sources and actual metadata parsing. A changed host schema or function signature fails the probe and requires inspection.

Validation on 2026-11-02 passed 18 MCP tests and 47 isolated native tests, including packaged `Note · Work`, creation names from sparse receipts, projected-read `Project plan · Work`, metadata privacy, exclusions and permission revocation. One live test remained ignored. Host Sources, source-icon limitation, mention and custom mention-icon checks passed. `scripts/Build-App.ps1 -s -c` completed once with `BLOBUI_BUILD_ONLY=1`. No live Anytype instance was automated or restarted.

Build with `BLOBUI_BUILD_ONLY=1` and `scripts/Build-App.ps1 -s -c`. The resulting workspace package is `codex-plugin/dist/anyblob.zip`. Existing installed copies do not update merely because workspace files were built. Update Anyblob through the rebuilt Settings app and refresh its Codex connection before testing new reads. Existing responses without activity metadata are not backfilled. This work does not modify installed plugin files or restart a running app.
