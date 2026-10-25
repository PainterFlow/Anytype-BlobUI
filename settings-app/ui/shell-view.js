(function (root) {
  "use strict";
  const UI = root.BlobUI;
  const C = UI.Components;
  UI.ScreenDefinitions = Object.assign(UI.ScreenDefinitions || {}, {
    "documentation-back": content => C.Button({
      attributes: {
        id: "documentation-back",
        class: "window-control",
        "aria-label": "Back to settings",
        title: "Back to settings",
        hidden: ""
      },
      content
    }),
    "window-minimize": content => C.Button({
      attributes: {
        id: "window-minimize",
        class: "window-control",
        "aria-label": "Minimize",
        title: "Minimize"
      },
      content
    }),
    "window-close": content => C.Button({
      attributes: {
        id: "window-close",
        class: "window-control window-close",
        "aria-label": "Close to background",
        title: "Close to background. Numpad 1 reopens Settings in Anytype. Ctrl+Q quits."
      },
      content
    }),
    "documentation-retry": content => C.Button({
      attributes: {
        id: "documentation-retry",
        class: "banner-image-button"
      },
      content
    }),
    "preferences-nav": content => C.Button({
      attributes: {
        id: "preferences-nav",
        class: "nav-item active",
        "aria-current": "page",
        "data-settings-page": "preferences"
      },
      content
    }),
    "formulas-nav": content => C.Button({
      attributes: {
        id: "formulas-nav",
        class: "nav-item",
        "data-settings-page": "formulas"
      },
      content
    }),
    "accessibility-nav": content => C.Button({
      attributes: {
        id: "accessibility-nav",
        class: "nav-item",
        "data-settings-page": "accessibility"
      },
      content
    }),
    "advanced-nav": content => C.Button({
      attributes: {
        id: "advanced-nav",
        class: "nav-item",
        "data-settings-page": "advanced"
      },
      content
    }),
    "save-button": content => C.Button({
      attributes: {
        id: "save-button",
        class: "sidebar-action save-button",
        title: "Save changes",
        hidden: "",
        disabled: ""
      },
      content
    }),
    "discard-button": content => C.Button({
      attributes: {
        id: "discard-button",
        class: "sidebar-action discard-button",
        title: "Undo changes",
        "aria-label": "Undo changes",
        hidden: "",
        disabled: ""
      },
      content
    }),
    "update-now-button": content => C.Button({
      attributes: {
        id: "update-now-button",
        class: "sidebar-action update-now-button",
        title: "Update now",
        "aria-label": "Update now",
        hidden: ""
      },
      content
    }),
    "log-button": content => C.Button({
      attributes: {
        id: "log-button",
        class: "sidebar-action log-button",
        title: "Logs",
        "aria-expanded": "false",
        "aria-controls": "log-popup",
        hidden: ""
      },
      content
    }),
    "copy-logs": content => C.Button({
      attributes: {
        id: "copy-logs",
        class: "copy-logs",
        "aria-label": "Copy all logs",
        title: "Copy all logs"
      },
      content
    }),
    "help-button": content => C.Button({
      attributes: {
        id: "help-button",
        class: "sidebar-action help-button",
        title: "Help",
        "aria-label": "Help",
        "aria-expanded": "false",
        "aria-controls": "help-popup"
      },
      content
    }),
    "help-whats-new": content => C.Button({
      attributes: {
        id: "help-whats-new",
        class: "help-item"
      },
      content
    }),
    "help-documentation": content => C.Button({
      attributes: {
        id: "help-documentation",
        class: "help-item"
      },
      content
    }),
    "help-keybinds": content => C.Button({
      attributes: {
        id: "help-keybinds",
        class: "help-item"
      },
      content
    }),
    "open-author": content => C.Button({
      attributes: {
        id: "open-author",
        class: "help-credit",
        "aria-label": "Made by @PainterFlow11",
        title: "https://github.com/PainterFlow/Anytype-BlobUI"
      },
      content
    }),
    "open-repository": content => C.Button({
      attributes: {
        id: "open-repository",
        class: "help-credit",
        "aria-label": "Licensed under GPL V3",
        title: "https://github.com/PainterFlow/Anytype-BlobUI"
      },
      content
    }),
  });
})(window);
