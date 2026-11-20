# Anyblob global object mentions

Anyblob 0.2.7 adds individual Sources entries through HTTPS resource activity metadata while retaining canonical MCP resource links. See [Sources integration](anyblob-navigation-compatibility.md) for the production-fixture host checks, installation steps and opening behavior. Task-row integration is outside this change.

Implemented on 2026-11-01 for Anyblob 0.1.3 and Settings 2.0.1. The earlier mention suggestions and message-scoped approval exceptions are superseded.

## Settings

Object Mentions appears below General with Filtered spaces and Filtered object types. The shared category box and multi-select controls stay independently editable during loading, empty selections and discovery failures. Dynamic All includes newly accessible spaces and types. All and individual selections are mutually exclusive; an empty explicit selection matches nothing.

Types use space IDs and full type IDs, and labels include space names. Explicit type selections outside filtered spaces stay saved and appear inactive. Unavailable selections stay visible. Discovery failures preserve saved filters and available options. The existing Save/Undo draft flow applies edits. Unsaved changes never affect searches. Space and type discovery do not load object documents.

Long picker labels truncate on one line with an ellipsis. Filter Incomplete Tasks sits below the type picker and defaults off. Saved `filterIncompleteTasks` excludes checked Done values before the result limit, including custom task types. Search requests only the Done property in addition to its normal metadata when this option is on. Unset or unchecked values remain eligible, and malformed completion values return an error. Completion data is never included in suggestions. Changing this saved option invalidates in-flight searches through the existing filter comparison.

## MCP integration

Initialization advertises `openai/mentions` with `searchTool: search_mentions` through extensions and the legacy experimental capability location. The read-only tool is app-visible, accepts `{query: string}` including empty queries and returns structured `{items: ResourceLink[]}` with at most 20 results. This follows the [official extension documentation](https://developers.openai.com/plugins/build/extensions) and linked [composer mention specification](https://github.com/openai/mcp-extensions/blob/main/docs/spec.md#composer-at-mentions).

Metadata search requires the enabled integration and existing bridge authentication. It is independent of saved read grants. Suggestions contain only object identity, title and space metadata, and may include objects that the saved space permission does not allow Anyblob to read. They contain no previews, properties, bodies or attachment metadata. Selecting a suggestion grants no access.

Search uses API v2, enumerating selected spaces with empty-query search and matching titles by Unicode lowercase substring comparison. This avoids full-text capped snapshots and overlapping pages. Saved full type IDs resolve to current API keys before filtering, preserving space-scoped identity despite duplicate names or keys. Unknown types never broaden searches. Filtering precedes deduplication, deterministic sorting and limiting. Sort order is title, space name, space ID and object ID in Rust string order.

Anyblob 0.1.4 matches every whitespace-separated query term, ignoring leading, trailing and repeated whitespace. A canceled client that disconnects before pipe accept no longer kills the bridge listener. Concurrent requests wait for a busy pipe instance with cancellation and timeout checks. Isolated Windows regression tests reproduce the original error 232 and verify recovery. Desktop logs showed unavailable-bridge errors being hidden by the native mention UI as empty results. Settings must remain running for searches; closing its window hides it, while Quit stops the bridge.

Failures return errors without broader fallbacks. Enumeration fails at 100,000 rows per space or space list. Large spaces may be slow. There are no search or body caches. Requests recheck cancellation, authentication, saved filters and integration revision between HTTP requests and before returning results. Settings and connection changes invalidate in-flight requests, including changes during HTTP.

## Resource reads

Object URIs use `anyblob://objects/{space_id}/{object_id}`. Some Codex hosts wrap that URI as one percent-encoded path segment in `mcp-resource://anyblob/{encoded_uri}`. Anyblob decodes exactly that wrapper once, then applies the same strict canonical URI checks. Extra segments, encoded IDs, query strings, fragments, other schemes and model-controlled identity or consent arguments remain rejected.

`resources/read` maps directly to the existing `ReadObject` path. Resource reads use the same saved per-space permission and approval checks as `read_object`. A resource URI does not grant access. Resource content is JSON text containing the authorized object document and Markdown. Denials return MCP errors without content. No approval exceptions or writes were added.

## Validation and limits

Focused fixtures cover Unicode, duplicate titles/types, empty queries and selections, dynamic All, type ID/key resolution, filtering before limiting, sorting, pagination, metadata projection, cancellation, concurrent queries, filter changes during HTTP and bridge failures. Permission fixtures cover saved space permissions, Always Ask, revocation and source changes. The packaged executable-to-pipe fixture also covers mention searches, resource resolution, Always Ask and denied resource reads.

Dark browser fixtures cover both layouts, keyboard use, Save/Undo, unavailable and inactive selections, dynamic All, discovery failures and unlocked controls during both space and type loading. The Anyblob fixture's End-key assertion targets the Permissions tab.

The installed-host probe reads package `OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0` without registering plugins, creating chats or accessing personal state. The installed provider selector accepts discovery responses from the packaged MCP executable and rejects a non-read-only control. The installed mention dispatcher and resource-read helper run against an isolated resolver and stdio fixture. Global concurrent metadata searches and resource reads at the webview RPC boundary pass. Synthetic space permissions produce the expected read outcomes.

Validation passed 32 focused native tests, the packaged executable/pipe fixture, four MCP unit/stdio tests, three dark UI fixtures, the existing UI state checks and the installed-host probe. Plugin packaging and `scripts/Build-App.ps1 -s -c` passed with `BLOBUI_BUILD_ONLY=1`.

The desktop backend's final host-to-MCP resource identity forwarding and complete live composer workflow remain unverified. The existing reader denies access when the required identity is missing. No live app smoke test was run, and no running app was restarted or automated.

```powershell
node codex-plugin/tests/mentions-host-compatibility.cjs 'C:/Program Files/WindowsApps/OpenAI.Codex_26.1027.3384.0_x64__2p2nqsd0c76g0/app/resources/app.asar'
```

Build with `scripts/Build-App.ps1 -s -c` and `BLOBUI_BUILD_ONLY=1`. Use the isolated environment in `scripts/tests/Test-Anyblob.ps1` for native tests, never the production pipe or personal credentials.

## Anyblob 0.2.5 mention icons

Initialization now advertises the existing 256px app PNG through `serverInfo.icons` as a data URI. The plugin manifest logo alone does not supply the installed host's MCP resource mention fallback. Its provider selector reads server icons and forwards the selected URL when inserting resource mentions. No object search fields or permissions changed.

The icon is embedded in `mcp-server/src/icon-data-uri.txt`, extracted byte for byte from `settings-app/icons/icon.ico`. Run `node codex-plugin/tests/mention-icons.cjs <installed-app.asar>` to check logo identity, version consistency, initialization wiring and the installed host's actual icon selector without launching apps. This check passed against Codex 26.1027.3384.0. The stdio regression also asserts initialization icon metadata, but was not run for this change because compilation was explicitly deferred. No build or live smoke test was run.
