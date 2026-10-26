(function (root) {
  "use strict";
  const UI = root.BlobUI;
  const C = UI.Components;
  UI.ScreenDefinitions = Object.assign(UI.ScreenDefinitions || {}, {
    "formula-calculator-backdrop": content => C.DialogShell({
      attributes: {
        id: "formula-calculator-backdrop"
      },
      dialogAttributes: {
        id: "formula-calculator-dialog",
        class: "confirm-dialog formula-calculator-dialog",
        "aria-label": "Kanban calculation"
      },
      content
    }),
    "formula-calculator-close": content => C.Button({
      attributes: {
        id: "formula-calculator-close",
        class: "confirm-close",
        "aria-label": "Close calculator",
        title: "Close"
      },
      content
    }),
    "uninstall-backdrop": content => C.DialogShell({
      attributes: {
        id: "uninstall-backdrop"
      },
      dialogAttributes: {
        id: "uninstall-dialog",
        role: "alertdialog",
        "aria-labelledby": "uninstall-title",
        "aria-describedby": "uninstall-message"
      },
      content
    }),
    "uninstall-close": content => C.Button({
      attributes: {
        id: "uninstall-close",
        class: "confirm-close",
        "aria-label": "Cancel uninstall"
      },
      content
    }),
    "uninstall-cancel": content => C.Button({
      attributes: {
        id: "uninstall-cancel",
        class: "confirm-cancel"
      },
      content
    }),
    "uninstall-confirm": content => C.Button({
      attributes: {
        id: "uninstall-confirm",
        class: "confirm-save danger-button"
      },
      content
    }),
    "blob-import-backdrop": content => C.DialogShell({
      attributes: { id: "blob-import-backdrop" },
      dialogAttributes: {
        id: "blob-import-dialog",
        role: "alertdialog",
        "aria-labelledby": "blob-import-title",
        "aria-describedby": "blob-import-message"
      },
      content
    }),
    "blob-import-close": content => C.Button({
      attributes: {
        id: "blob-import-close",
        class: "confirm-close",
        "aria-label": "Cancel backup import"
      },
      content
    }),
    "blob-import-cancel": content => C.Button({
      attributes: {
        id: "blob-import-cancel",
        class: "confirm-cancel"
      },
      content
    }),
    "blob-import-confirm": content => C.Button({
      attributes: {
        id: "blob-import-confirm",
        class: "confirm-save"
      },
      content
    }),
    "api-key-removal-backdrop": content => C.DialogShell({
      attributes: {
        id: "api-key-removal-backdrop"
      },
      dialogAttributes: {
        id: "api-key-removal-dialog",
        role: "alertdialog",
        "aria-labelledby": "api-key-removal-title",
        "aria-describedby": "api-key-removal-message"
      },
      content
    }),
    "api-key-removal-close": content => C.Button({
      attributes: {
        id: "api-key-removal-close",
        class: "confirm-close",
        "aria-label": "Undo all unsaved changes",
        title: "Undo all changes"
      },
      content
    }),
    "api-key-removal-undo": content => C.Button({
      attributes: {
        id: "api-key-removal-undo",
        class: "confirm-undo"
      },
      content
    }),
    "api-key-removal-save": content => C.Button({
      attributes: {
        id: "api-key-removal-save",
        class: "confirm-save"
      },
      content
    }),
    "discard-confirmation-backdrop": content => C.DialogShell({
      attributes: {
        id: "discard-confirmation-backdrop"
      },
      dialogAttributes: {
        id: "discard-confirmation-dialog",
        role: "alertdialog",
        "aria-labelledby": "discard-confirmation-title",
        "aria-describedby": "discard-confirmation-message"
      },
      content
    }),
    "discard-confirmation-close": content => C.Button({
      attributes: {
        id: "discard-confirmation-close",
        class: "confirm-close",
        "aria-label": "Close without undoing",
        title: "Close"
      },
      content
    }),
    "discard-confirmation-undo": content => C.Button({
      attributes: {
        id: "discard-confirmation-undo",
        class: "confirm-undo"
      },
      content
    }),
    "release-notes-backdrop": content => C.DialogShell({
      attributes: {
        id: "release-notes-backdrop"
      },
      dialogAttributes: {
        id: "release-notes-dialog",
        class: "confirm-dialog release-notes-dialog",
        "aria-label": "Changelog"
      },
      content
    }),
    "release-notes-previous": content => C.Button({
      attributes: {
        id: "release-notes-previous",
        class: "release-notes-nav-button",
        disabled: ""
      },
      content
    }),
    "release-notes-after": content => C.Button({
      attributes: {
        id: "release-notes-after",
        class: "release-notes-nav-button",
        hidden: ""
      },
      content
    }),
    "help-error-backdrop": content => C.DialogShell({
      attributes: {
        id: "help-error-backdrop"
      },
      dialogAttributes: {
        id: "help-error-dialog",
        class: "confirm-dialog help-error-dialog",
        role: "alertdialog",
        "aria-labelledby": "help-error-title",
        "aria-describedby": "help-error-message"
      },
      content
    }),
    "help-error-close": content => C.Button({
      attributes: {
        id: "help-error-close",
        class: "confirm-close",
        "aria-label": "Close error"
      },
      content
    }),
    "keybinds-backdrop": content => C.DialogShell({
      attributes: {
        id: "keybinds-backdrop"
      },
      dialogAttributes: {
        id: "keybinds-dialog",
        class: "confirm-dialog keybinds-dialog",
        "aria-labelledby": "keybinds-title",
        "aria-describedby": "keybinds-description"
      },
      content
    }),
    "keybinds-close": content => C.Button({
      attributes: {
        id: "keybinds-close",
        class: "confirm-close",
        "aria-label": "Close keyboard shortcuts",
        title: "Close"
      },
      content
    }),
  });
})(window);
