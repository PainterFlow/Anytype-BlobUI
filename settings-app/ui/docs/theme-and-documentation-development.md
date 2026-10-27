---
category: development
index: 2
icon: code
name: Theme & docs dev
---

# Theme & docs dev

## Change the theme

CSS sources under `Dev` compile in relative-path order. Numeric prefixes group settings, fonts, banners, icons, accent fixes, calendars, text effects, and accessibility. Keep overrides in the relevant module and inspect the cascade when modules share a prefix.

1. Find the existing module and Anytype selector for the behavior.
2. Make the smallest source change. Do not edit generated CSS or release copies.
3. Build Settings with `scripts/Build-App.ps1 -s -c`, generate CSS against an intended test data folder, and inspect Anytype.
4. Check both themes, wrapping, keyboard use, and reduced motion where relevant.

The compiler recognizes annotated placeholders such as `accentColor` and `bannerUrl`. Extend Rust validation and compilation when adding generated values rather than inserting unvalidated CSS from the UI.

## Add an appearance control

Follow the existing Preferences and Accessibility layout. Update the frontend draft, dirty comparison, validation, preview, Save, and Undo together. Add the corresponding persisted Rust settings and CSS generation. Test older local settings and rollback when saving fails.

Use the shared DOM components in `ui/components.js` for settings rows, fields, buttons, and groups. Add properties to the relevant screen definition and a named template slot in `index.html`. `screens.js` mounts these before application bindings run. Keep component updates separate from saving and native commands. Read `settings-app/UI_REFERENCE.md` for the lifecycle and controller contracts.

Appearance changes remain drafts until Save. App preferences and shortcuts have separate persistence paths; follow their existing behavior. Use accessible labels, keyboard focus, status messages, and reduced-motion handling.

## Add text effects

Text effects use links on Anytype text as markers. The containing block can be selected with `:has()` when a block-level hover or layout rule is needed.

```css
.pageMain .editor .blockText .editable a[href="blob://text.sfx/spoiler"] {
    /* Rules for marked text. */
}
```

Keep marker links outside normal hover-preview settings. Check formatted fragments, wrapping, spoiler reveal boundaries, and focus behavior. Add reduced-motion rules for animated effects and document the exact marker URL in Features as code.

## Write articles

The seven production Markdown files live in `settings-app/ui/docs`. Lowercase filename slugs are article IDs. `docs-data.js` registers the explicit file list, categories, and allowlisted images. `docs.js` renders navigation and orders articles by their numeric `index` and opens the first root in category order.

```markdown
---
category: getting-started
index: 1
icon: blobui-mark
name: Introduction
---

# Introduction

Write the article here.
```

Use `getting-started` for User guide and `development` for Development. The production catalog has no nesting. The parser still accepts an optional `parent` field for one level; parents must exist in the same category and cannot have parents themselves. Required fields are `category`, `icon`, and `name`. Add a positive integer `index`, starting at 1 in each category, to control the order. Articles without an index follow indexed articles; ties use name and ID.

Use sentence case, numbered procedures with expected results, and recovery advice beside the relevant task. Keep primary instructions complete for offline readers. README links are repository-relative. Article cross-links use absolute GitHub file URLs because relative links are stripped by the in-app sanitizer.

## Markdown and images

The shared renderer supports headings, lists, tables, quotes, code blocks, and sanitized formatting HTML. Scripts, forms, embedded documents, event handlers, arbitrary styles, and unsafe links are removed. HTTP and HTTPS links open externally. Show `blob://` URLs in code.

Put optional images under `settings-app/ui/cdn` and register an alias in `docs-data.js`. Article images support PNG, JPEG, GIF, and WebP. Reference them with a Markdown target such as `@theme-swatches` and descriptive alt text. The viewer converts allowlisted local images to data URLs; no remote image request is needed. SVG is supported for navigation icons, not article-body images. Built-in outline icon names include `book`, `introduction`, `features`, `installation`, `updating`, `setup`, `code`, and `testing`. Unknown icons use the document fallback.

The catalog API remains `BlobUI.DocsData.load()`, returning `{ categories, articles }`. `buildCatalog()` validates IDs, metadata, and relationships. Update catalog and viewer tests when changing it. The [documentation data reference](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/docs/DOCUMENTATION.md) describes the implementation.

## Onboarding

Onboarding steps use `onboardingSteps` in `app.js`. Conditional steps use `when`, and `advanceOnboarding()` falls back to Settings when finished. Check Escape behavior, authentication failures, independent calendar failures, and reduced-motion transitions after changing the flow.

## Video documentation

Register bundled MP4 assets in DocsData.videoAliases and embed them with an @alias in normal Markdown. Use descriptive lowercase filenames and provide a description in the surrounding instructions. Markdown.mount renders both images and videos through the same sanitized reader used by release notes. The global player provides hover controls, keyboard access, muted autoplay, and cleanup. Read the [documentation data reference](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/docs/DOCUMENTATION.md) and [release notes authoring guide](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/docs/RELEASE-NOTES.md) for manifest shapes and limits.
