(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};

  function attach({ container, buttons, panels, onSelect, scrollContainer = container.closest?.(".page-content") }) {
    let selected = Math.max(0, buttons.findIndex(button => button.getAttribute("aria-selected") === "true"));
    let enabled = true;
    let destroyed = false;
    const listeners = [];

    function refresh() {
      if (destroyed || !enabled) return;
      const button = buttons[selected];
      if (!button?.offsetWidth) return;
      const initializing = !container.classList.contains("indicator-ready");
      if (initializing) container.classList.add("indicator-initializing");
      container.style.setProperty("--tab-left", `${button.offsetLeft}px`);
      container.style.setProperty("--tab-width", `${button.offsetWidth}px`);
      container.classList.add("indicator-ready");
      if (initializing) {
        void container.offsetWidth;
        container.classList.remove("indicator-initializing");
      }
      reveal();
    }

    function syncPanels() {
      buttons.forEach((button, index) => {
        const active = index === selected;
        button.setAttribute("aria-selected", String(active));
        button.tabIndex = active ? 0 : -1;
        panels[index].hidden = enabled && !active;
      });
    }

    function reveal() {
      const button = buttons[selected];
      if (!button?.offsetWidth || !container.clientWidth) return;
      const left = button.offsetLeft;
      const right = left + button.offsetWidth;
      if (left < container.scrollLeft) container.scrollLeft = left;
      else if (right > container.scrollLeft + container.clientWidth) container.scrollLeft = right - container.clientWidth;
    }

    function select(index, { focus = false } = {}) {
      if (!Number.isInteger(index) || index < 0 || index >= buttons.length) return;
      const changed = index !== selected;
      const focusWasInPanel = panels[selected].contains(root.document?.activeElement);
      selected = index;
      syncPanels();
      refresh();
      if (enabled) {
        // Move only the tab strip. Scrolling ancestors makes the last tab jump.
        if (focus || focusWasInPanel) buttons[index].focus({ preventScroll: true });
      }
      if (onSelect) onSelect(index);
      if (changed && enabled && scrollContainer) {
        // Every panel change starts at the top, regardless of the previous
        // panel height, focus position, or navigation entry point.
        if (scrollContainer.scrollTo) scrollContainer.scrollTo({ top: 0, behavior: "instant" });
        else scrollContainer.scrollTop = 0;
      }
    }

    buttons.forEach((button, index) => {
      const click = () => select(index);
      const keydown = event => {
        if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
        event.preventDefault();
        const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1
          : (index + (event.key === "ArrowRight" ? 1 : -1) + buttons.length) % buttons.length;
        select(next, { focus: true });
      };
      button.addEventListener("click", click);
      button.addEventListener("keydown", keydown);
      listeners.push({ button, click, keydown });
    });
    root.addEventListener("resize", refresh);
    const observer = typeof root.ResizeObserver === "function" ? new root.ResizeObserver(refresh) : null;
    observer?.observe(container);
    if (root.document?.fonts) root.document.fonts.ready.then(refresh);
    syncPanels();
    refresh();
    return {
      select,
      refresh,
      setEnabled(value) { enabled = value === true; syncPanels(); refresh(); },
      destroy() {
        destroyed = true;
        observer?.disconnect();
        root.removeEventListener("resize", refresh);
        listeners.forEach(({ button, click, keydown }) => {
          button.removeEventListener("click", click);
          button.removeEventListener("keydown", keydown);
        });
      },
    };
  }
  UI.Tabs = { attach };
})(window);
