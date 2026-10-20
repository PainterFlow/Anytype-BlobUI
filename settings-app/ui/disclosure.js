(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};
  const toggleSelector = "[data-disclosure-toggle][aria-controls]";
  const reducedMotionQuery = "(prefers-reduced-motion: reduce)";

  function panelFor(toggle, container) {
    const panelId = toggle.getAttribute("aria-controls");
    const panel = panelId && root.document.getElementById(panelId);
    return panel && container.contains(panel) && panel.hasAttribute("data-disclosure-panel") ? panel : null;
  }

  function setInert(panel, inert) {
    if ("inert" in panel) panel.inert = inert;
    else panel.toggleAttribute("inert", inert);
  }

  function transitionDuration(panel) {
    const style = root.getComputedStyle(panel);
    const durations = style.transitionDuration.split(",").map(parseTime);
    const delays = style.transitionDelay.split(",").map(parseTime);
    return Math.max(0, ...durations.map((duration, index) => duration + delays[index % delays.length]));
  }

  function parseTime(value) {
    const time = Number.parseFloat(value);
    if (!Number.isFinite(time)) return 0;
    return value.trim().endsWith("ms") ? time : time * 1000;
  }

  function prefersReducedMotion() {
    return root.document.documentElement.dataset.reducedMotion === "true"
      || (typeof root.matchMedia === "function" && root.matchMedia(reducedMotionQuery).matches);
  }

  function attach(container, options = {}) {
    if (!container || typeof container.addEventListener !== "function") {
      throw new TypeError("A disclosure container is required.");
    }

    const states = new Map();
    for (const toggle of container.querySelectorAll(toggleSelector)) {
      const panel = panelFor(toggle, container);
      if (!panel) continue;
      const expanded = toggle.getAttribute("aria-expanded") === "true";
      toggle.setAttribute("aria-expanded", String(expanded));
      panel.dataset.disclosureState = expanded ? "open" : "closed";
      panel.hidden = !expanded;
      setInert(panel, !expanded);
      states.set(toggle, { panel, frame: 0, timer: 0, transitionEnd: null });
    }

    function cancelPending(state) {
      if (state.frame) root.cancelAnimationFrame(state.frame);
      if (state.timer) root.clearTimeout(state.timer);
      if (state.transitionEnd) state.panel.removeEventListener("transitionend", state.transitionEnd);
      state.frame = 0;
      state.timer = 0;
      state.transitionEnd = null;
    }

    function setExpanded(toggle, expanded) {
      const state = states.get(toggle);
      if (!state || typeof expanded !== "boolean") return false;
      const current = toggle.getAttribute("aria-expanded") === "true";
      if (current === expanded) return false;

      const { panel } = state;
      const wasOpening = Boolean(state.frame);
      const wasHidden = panel.hidden;
      cancelPending(state);
      toggle.setAttribute("aria-expanded", String(expanded));

      if (expanded) {
        panel.hidden = false;
        setInert(panel, false);
        if (wasHidden) {
          panel.dataset.disclosureState = "closed";
          if (prefersReducedMotion()) panel.dataset.disclosureState = "open";
          else {
            panel.getBoundingClientRect();
            state.frame = root.requestAnimationFrame(() => {
              state.frame = 0;
              if (toggle.getAttribute("aria-expanded") === "true") panel.dataset.disclosureState = "open";
            });
          }
        } else panel.dataset.disclosureState = "open";
      } else {
        if (panel.contains(root.document.activeElement)) toggle.focus();
        panel.dataset.disclosureState = "closed";
        setInert(panel, true);

        const duration = prefersReducedMotion() ? 0 : transitionDuration(panel);
        if (!duration || wasOpening) {
          panel.hidden = true;
        } else {
          const finish = () => {
            cancelPending(state);
            panel.hidden = true;
          };
          state.transitionEnd = (event) => {
            if (event.target === panel && event.elapsedTime * 1000 >= duration - 20) finish();
          };
          panel.addEventListener("transitionend", state.transitionEnd);
          state.timer = root.setTimeout(finish, duration + 50);
        }
      }

      if (typeof options.onToggle === "function") {
        options.onToggle({ toggle, panel, expanded, previousExpanded: current });
      }
      return true;
    }

    function handleClick(event) {
      const target = event.target instanceof root.Element ? event.target.closest(toggleSelector) : null;
      if (!target || !container.contains(target) || !states.has(target)) return;
      setExpanded(target, target.getAttribute("aria-expanded") !== "true");
    }

    container.addEventListener("click", handleClick);
    return {
      setExpanded,
      destroy() {
        container.removeEventListener("click", handleClick);
        for (const state of states.values()) cancelPending(state);
        states.clear();
      },
    };
  }

  BlobUI.Disclosure = { attach };
})(window);
