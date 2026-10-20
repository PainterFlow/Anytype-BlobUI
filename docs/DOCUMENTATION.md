# Settings documentation data

The Settings UI reads its bundled Markdown articles through `BlobUI.DocsData` in `settings-app/ui/docs-data.js`. `load()` fetches the explicit article list relative to the UI directory and returns `{ categories, articles }`. Categories are ordered in JavaScript; articles are sorted alphabetically by name.

Each article is a Markdown file in `settings-app/ui/docs/`. Its filename is the stable lowercase slug used as the article ID. The frontmatter fields are `category`, optional `parent`, `icon`, and `name`. The article body starts after the closing `---` line.

```markdown
---
category: getting-started
parent: customize-appearance
icon: theme-swatches
name: Accent colors
---

![Theme color swatches](@theme-swatches)

The article body accepts the Markdown supported by BlobUI's existing renderer.
```

Categories are `getting-started` and `reference`. Parent references may use an article slug or a unique article name. Articles and their parent must share a category. Only one parent level is supported, so parent articles cannot themselves have parents. `buildCatalog(entries)` validates these rules, unknown categories and parents, duplicate IDs, and cycles, and reports the article and cause in its error.

`resolveImage(alias)` accepts a short alias from the private allowlist and returns a relative local path such as `./cdn/blobui-mark.svg`; unknown aliases return an empty string. The same allowlist is exposed as `imageAliases` for UI code that loads image data, and the `theme-swatches` alias demonstrates an article-body image. Add local assets under `settings-app/ui/cdn/` and register their paths in `docs-data.js`. Never put remote URLs or user-controlled paths in this map. The existing Markdown renderer accepts PNG, JPEG, GIF, or WebP data URLs for article-body images, so fetch the resolved local asset, convert it to an approved data URL, and pass it through the renderer's `images` option. SVG assets may be used directly by UI controls when loaded from the allowlisted local path.
