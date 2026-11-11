(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const videoPaths = Object.freeze(Object.assign(Object.create(null), {
    "kanban-compact-collapsing": "./cdn/kanban-compact-collapsing.mp4",
    "text-effects-and-hover-settings": "./cdn/text-effects-and-hover-settings.mp4",
  }));
  const cdnImageSource = /^\.\/cdn\/[A-Za-z0-9_-]+\.(?:png|jpe?g|gif|webp)$/i;
  const imageMimeTypes = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);
  const imageCache = new Map();

  function loadImage(source) {
    const path = String(source || "");
    if (!cdnImageSource.test(path)) return Promise.resolve(null);
    if (!imageCache.has(path)) {
      imageCache.set(path, (async () => {
        const response = await root.fetch(path);
        const mimeType = (response.headers.get("content-type") || "").split(";", 1)[0].toLowerCase();
        const length = Number(response.headers.get("content-length"));
        if (!response.ok || !imageMimeTypes.has(mimeType) || (Number.isFinite(length) && length > 8 * 1024 * 1024)) return null;
        const bytes = new Uint8Array(await response.arrayBuffer());
        if (bytes.length === 0 || bytes.length > 8 * 1024 * 1024) return null;
        let binary = "";
        for (let offset = 0; offset < bytes.length; offset += 0x8000) {
          binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
        }
        return { src: `data:${mimeType};base64,${root.btoa(binary)}` };
      })().catch(() => null));
    }
    return imageCache.get(path);
  }

  BlobUI.MarkdownMedia = Object.freeze({
    loadImage,
    videoAliases: videoPaths,
  });
})(typeof window !== "undefined" ? window : globalThis);
