# Anyblob content tools validation

Anyblob 0.2.0 adds 41 explicit API v2 tools alongside the existing four tools. The allowlisted catalog is in `mcp-server/src/content-tools.json`. The MCP executable forwards typed operations through the authenticated bridge; BlobUI owns HTTP, permission checks, upload snapshots and approvals.

Create-Delete is cumulative. Scoped member grants cannot create standalone objects, alter source definitions, or modify space-wide schemas. Explicit query and collection destinations have a separate creation authorization path; ordinary operations retain the existing restrictions. File-valued custom properties, attachment deletion and Markdown images require file access. Nested replacement of view lists requires Create-Delete.

## Automated checks

- `scripts/tests/Test-Anyblob.ps1 -SkipBuild` covers credentials, project identity, source membership, migration opt-in, capability-specific approval merging, cancellation, schemas, fixed HTTP routes, ETags, preflights, unknown outcomes, file access, snapshot cleanup, multipart bytes and the executable-to-pipe-to-bridge-to-mock-API path, including creation.
- The MCP suite checks the complete tool catalog, strict argument parsing, host-supplied identity, canonical resources, nested view capability checks, named-pipe recovery and stdio behavior.
- The four dark browser tests cover permission Save/Undo in both layouts, scoped controls, object mentions, request-bound approvals, irreversible-effect text and narrow layouts. Screenshots stay under `.test-run`.
- The handoff build uses `scripts/Build-App.ps1 -s -c` with `BLOBUI_BUILD_ONLY=1` so it does not close or launch Settings or Anytype. Packaging validates the executable and plugin manifests.

## Live validation on 2026-11-01

The explicit live runner resolved one space named Codex Test and pinned its full ID in `.test-run/anyblob-live-content/ledger.json`. It read the schema index, then Anytype refused the first create preflight with `write_not_granted`. The saved API key grants all spaces read-only access. BlobUI permissions cannot override that grant.

No live object, property, view, chat message or file was created or changed. The ledger records zero upload attempts and no created fixtures. Cleanup succeeded. The temporary encrypted credential copy was removed; the real Anytype profile and running app were not modified.

A write-enabled v2 key is required to finish live mutation validation. Use `scripts/tests/Test-Anyblob-Live.ps1 -TestExecutable <workspace-native-test-executable>` after that grant is available. The script copies only connection settings and the encrypted key to a workspace fixture and removes the credential copy afterward. It does not change the real saved connection or permissions.

The live runner reserves preflight and commit attempts before uploading. The persistent limit is ten attempts across runs, including uncertain outcomes. Local and controlled URL fixtures contain 1,024 bytes and use names `codex-test-{num}`. Never reset the ledger to obtain more attempts.

## Server limits

The connected server's schemas and dry runs remain authoritative. Property updates currently rename definitions; option creation uses supported create bodies or `create_missing_options`, which requires space-wide Create-Delete. Unsupported option removal or message pinning is not simulated. Object, type and property deletion uses archive routes with server ownership restrictions. Message deletion is irreversible and can destroy attachment content, so it always requires explicit approval.

Document patches require ETags. Confirmed mutations return receipts without a content reread, even if the edit changes source membership. Unknown outcomes preserve the retry key and prohibit automatic retries. Upload and attachment remain separate operations. File downloads and account/space administration are outside this release.

## Creation into a saved source

`list_creation_targets` returns only saved Create-Delete queries and collections in the verified project and requested space, even if they have no members. Each query includes its current source types and all views. `create_object` accepts `destination: {kind: "query" | "collection", id}` and requires a caller-supplied `idempotency_key` for this path. Omitting destination preserves space-wide creation.

Query creation requires `template: "none"` and a shortcut payload. Source types and property sources combine with OR. A property source matches explicit presence, including empty strings, false, zero, empty lists and null; omission does not prove presence. API keys, display names and server-supplied internal-key mappings resolve against v2 metadata. The source must match AND at least one current view must match. Unfiltered views and nested AND/OR expressions with positive equal conditions on explicit text, number, checkbox and single-select values can prove a view. Positive in conditions are also supported for single-select values and native Object type. Unsupported conditions remain unknown. A supported matching OR branch is sufficient; every AND branch must match. No values or options are added or changed automatically. Definitions and metadata are reread before the server dry run and commit. Actual current membership is checked afterward; concurrent changes return the creation receipt with a placement failure and retain the object.

Collections validate and approve creation, then add only the confirmed new ID through the v2 add_items operation with the collection ETag. A flushed append-only journal beside the executable records each operation by verified project, space, destination and caller key. A confirmed refusal can resume placement on the identical request after restart. Different payloads with the same key are refused. Pending or uncertain writes and damaged journal tails block automatic retry. Keep the journal with the installation. A journal entry grants no general object access. Subsequent reads and edits must pass normal membership checks.

Validation uses isolated HTTP fixtures only. Do not operate the running Anytype instance.

The issue #78 handoff passed 19 MCP tests, 62 focused native tests and 12 dark Settings/documentation tests. The live Anytype test remained ignored. The required `scripts/Build-App.ps1 -s -c` build succeeded once with `BLOBUI_BUILD_ONLY=1`. No running Anytype instance was operated, and `Changelogs.md` was left unchanged.

### Query creation follow-up in Anyblob 0.2.11

When a payload could satisfy the supported query but needs different or additional values, `create_object` returns `outcome: "needs_input"`. The response includes `requirements.source` alternatives, per-view AND/OR filter requirements, existing select options, the destination and the caller idempotency key. `creation.outcome` and `placement.outcome` are both `not_attempted`. No object is created, no edit exception is granted, and no durable operation record binds the incomplete payload. The AI asks the user when a choice is ambiguous and resubmits `create_object` with the chosen values, the same destination and the same key. Each submission rechecks saved project permissions, file switches, metadata and current query definitions. Once a creation attempt has bound the key, changing its payload is refused.

`unsupported_query` means no supported proof path exists; it must not be presented as a request that changing a value will necessarily fix. Destination discovery reports supported or unsupported query requirements and source property presence explicitly. Collections retain their existing creation, placement and recovery behavior.

Validation for 0.2.11 passed 72 focused native tests, 20 MCP tests and 12 dark Settings/documentation tests. The required `scripts/Build-App.ps1 -s -c` build succeeded once with `BLOBUI_BUILD_ONLY=1`. Package and version checks passed. Collection execution and recovery code matched the previous version exactly. The live Anytype test remained ignored and no running Anytype instance was operated. `Changelogs.md` was left unchanged.

### Real v2 identifiers and membership filters in 0.2.12

Creation resolves body.type against both live API keys and full object IDs. The canonical API key is sent to v2 while the original caller payload remains the idempotency identity. Metadata and canonical type identity are checked again before dispatch, preventing a renamed or reassigned API key from selecting a different type. Type normalization also rechecks file and schema restrictions.

Current v2 excludes property_internal_keys and type_internal_key from object reads. Source-property mappings therefore also read live type_settings.property_definitions, matching property to the live API key and internal_key to the query source. Names and equal option lists are never used to guess property identity. Uninstalled definitions are ignored; unavailable or conflicting identities remain unprovable. Only server metadata supplies these mappings.

Native Object type filters resolve IDs or API keys and compare against body.type. A value under body.properties.type cannot override the requested type. Single-select in filters resolve existing option names through metadata. Requirements retain AND/OR structure and include allowed_values and value_location. Calendar and general object-reference conditions remain unsupported when their semantics cannot be proven. Other supported views can still establish membership. Collection behavior is unchanged.

The regression fixtures preserve the captured response shape without private space/object identifiers: a raw BSON source-property key, type listings with keys and names but no IDs, full type documents with property definitions, Project in a list of option names, native type in a list of full IDs, and several views including an unsupported Calendar view. They exercise discovery, a creation dry run and one commit, normal read/edit access, mismatches, spoofed body.properties.type, unresolved property identities and rejected conditions. No live Anytype instance is used.

Validation for 0.2.12 passed 79 focused native tests, 20 MCP tests and 12 dark Settings/documentation tests. The final query/collection rerun passed all 30 tests, including the captured v2 response shape and ambiguous type-ID guard. The required `scripts/Build-App.ps1 -s -c` build succeeded once with `BLOBUI_BUILD_ONLY=1`; packaged version and plugin checks passed. The live Anytype test remained ignored. No running Anytype instance was operated and `Changelogs.md` remained unchanged.
