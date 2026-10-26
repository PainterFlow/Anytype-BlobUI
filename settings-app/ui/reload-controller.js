(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};

  UI.ReloadController = {
    create({ button = null, onBusyChange = null, onError = null } = {}) {
      const handlers = new Set();
      let pending = null;
      let boundButton = null;
      let buttonClick = null;

      function register(handler) {
        if (typeof handler !== "function") throw new TypeError("A reload handler is required.");
        handlers.add(handler);
        return () => handlers.delete(handler);
      }

      function run() {
        if (pending) return pending;
        if (!handlers.size) return Promise.resolve();
        if (boundButton) boundButton.setAttribute("aria-busy", "true");
        if (typeof onBusyChange === "function") {
          try { onBusyChange(true); } catch (_) { /* Keep reload execution independent of UI callbacks. */ }
        }
        pending = Promise.all([...handlers].map(handler => Promise.resolve().then(handler)))
          .catch(error => {
            if (typeof onError === "function") {
              try { onError(error); } catch (_) { /* Keep keyboard reload errors handled. */ }
            }
          })
          .finally(() => {
            pending = null;
            if (boundButton) boundButton.setAttribute("aria-busy", "false");
            if (typeof onBusyChange === "function") {
              try { onBusyChange(false); } catch (_) { /* Keep busy cleanup reliable. */ }
            }
          });
        return pending;
      }

      function bindButton(nextButton) {
        if (boundButton && buttonClick) boundButton.removeEventListener("click", buttonClick);
        boundButton = nextButton || null;
        buttonClick = boundButton ? () => void run() : null;
        if (boundButton) boundButton.addEventListener("click", buttonClick);
      }

      function handleKeydown(event) {
        if (!event.ctrlKey || event.metaKey || event.altKey || event.shiftKey || event.key.toLowerCase() !== "r") return;
        event.preventDefault();
        void run();
      }

      if (button) bindButton(button);
      root.document.addEventListener("keydown", handleKeydown);

      return {
        register,
        bindButton,
        run,
        destroy() {
          root.document.removeEventListener("keydown", handleKeydown);
          if (boundButton && buttonClick) boundButton.removeEventListener("click", buttonClick);
          handlers.clear();
        },
      };
    },
  };
})(window);
