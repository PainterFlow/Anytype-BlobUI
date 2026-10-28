(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  const doc = root.document, html = doc.documentElement;
  const key = "blobui.sidebarGeometry";
  const tooltip = doc.createElement("div");
  tooltip.id = "sidebar-navigation-tooltip"; tooltip.className = "sidebar-navigation-tooltip";
  tooltip.setAttribute("role", "tooltip"); tooltip.hidden = true; doc.body.append(tooltip);
  let tooltipTarget = null;
  function hideTooltip() {
    if (tooltipTarget) {
      const ids = (tooltipTarget.getAttribute("aria-describedby") || "").split(/\s+/).filter(id => id && id !== tooltip.id);
      if (ids.length) tooltipTarget.setAttribute("aria-describedby", ids.join(" ")); else tooltipTarget.removeAttribute("aria-describedby");
    }
    tooltipTarget = null; tooltip.hidden = true;
  }
  function showTooltip(target) {
    if (!target || drag || target === tooltipTarget) return;
    if (mode === "standard") { updateNativeTooltips(); return; }
    const label = target.querySelector(".documentation-item-label") || target.querySelector("span:not(.nav-icon)");
    if (html.dataset.sidebarCollapsed !== "true" && (!label || label.scrollWidth <= label.clientWidth)) return;
    hideTooltip(); tooltipTarget = target;
    tooltip.textContent = target.dataset.sidebarTooltip;
    target.setAttribute("aria-describedby", [target.getAttribute("aria-describedby"), tooltip.id].filter(Boolean).join(" "));
    tooltip.hidden = false;
    const rect = target.getBoundingClientRect(), box = tooltip.getBoundingClientRect();
    tooltip.style.left = `${Math.max(8, Math.min(rect.right + 10, root.innerWidth - box.width - 8))}px`;
    tooltip.style.top = `${Math.max(8, Math.min(rect.top + (rect.height - box.height) / 2, root.innerHeight - box.height - 8))}px`;
  }
  let mode = "standard", width = 256, collapsed = false, drag = null;
  try {
    const saved = JSON.parse(root.localStorage.getItem(key));
    if (Number.isFinite(saved?.width)) width = Math.max(160, Math.min(320, saved.width));
    collapsed = saved?.collapsed === true;
  } catch (_) { /* Storage may be unavailable. Keep session defaults. */ }
  const max = () => Math.max(160, Math.min(320, root.innerWidth * .4));
  const current = () => mode === "pro" && collapsed ? 56 : Math.min(width, max());
  const handles = [];
  function updateNativeTooltips() {
    doc.querySelectorAll("[data-sidebar-tooltip]").forEach(button => {
      const label = button.querySelector(".documentation-item-label") || button.querySelector("span:not(.nav-icon)");
      if (mode === "standard" && label && label.scrollWidth > label.clientWidth) button.setAttribute("title", button.dataset.sidebarTooltip);
      else button.removeAttribute("title");
    });
  }
  function persist() {
    try { root.localStorage.setItem(key, JSON.stringify({ width, collapsed })); } catch (_) { /* Session state still works. */ }
  }
  function render() {
    hideTooltip();
    html.dataset.sidebarCollapsed = String(mode === "pro" && collapsed);
    if (mode === "off") html.style.removeProperty("--sidebar-width");
    else html.style.setProperty("--sidebar-width", `${current()}px`);
    for (const handle of handles) {
      handle.hidden = mode === "off";
      handle.setAttribute("aria-valuemin", mode === "pro" ? "56" : "160");
      handle.setAttribute("aria-valuemax", String(max()));
      handle.setAttribute("aria-valuenow", String(current()));
      handle.setAttribute("aria-valuetext", collapsed && mode === "pro" ? "Icons only" : `${Math.round(current())} pixels`);
    }
    updateNativeTooltips();
  }
  function resize(value) {
    collapsed = mode === "pro" && value < 160;
    if (!collapsed) width = Math.max(160, Math.min(max(), value));
    render();
  }
  function finish(cancel) {
    if (!drag) return;
    const previous = drag; drag = null;
    if (cancel) { width = previous.width; collapsed = previous.collapsed; }
    if (previous.handle.hasPointerCapture(previous.id)) previous.handle.releasePointerCapture(previous.id);
    html.classList.remove("sidebar-resizing");
    render();
    if (!cancel) persist();
  }
  for (const [selector, label] of [[".sidebar", "Settings sidebar width"], [".documentation-layout", "Documentation sidebar width"]]) {
    const host = doc.querySelector(selector);
    if (!host) continue;
    const handle = doc.createElement("div");
    handle.className = "sidebar-resizer"; handle.tabIndex = 0;
    handle.setAttribute("role", "separator"); handle.setAttribute("aria-orientation", "vertical"); handle.setAttribute("aria-label", label);
    host.append(handle); handles.push(handle);
    handle.addEventListener("pointerdown", event => {
      if (mode === "off" || event.button !== 0 || drag) return;
      hideTooltip();
      event.preventDefault();
      drag = { handle, id: event.pointerId, width, collapsed, offset: event.clientX - current() };
      handle.setPointerCapture(event.pointerId); html.classList.add("sidebar-resizing");
    });
    handle.addEventListener("pointermove", event => { if (drag?.handle === handle && drag.id === event.pointerId) resize(event.clientX - drag.offset); });
    handle.addEventListener("pointerup", event => { if (drag?.id === event.pointerId) finish(false); });
    handle.addEventListener("pointercancel", () => finish(true));
    handle.addEventListener("lostpointercapture", () => finish(true));
    handle.addEventListener("keydown", event => {
      if (event.key === "Escape" && drag) { finish(true); return; }
      if (mode === "off" || !["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
      event.preventDefault();
      const value = event.key === "Home" ? (mode === "pro" ? 56 : 160) : event.key === "End" ? max() : event.key === "ArrowRight" && mode === "pro" && collapsed ? 160 : current() + (event.key === "ArrowLeft" ? -8 : 8);
      resize(value); persist();
    });
    updateNativeTooltips();
  }
  function labelButtons() {
    doc.querySelectorAll(".sidebar .nav-item,.sidebar .documentation-item,.documentation-navigation .documentation-item,.sidebar-action").forEach(button => {
      const label = button.querySelector(".documentation-item-label")?.textContent || button.getAttribute("aria-label") || button.textContent.trim();
      if (label) { button.setAttribute("aria-label", label); button.dataset.sidebarTooltip = label; button.removeAttribute("title"); }
    });
  }
  const observer = new MutationObserver(labelButtons);
  for (const selector of ["#sidebar-nav", "#documentation-navigation"]) {
    const target = doc.querySelector(selector);
    if (target) observer.observe(target, { childList: true, subtree: true });
  }
  labelButtons();
  const resizeObserver = new ResizeObserver(updateNativeTooltips);
  doc.querySelectorAll(".sidebar,.documentation-navigation").forEach(sidebar => resizeObserver.observe(sidebar));
  doc.addEventListener("pointerover", event => showTooltip(event.target.closest?.("[data-sidebar-tooltip]")));
  doc.addEventListener("pointermove", event => showTooltip(event.target.closest?.("[data-sidebar-tooltip]")));
  doc.addEventListener("focusin", event => showTooltip(event.target.closest?.("[data-sidebar-tooltip]")));
  doc.addEventListener("pointerout", event => { if (tooltipTarget?.contains(event.target) && !tooltipTarget.contains(event.relatedTarget)) hideTooltip(); });
  doc.addEventListener("focusout", hideTooltip);
  doc.addEventListener("pointerdown", hideTooltip);
  doc.addEventListener("scroll", hideTooltip, true);
  root.addEventListener("resize", render);
  root.addEventListener("blur", () => { hideTooltip(); finish(true); });
  root.addEventListener("keydown", event => {
    if (event.key === "Escape") hideTooltip();
    if (event.key === "Escape" && drag) { event.preventDefault(); event.stopImmediatePropagation(); finish(true); }
  }, true);
  root.addEventListener("unload", () => { observer.disconnect(); resizeObserver.disconnect(); });
  UI.SidebarResize = { setMode(value) { finish(true); mode = "standard"; render(); } };
  render();
})(window);
