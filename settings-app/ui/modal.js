(function () {
  "use strict";

  const BlobUI = window.BlobUI = window.BlobUI || {};

  BlobUI.createModalController = function createModalController({
    backdrop,
    dialog,
    initialFocus,
    fallbackFocus,
    inertTargets = [],
    onClose,
  }) {
    if (!backdrop || !dialog) throw new TypeError("A modal backdrop and dialog are required.");

    let returnFocus = null;
    let closeTimer = null;
    let savedInert = [];

    function resolveElement(value) {
      return typeof value === "function" ? value() : value;
    }

    function isVisible(element) {
      if (!element || typeof element.focus !== "function" || element.disabled) return false;
      if (element.closest("[hidden], [inert]") || element.getAttribute("aria-hidden") === "true") return false;
      return element.getClientRects().length > 0;
    }

    function focus(element) {
      if (!isVisible(element)) return false;
      element.focus();
      return document.activeElement === element;
    }

    function getFocusable() {
      return [...dialog.querySelectorAll(
        'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      )].filter(isVisible);
    }

    function restoreBackground() {
      savedInert.forEach(({ element, inert }) => { element.inert = inert; });
      savedInert = [];
    }

    function finishClose(restoreFocus) {
      backdrop.hidden = true;
      backdrop.classList.remove("is-closing");
      restoreBackground();
      if (restoreFocus && !focus(returnFocus)) focus(resolveElement(fallbackFocus));
      returnFocus = null;
      closeTimer = null;
      if (typeof onClose === "function") onClose();
    }

    function open() {
      if (closeTimer !== null) {
        window.clearTimeout(closeTimer);
        closeTimer = null;
      }
      backdrop.classList.remove("is-closing");
      if (backdrop.hidden) {
        returnFocus = document.activeElement;
        savedInert = inertTargets.map((element) => ({ element, inert: element.inert }));
        inertTargets.forEach((element) => { element.inert = true; });
        backdrop.hidden = false;
      }
      const requested = resolveElement(initialFocus);
      if (!focus(requested)) focus(getFocusable()[0]) || dialog.focus();
    }

    function close({ restoreFocus = true } = {}) {
      if (backdrop.hidden) return;
      backdrop.classList.add("is-closing");
      if (closeTimer !== null) window.clearTimeout(closeTimer);
      const reduced = document.documentElement.dataset.reducedMotion === "true"
        || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      const delay = reduced ? 0 : 140;
      if (delay === 0) finishClose(restoreFocus);
      else closeTimer = window.setTimeout(() => finishClose(restoreFocus), delay);
    }

    function handleKeydown(event) {
      if (backdrop.hidden || backdrop.classList.contains("is-closing") || event.defaultPrevented) return;
      if (event.metaKey || event.altKey || event.ctrlKey) return;
      if (event.key === "Escape") {
        event.preventDefault();
        close();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = getFocusable();
      if (!focusable.length) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const currentIndex = focusable.indexOf(document.activeElement);
      const nextIndex = event.shiftKey
        ? currentIndex <= 0 ? focusable.length - 1 : currentIndex - 1
        : currentIndex < 0 || currentIndex === focusable.length - 1 ? 0 : currentIndex + 1;
      event.preventDefault();
      focusable[nextIndex].focus();
    }

    backdrop.addEventListener("pointerdown", (event) => {
      if (!backdrop.hidden && !backdrop.classList.contains("is-closing") && event.target === backdrop) close();
    });
    document.addEventListener("keydown", handleKeydown);

    return {
      open,
      close,
      isOpen: () => !backdrop.hidden,
    };
  };
})();
