(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};

  function externalHttpUrl(value) {
    const href = String(value || "").trim();
    return /^https?:\/\//i.test(href) ? href : "";
  }

  function attach(targetDocument, options = {}) {
    const open = options.open || ((url) => {
      const api = root.__TAURI__ && root.__TAURI__.core;
      if (!api || typeof api.invoke !== "function") {
        return Promise.reject(new Error("The settings app bridge is unavailable. Launch BlobUI Settings using blobui-settings.exe."));
      }
      return api.invoke("open_external_link", { url });
    });
    const onError = options.onError || ((error) => root.console.error("Could not open an external link.", error));

    function handleActivation(event) {
      if ((event.type === "click" && event.button !== 0)
        || (event.type === "auxclick" && event.button !== 1)) return;
      const eventTarget = event.target;
      const element = eventTarget && (eventTarget.nodeType === 1 ? eventTarget : eventTarget.parentElement);
      const anchor = element && typeof element.closest === "function" ? element.closest("a[href]") : null;
      if (!anchor || !targetDocument.documentElement.contains(anchor)) return;
      const url = externalHttpUrl(anchor.getAttribute("href"));
      if (!url) return;

      event.preventDefault();
      event.stopImmediatePropagation();
      Promise.resolve().then(() => open(url)).catch((error) => onError(error, url));
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

  BlobUI.ExternalLinks = Object.freeze({ attach });
})(typeof window !== "undefined" ? window : globalThis);
