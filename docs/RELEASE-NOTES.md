# Release notes authoring

The Windows release package carries `Changelogs.md`, `Changelogs.yaml`, and any referenced image files. After an update finishes and Settings reopens, the app reads the installed package, renders the notes in a scrollable dialog, and records the version as seen when the dialog closes. An initial install does not open the dialog.

The dialog shows the changelog's version and marks entries that match the running app's current version or the latest available changelog. These markers follow the selected entry when browsing release history.

## Markdown source

Write release notes in the repository-root `Changelogs.md`. The renderer uses Marked with GitHub Flavored Markdown, including headings, paragraphs, emphasis, strikethrough, blockquotes, nested ordered and unordered lists, task lists, fenced code blocks, tables, and inline code.

Raw HTML can be used for semantic formatting such as `<details>`, `<summary>`, `<strong>`, and `<table>`. The renderer sanitizes the result. It keeps a small formatting allowlist, strips scripts, forms, embedded documents, SVG, event handlers, CSS, and unsafe attributes, and only allows `http` and `https` links. Activating a link opens it through the user's default browser.

## Local images

Put image files in `assets/changelogs/`. Declare them in `Changelogs.yaml` using this list shape:

```yaml
images:
  - id: setup
    src: assets/changelogs/setup.png
    alt: The setup screen
  - id: finished
    src: assets/changelogs/finished.jpg
    alt: Completed settings
```

Use the id as an at-mention in Markdown or as a Markdown image target:

```markdown
The new setup is ready: @setup

![Completed settings](@finished)
```

An alias can also be used in a raw HTML image tag:

```html
<img src="@setup" alt="The setup screen">
```

Mentions are resolved only in normal Markdown text and raw HTML image sources. A mention inside inline or fenced code remains literal text. Supported files are PNG, JPEG, GIF, and WebP; SVG is deliberately rejected. At runtime the image bytes are checked against their declared file type and embedded locally, so no network image requests are made. Sources must stay within `assets/changelogs/`; absolute paths, parent-directory segments, and other folders are rejected.

Keep each of `Changelogs.md` and `Changelogs.yaml` at or below 512 KiB. Each image can be at most 8 MiB. The package validator enforces these limits before it creates the release ZIP.

The manifest accepts only one `images:` section with list entries containing `id`, `src`, and optional `alt` fields. Use two spaces before each list item and four spaces before its fields. IDs contain letters and numbers and may include hyphens or underscores. When no alt text is given, the id becomes the accessible image description.

## Reusable renderer API

The Settings UI loads the vendored Marked bundle before `settings-app/ui/markdown.js`. That script exposes `window.BlobUI.Markdown`:

```js
const manifest = BlobUI.Markdown.parseImageManifest(yamlText);
const images = Object.fromEntries(
  await Promise.all(Object.entries(manifest).map(async ([id, image]) => [
    id,
    { ...image, src: await getLocalImageDataUrl(image.src) },
  ])),
);

const safeHtml = BlobUI.Markdown.render(markdownText, { images });
container.innerHTML = safeHtml;
```

`parseImageManifest(yamlText)` validates the supported manifest subset and returns an object keyed by image id. Each value has `src` and `alt`. The caller supplies `render` an `images` object keyed by id, where each `src` is a base64 `data:image/png`, `image/jpeg`, `image/gif`, or `image/webp` URL. `render(markdownText, { images, document })` returns sanitized HTML; `document` is optional and defaults to the current window document. The renderer is intended for inert, local Markdown content and does not grant arbitrary HTML execution.

For production packaging, build Settings with `scripts/Build-App.ps1 -s -c`, then run `scripts/Build-Installer.ps1 -PackageRelease`. Packaging fails if either changelog source file is missing. The native installer places them in BlobUI's managed runtime, and update installation replaces them with the rest of the managed release files.

## MP4 videos

Add a videos section beside images in Changelogs.yaml. IDs must be unique across both sections. Existing image-only manifests remain supported.

```yaml
images: []
videos:
  - id: board-demo
    src: assets/changelogs/board-demo.mp4
    alt: Compact Kanban with collapsing categories
```

Embed it with @board-demo or ![Compact Kanban](@board-demo). MP4 files must stay under assets/changelogs/, have an ftyp signature, and be no larger than 64 MiB each. Native retrieval and installer validation check paths, signatures, and limits. Release history loads videos from the selected version tag. Failed downloads leave an inline error while the rest of the notes remain readable.

parseMediaManifest(yaml) returns { images, videos }; parseImageManifest remains compatible and returns only images. Use Markdown.mount(container, text, { images, videos }) for interactive readers. Each video entry accepts alt and a validated data:video/mp4;base64 source, or a local MP4 Blob. The shared player owns its temporary object URL and disposes it when replaced or closed. Remote video URLs and raw HTML video tags are rejected.

Autoplay videos in Advanced Settings > App settings defaults to on for existing and new installations. Videos start muted when at least half visible, pause when hidden, and stop at the end. A manual pause is preserved. Turning autoplay off requires manual playback. Move the pointer or focus the player to reveal its shared controls.
