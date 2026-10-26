# Settings documentation data

The Settings UI reads its bundled Markdown articles through `BlobUI.DocsData` in `settings-app/ui/docs-data.js`. `load()` fetches the explicit article list relative to the UI directory and returns `{ categories, articles }`. Categories are ordered in JavaScript; articles are sorted by their numeric `index`, then alphabetically by name for ties or missing indices.

Each article is a Markdown file in `settings-app/ui/docs/`. Its filename is the stable lowercase slug used as the article ID. The frontmatter fields are `category`, `index`, optional `parent`, `icon`, and `name`. The article body starts after the closing `---` line.

```markdown
---
category: getting-started
index: 3
icon: theme-swatches
name: Features
---

![Theme color swatches](@theme-swatches)

The article body accepts the Markdown supported by BlobUI's existing renderer.
```

Categories are `getting-started` (User guide) and `development` (Development). The production catalog has eleven root articles with no nesting. The first indexed root in category order opens initially. Indices start at 1 in each category. User guide uses Introduction (1), Installation and removal (2), Features (3), Updating (4), Kanban layouts and formulas (5), API connection and calendar colors (6), Experimental Settings layout (7), and Text effects and hover previews (8). Development uses Setup and architecture (1), Theme & docs dev (2), and Testing and releases (3). Parent references may use an article slug or a unique article name. Articles and their parent must share a category. Only one parent level is supported, so parent articles cannot themselves have parents. `buildCatalog(entries)` validates these rules, unknown categories and parents, duplicate IDs, and cycles, and reports the article and cause in its error.

`resolveImage(alias)` accepts a short alias from the private allowlist and returns a relative local path such as `./cdn/blobui-mark.svg`; unknown aliases return an empty string. Named built-in outline icons are also supported for articles and categories by `docs.js`; unknown names use the document fallback. The same allowlist is exposed as `imageAliases` for UI code that loads image data, and the `theme-swatches` alias demonstrates an article-body image. Add local assets under `settings-app/ui/cdn/` and register their paths in `docs-data.js`. Never put remote URLs or user-controlled paths in this map. The existing Markdown renderer accepts PNG, JPEG, GIF, or WebP data URLs for article-body images, so fetch the resolved local asset, convert it to an approved data URL, and pass it through the renderer's `images` option. SVG assets may be used directly by UI controls when loaded from the allowlisted local path.

## Production articles

User guide contains Introduction, Features, Installation and removal, Updating, Kanban layouts and formulas, API connection and calendar colors, Experimental Settings layout, and Text effects and hover previews. Development contains Setup and architecture, Theme & docs dev, and Testing and releases. Register every file explicitly in `load()` and remove obsolete files when replacing articles.

Use repository-relative links in README.md and absolute GitHub file URLs between articles. Relative article links are stripped by the renderer. Keep primary instructions complete for offline use. Show BlobUI marker URLs as code. HTTP and HTTPS references open in the browser.

These articles are also the repository documentation. Do not maintain a second copy. Update catalog and viewer tests for changes to IDs or categories. Production articles must not contain renderer demonstration text.

`index` must be a positive safe integer when supplied. Existing articles without it sort after indexed articles. Equal indices sort by name, then ID. Number new production articles from 1 within their category and avoid ties. The index is ordering metadata and does not appear in the article title.

## Video aliases and shared playback

Register MP4 files under `settings-app/ui/cdn/` in `DocsData.videoAliases`. Use `@alias` or `![Description](@alias)` in normal Markdown. Aliases inside inline and fenced code remain literal. Each MP4 must have an ftyp signature and be no larger than 64 MiB. Images retain their existing formats and 8 MiB release limit.

Call `BlobUI.Markdown.mount(container, markdown, { images, videos })` to render interactive content. It sanitizes Markdown, disposes previous players, and mounts the shared player. A video entry contains an optional alt string and either a local MP4 Blob in blob or a validated MP4 data URL in src. Do not pass remote or arbitrary file paths. Raw HTML video elements remain blocked. The render method remains available for sanitized inert HTML.

Load video-player.js before markdown.js. Autoplay uses the current app preference, starts muted at 50% visibility, pauses in hidden readers, and stops at the end. Controls reveal on pointer movement or focus. Player teardown disconnects observers and listeners and revokes its object URL. Missing or invalid media shows an inline error.
