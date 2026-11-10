(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const imagePaths = Object.freeze(Object.assign(Object.create(null), {
    "blobui-mark": "./cdn/blobui-mark.svg",
    "theme-swatches": "./cdn/theme-swatches.png",
    "settings-ui-refresh": "./cdn/settings-ui-refresh.png",
    "api-connection-settings": "./cdn/api-connection-settings.png",
    "anyblob-codex-integration": "./cdn/anyblob-codex-integration.png",
    "anyblob-project-permissions": "./cdn/anyblob-project-permissions.png",
    "kanban-formula-calculator": "./cdn/kanban-formula-calculator.png",
    "calendar-event-colors": "./cdn/calendar-event-colors.png",
    "kanban-balanced-expanded": "./cdn/kanban-balanced-expanded.png",
  }));
  const videoPaths = Object.freeze(Object.assign(Object.create(null), {
    "kanban-compact-collapsing": "./cdn/kanban-compact-collapsing.mp4",
    "text-effects-and-hover-settings": "./cdn/text-effects-and-hover-settings.mp4",
  }));

  function resolveImage(alias) {
    const key = String(alias || "");
    return Object.prototype.hasOwnProperty.call(imagePaths, key) ? imagePaths[key] : "";
  }

  BlobUI.MarkdownMedia = Object.freeze({
    resolveImage,
    imageAliases: imagePaths,
    videoAliases: videoPaths,
  });
})(typeof window !== "undefined" ? window : globalThis);
