(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};

  const pageChildren = Object.freeze({
    preferences: Object.freeze({ colors: 0, font: 1, "space-banner": 2 }),
    accessibility: null,
    "calendar-events": Object.freeze({ appearance: 0, "color-by-property": 1 }),
    "kanban-views": Object.freeze({ appearance: 0, "kanban-calculations": 1 }),
    advanced: Object.freeze({ "api-key": 0, "app-settings": 1, experiments: 2 }),
    codex: Object.freeze({ settings: 0, permissions: 1 }),
  });
  const pageAliases = Object.freeze({
    preferences: "preferences",
    accessibility: "accessibility",
    calendar: "calendar-events",
    "calendar-events": "calendar-events",
    kanban: "kanban-views",
    "kanban-views": "kanban-views",
    advanced: "advanced",
    "advanced-settings": "advanced",
    anyblob: "codex",
    codex: "codex",
  });

  function decodeComponent(value) {
    try {
      return decodeURIComponent(value);
    } catch {
      return value;
    }
  }

  function slug(value) {
    return decodeComponent(String(value || "")).trim().toLowerCase()
      .normalize("NFKD").replace(/[\u0300-\u036f]/g, "")
      .replace(/['’]/g, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
  }

  function settingsPageUrl(value) {
    let url;
    try {
      url = new URL(String(value || "").trim());
    } catch {
      return null;
    }
    return url.protocol === "blob:" && url.hostname === "settings.page" ? url : null;
  }

  function parse(value) {
    const url = settingsPageUrl(value);
    if (!url || url.host !== "settings.page" || url.username || url.password || url.search) return null;
    const segments = url.pathname.split("/").filter(Boolean).map(slug);
    if (segments.length < 1 || segments.length > 2) return null;
    const page = pageAliases[segments[0]];
    if (!page) return null;
    const children = pageChildren[page];
    const childSlug = segments[1] || "";
    let child = "";
    let childIndex = null;
    if (children) {
      if (!childSlug || !Object.prototype.hasOwnProperty.call(children, childSlug)) return null;
      child = childSlug;
      childIndex = children[childSlug];
    } else if (childSlug) return null;
    const section = slug(url.hash.replace(/^#/, ""));
    return Object.freeze({ page, child, childIndex, section });
  }

  function attach(targetDocument, options = {}) {
    const open = typeof options.open === "function" ? options.open : () => {};
    const onError = options.onError || ((error) => root.console.error("Could not open the settings link.", error));

    function handleActivation(event) {
      if ((event.type === "click" && event.button !== 0)
        || (event.type === "auxclick" && event.button !== 1)) return;
      const eventTarget = event.target;
      const element = eventTarget && (eventTarget.nodeType === 1 ? eventTarget : eventTarget.parentElement);
      const anchor = element && typeof element.closest === "function" ? element.closest("a[href]") : null;
      if (!anchor || !targetDocument.documentElement.contains(anchor)) return;
      const url = settingsPageUrl(anchor.getAttribute("href"));
      if (!url) return;

      event.preventDefault();
      event.stopImmediatePropagation();
      const target = parse(url.href);
      if (!target) {
        onError(new Error("This BlobUI settings link does not match an existing settings tab."), url.href);
        return;
      }
      Promise.resolve().then(() => open(target, url.href)).catch((error) => onError(error, url.href));
    }

    targetDocument.addEventListener("click", handleActivation, true);
    targetDocument.addEventListener("auxclick", handleActivation, true);
    let attached = true;
    return {
      destroy() {
        if (!attached) return;
        attached = false;
        targetDocument.removeEventListener("click", handleActivation, true);
        targetDocument.removeEventListener("auxclick", handleActivation, true);
      },
    };
  }

  BlobUI.SettingsLinks = Object.freeze({ attach, parse, slug });
})(typeof window !== "undefined" ? window : globalThis);
