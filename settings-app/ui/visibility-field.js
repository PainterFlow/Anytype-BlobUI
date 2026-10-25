(function (root) {
  "use strict";

  function bind(options) {
    const input = options.input;
    const button = options.button;
    const field = options.field;
    let visible = Boolean(options.defaultVisible);
    let requestVersion = 0;
    let busy = false;

    function sync() {
      field.dataset.filled = String(input.value.length > 0);
    }

    function setVisible(nextVisible) {
      visible = Boolean(nextVisible);
      input.type = visible ? "text" : "password";
      button.setAttribute("aria-pressed", String(visible));
      const label = visible ? options.hideLabel : options.showLabel;
      button.setAttribute("aria-label", label);
      button.setAttribute("title", label);
      sync();
    }

    function invalidate() {
      requestVersion += 1;
      if (busy) {
        busy = false;
        if (options.onBusyChange) options.onBusyChange(false);
      }
    }

    function reset() {
      invalidate();
      setVisible(options.defaultVisible);
    }

    input.addEventListener("input", () => {
      invalidate();
      sync();
    });
    button.addEventListener("mousedown", (event) => {
      if (document.activeElement === input) event.preventDefault();
    });
    button.addEventListener("click", async (event) => {
      event.preventDefault();
      if (button.disabled || busy) return;

      const inputWasFocused = document.activeElement === input;
      const selectionStart = inputWasFocused ? input.selectionStart : null;
      const selectionEnd = inputWasFocused ? input.selectionEnd : null;

      if (!visible && options.shouldReveal && options.shouldReveal()) {
        busy = true;
        const version = ++requestVersion;
        const previousValue = input.value;
        if (options.onBusyChange) options.onBusyChange(true);
        try {
          const value = await options.reveal();
          if (version !== requestVersion || input.value !== previousValue || visible || (options.shouldReveal && !options.shouldReveal())) return;
          input.value = value;
        } catch (error) {
          if (version === requestVersion && input.value === previousValue && (!options.shouldReveal || options.shouldReveal()) && options.onRevealError) {
            options.onRevealError(error);
          }
          return;
        } finally {
          if (version === requestVersion) {
            busy = false;
            if (options.onBusyChange) options.onBusyChange(false);
          }
        }
      } else if (visible && options.onHide) {
        options.onHide();
      }

      if (busy) return;
      setVisible(!visible);
      if (inputWasFocused && selectionStart !== null && selectionEnd !== null && typeof input.setSelectionRange === "function") {
        try { input.setSelectionRange(selectionStart, selectionEnd); } catch (_) { /* Some input types do not expose a selection. */ }
      }
    });

    setVisible(visible);
    return { input, button, field, reset, sync, invalidate };
  }

  root.VisibilityField = { bind };
})(window);
