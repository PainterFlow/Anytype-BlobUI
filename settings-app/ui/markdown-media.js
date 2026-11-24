(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const cdnImageSource = /^\.\/cdn\/[A-Za-z0-9_-]+\.(?:png|jpe?g|gif|webp)$/i;
  const cdnVideoSource = /^\.\/cdn\/[A-Za-z0-9_-]+\.mp4$/i;
  const imageMimeTypes = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);
  const imageCache = new Map();
  const videoCache = new Map();

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

  function loadVideo(source) {
    const path = String(source || "");
    if (!cdnVideoSource.test(path)) return Promise.resolve(null);
    if (!videoCache.has(path)) {
      videoCache.set(path, (async () => {
        const response = await root.fetch(path);
        const length = Number(response.headers.get("content-length"));
        if (!response.ok || (Number.isFinite(length) && length > 64 * 1024 * 1024)) return null;
        const blob = await response.blob();
        return blob.size > 0 && blob.size <= 64 * 1024 * 1024
          ? { blob: new root.Blob([blob], { type: "video/mp4" }) }
          : null;
      })().catch(() => null));
    }
    return videoCache.get(path);
  }

  BlobUI.MarkdownMedia = Object.freeze({
    loadImage,
    loadVideo,
  });
})(typeof window !== "undefined" ? window : globalThis);
