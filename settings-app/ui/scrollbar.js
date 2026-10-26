(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  UI.Scrollbar = { attach(container, contentSelector) {
    let timer = null, destroyed = false;
    const observed = new Set();
    const resize = typeof root.ResizeObserver === "function" ? new root.ResizeObserver(update) : null;
    function update() {
      if (destroyed) return;
      const overflow = container.scrollHeight > container.clientHeight;
      container.classList.toggle("has-overflow", overflow);
      if (!overflow) {
        container.classList.remove("is-scrolling");
        root.clearTimeout(timer); timer = null;
      }
    }
    function observeContent() {
      if (resize) {
        const current = new Set([container, ...container.querySelectorAll(contentSelector || ":scope > *")]);
        for (const node of observed) if (!current.has(node)) { resize.unobserve(node); observed.delete(node); }
        for (const node of current) if (!observed.has(node)) { resize.observe(node); observed.add(node); }
      }
      update();
    }
    function scroll() {
      update();
      if (!container.classList.contains("has-overflow")) return;
      container.classList.add("is-scrolling");
      root.clearTimeout(timer);
      timer = root.setTimeout(() => { container.classList.remove("is-scrolling"); timer = null; }, 900);
    }
    const mutation = typeof root.MutationObserver === "function" ? new root.MutationObserver(observeContent) : null;
    mutation?.observe(container, { childList: true, subtree: true, attributes: true, attributeFilter: ["data-disclosure-state", "hidden"] });
    container.addEventListener("scroll", scroll, { passive: true });
    root.addEventListener("resize", update);
    observeContent();
    return { update, destroy() {
      if (destroyed) return;
      destroyed = true; root.clearTimeout(timer);
      mutation?.disconnect(); resize?.disconnect(); observed.clear();
      container.removeEventListener("scroll", scroll); root.removeEventListener("resize", update);
      container.classList.remove("is-scrolling");
    } };
  } };
})(window);
