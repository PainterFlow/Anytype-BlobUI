(function (root) {
  "use strict";
  const UI = root.BlobUI;
  const C = UI.Components;
  UI.ScreenDefinitions = Object.assign(UI.ScreenDefinitions || {}, {
    "accent-heading-section": content => C.Section({
      attributes: {
        class: "settings-group accent-group",
        "aria-labelledby": "accent-heading"
      },
      content
    }),
    "accent-heading": () => C.Heading({
      id: "accent-heading",
      title: "Accent color",
      description: "Choose a color for highlights and active controls."
    }),
    "accent-heading-list": content => C.Group({
      content
    }),
    "include-selections": () => C.ToggleSetting({
      id: "include-selections",
      label: "Include selections",
      inputAttributes: {
        "aria-describedby": "include-selections-help"
      },
      description: {
        id: "include-selections-help",
        text: "Use the accent color for text, table and block selections, selected choices, and focus indicators."
      }
    }),
    "native-accent": () => C.ToggleSetting({
      id: "native-accent",
      label: "Anytype Accent Color"
    }),
    "kanban-heading-section": content => C.Section({
      attributes: {
        "aria-labelledby": "kanban-heading"
      },
      content
    }),
    "kanban-heading": () => C.Heading({
      id: "kanban-heading",
      title: "Kanban views",
      description: "Choose how boards are displayed."
    }),
    "kanban-heading-list": content => C.Group({
      attributes: {
        class: "settings-list kanban-settings-list"
      },
      content
    }),
    "kanban-heading-row": content => C.SettingsRow({
      attributes: {
        class: "kanban-setting-row kanban-style-row"
      },
      label: "Board layout",
      description: {
        attributes: {
          id: "kanban-style-help",
          class: "kanban-setting-description"
        },
        text: "Choose the spacing used on inline and full-page boards."
      },
      content
    }),
    "kanban-view-style": content => C.Button({
      attributes: {
        id: "kanban-view-style",
        class: "kanban-select-trigger",
        value: "default",
        "aria-haspopup": "listbox",
        "aria-expanded": "false",
        "aria-controls": "kanban-style-options",
        "aria-labelledby": "kanban-style-label kanban-style-value",
        "aria-describedby": "kanban-style-help"
      },
      content
    }),
    "kanban-style-options-button": content => C.Button({
      attributes: {
        class: "kanban-select-option",
        role: "option",
        "data-value": "default",
        "aria-selected": "true",
        tabindex: "-1"
      },
      content
    }),
    "kanban-style-options-button-2": content => C.Button({
      attributes: {
        class: "kanban-select-option",
        role: "option",
        "data-value": "balanced",
        "aria-selected": "false",
        tabindex: "-1"
      },
      content
    }),
    "kanban-style-options-button-3": content => C.Button({
      attributes: {
        class: "kanban-select-option",
        role: "option",
        "data-value": "compact",
        "aria-selected": "false",
        tabindex: "-1"
      },
      content
    }),
    "kanban-heading-row-2": content => C.SettingsRow({
      attributes: {
        class: "kanban-setting-row formula-display-row"
      },
      label: "Display formulas",
      description: {
        attributes: {
          id: "formula-display-help",
          class: "kanban-setting-description"
        },
        text: "Choose where category calculation results appear."
      },
      content
    }),
    "formula-display": content => C.Button({
      attributes: {
        id: "formula-display",
        class: "kanban-select-trigger",
        value: "balanced",
        "aria-haspopup": "listbox",
        "aria-expanded": "false",
        "aria-controls": "formula-display-options",
        "aria-labelledby": "formula-display-label formula-display-value",
        "aria-describedby": "formula-display-help"
      },
      content
    }),
    "formula-display-options-button": content => C.Button({
      attributes: {
        class: "kanban-select-option",
        role: "option",
        "data-value": "balanced",
        "aria-selected": "true",
        tabindex: "-1"
      },
      content
    }),
    "formula-display-options-button-2": content => C.Button({
      attributes: {
        class: "kanban-select-option",
        role: "option",
        "data-value": "spacious",
        "aria-selected": "false",
        tabindex: "-1"
      },
      content
    }),
    "collapsed-categories": () => C.ToggleSetting({
      id: "collapsed-categories",
      label: "Collapsed Categories",
      className: "native-accent-control settings-row kanban-setting-row",
      inputAttributes: {
        "aria-describedby": "kanban-collapse-help"
      }
    }),
    "open-formulas": content => C.Button({
      attributes: {
        id: "open-formulas",
        class: "formula-button button-label-centered"
      },
      content
    }),
    "calendar-heading-section": content => C.Section({
      attributes: {
        "aria-labelledby": "calendar-heading"
      },
      content
    }),
    "calendar-heading": () => C.Heading({
      id: "calendar-heading",
      title: "Calendar",
      description: "Color calendar objects by a property or their Calendar Color value."
    }),
    "calendar-heading-list": content => C.Group({
      content
    }),
    "calendar-colors-enabled": () => C.ToggleSetting({
      id: "calendar-colors-enabled",
      label: "Calendar Colors",
      inputAttributes: {
        checked: "",
        disabled: "",
        "aria-describedby": "calendar-colors-help"
      }
    }),
    "typography-heading-section": content => C.Section({
      attributes: {
        class: "settings-group typography-group",
        "aria-labelledby": "typography-heading"
      },
      content
    }),
    "typography-heading": () => C.Heading({
      id: "typography-heading",
      title: "Font",
      description: "Use the same fonts as your Anytype workspace."
    }),
    "typography-heading-fields": content => C.Group({
      attributes: {
        class: "font-fields"
      },
      content
    }),
    "font-family": content => C.TextField({
      tag: "label",
      labelTag: "span",
      labelAttributes: {},
      inputAttributes: {
        id: "font-family",
        type: "text",
        maxlength: "80",
        autocomplete: "off",
        spellcheck: "false",
        placeholder: "Inter"
      },
      labelContent: content
    }),
    "code-font-family": content => C.TextField({
      tag: "label",
      labelTag: "span",
      labelAttributes: {},
      inputAttributes: {
        id: "code-font-family",
        type: "text",
        maxlength: "80",
        autocomplete: "off",
        spellcheck: "false",
        placeholder: "Consolas"
      },
      labelContent: content
    }),
    "space-banner-heading-section": content => C.Section({
      attributes: {
        class: "settings-group space-banner-group",
        "aria-labelledby": "space-banner-heading"
      },
      content
    }),
    "space-banner-heading": () => C.Heading({
      id: "space-banner-heading",
      title: "Space banner",
      description: "Choose an image to show at the top of your Anytype sidebar."
    }),
    "choose-banner-image": content => C.Button({
      attributes: {
        id: "choose-banner-image",
        class: "banner-image-button"
      },
      content
    }),
    "reset-banner-image": content => C.Button({
      attributes: {
        id: "reset-banner-image",
        class: "banner-reset-button",
        hidden: ""
      },
      content
    }),
    "formula-search-toggle": content => C.IconButton({
      attributes: {
        id: "formula-search-toggle",
        class: "neutral-icon-button",
        "aria-label": "Search Kanban views",
        "aria-controls": "formula-search",
        "aria-expanded": "false"
      },
      content
    }),
    "formula-reload": content => C.IconButton({
      attributes: {
        id: "formula-reload",
        class: "neutral-icon-button",
        "aria-label": "Reload available views",
        title: "Reload available views"
      },
      content
    }),
    "formula-filter": content => C.IconButton({
      attributes: {
        id: "formula-filter",
        class: "neutral-icon-button",
        "aria-label": "Filter by space",
        title: "Filter by space",
        "aria-haspopup": "listbox",
        "aria-expanded": "false",
        "aria-controls": "formula-filter-options"
      },
      content
    }),
    "reduced-motion-heading-section": content => C.Section({
      attributes: {
        "aria-labelledby": "reduced-motion-heading"
      },
      content
    }),
    "reduced-motion-heading": () => C.Heading({
      id: "reduced-motion-heading",
      title: "Reduced Motion",
      description: "Stops BlobUI’s animated text effects to reduce motion."
    }),
    "reduced-motion-heading-list": content => C.Group({
      content
    }),
    "reduced-motion": () => C.ToggleSetting({
      id: "reduced-motion",
      label: "Reduced Motion",
      className: "native-accent-control settings-row accessibility-row"
    }),
    "hover-effects-heading-section": content => C.Section({
      attributes: {
        "aria-labelledby": "hover-effects-heading"
      },
      content
    }),
    "hover-effects-heading": () => C.Heading({
      id: "hover-effects-heading",
      title: "Hover Effects",
      description: "Choose which hover previews appear in Anytype."
    }),
    "hover-effects-heading-list": content => C.Group({
      content
    }),
    "tab-hover-effect": () => C.ToggleSetting({
      id: "tab-hover-effect",
      label: "Tab hover effect",
      className: "native-accent-control settings-row accessibility-row"
    }),
    "link-hover-effect": () => C.ToggleSetting({
      id: "link-hover-effect",
      label: "Links hover effect",
      className: "native-accent-control settings-row accessibility-row"
    }),
    "object-mention-hover-effect": () => C.ToggleSetting({
      id: "object-mention-hover-effect",
      label: "Object mention hover effect",
      className: "native-accent-control settings-row accessibility-row"
    }),
    "advanced-api-tab": content => C.Button({
      attributes: {
        id: "advanced-api-tab",
        role: "tab",
        "aria-selected": "true",
        "aria-controls": "advanced-api-panel"
      },
      content
    }),
    "advanced-app-tab": content => C.Button({
      attributes: {
        id: "advanced-app-tab",
        role: "tab",
        "aria-selected": "false",
        "aria-controls": "advanced-app-panel",
        tabindex: "-1"
      },
      content
    }),
    "advanced-experiments-tab": content => C.Button({
      attributes: {
        id: "advanced-experiments-tab",
        role: "tab",
        "aria-selected": "false",
        "aria-controls": "advanced-experiments-panel",
        tabindex: "-1"
      },
      content
    }),
    "advanced-experiments-panel": content => C.Section({
      attributes: {
        id: "advanced-experiments-panel",
        role: "tabpanel",
        "aria-labelledby": "advanced-experiments-tab",
        hidden: ""
      },
      content
    }),
    "experiments-heading": content => C.Group({
      attributes: {
        class: "section-heading"
      },
      content
    }),
    "advanced-experiments-tab-list": content => C.Group({
      content
    }),
    "settings-v21-enabled": () => C.ToggleSetting({
      id: "settings-v21-enabled",
      label: "Settings Future Proof UI Refresh",
      className: "native-accent-control settings-row accessibility-row",
      inputAttributes: {
        "aria-describedby": "experiments-warning"
      }
    }),
    "advanced-api-panel": content => C.Section({
      attributes: {
        id: "advanced-api-panel",
        role: "tabpanel",
        "aria-labelledby": "advanced-api-tab"
      },
      content
    }),
    "advanced-api-heading": () => C.Heading({
      id: "advanced-api-heading",
      title: "API key",
      description: "Configure the local API connection used by BlobUI."
    }),
    "advanced-api-tab-list": content => C.Group({
      attributes: {
        class: "settings-list advanced-fields"
      },
      content
    }),
    "advanced-api-key": content => C.TextField({
      attributes: {
        class: "input-field advanced-row"
      },
      inputAttributes: {
        id: "advanced-api-key",
        type: "password",
        autocomplete: "new-password",
        spellcheck: "false",
        maxlength: "4096",
        "aria-describedby": "advanced-api-key-hint advanced-api-error"
      },
      visibility: {
        buttonAttributes: {
          id: "advanced-api-key-visibility",
          class: "visibility-toggle",
          "aria-controls": "advanced-api-key",
          "aria-pressed": "false",
          "aria-label": "Show Anytype API key",
          title: "Show Anytype API key"
        }
      },
      labelContent: content
    }),
    "advanced-api-ip": content => C.TextField({
      attributes: {
        class: "input-field advanced-row"
      },
      inputAttributes: {
        id: "advanced-api-ip",
        type: "text",
        inputmode: "decimal",
        autocomplete: "off",
        spellcheck: "false",
        maxlength: "45",
        placeholder: "127.0.0.1",
        "aria-describedby": "advanced-api-error"
      },
      visibility: {
        buttonAttributes: {
          id: "advanced-api-ip-visibility",
          class: "visibility-toggle",
          "aria-controls": "advanced-api-ip",
          "aria-pressed": "true",
          "aria-label": "Hide API key IP",
          title: "Hide API key IP"
        }
      },
      labelContent: content
    }),
    "advanced-api-port": content => C.TextField({
      attributes: {
        class: "input-field advanced-row"
      },
      inputAttributes: {
        id: "advanced-api-port",
        type: "text",
        inputmode: "numeric",
        autocomplete: "off",
        spellcheck: "false",
        maxlength: "5",
        placeholder: "31009",
        "aria-describedby": "advanced-api-error"
      },
      visibility: {
        buttonAttributes: {
          id: "advanced-api-port-visibility",
          class: "visibility-toggle",
          "aria-controls": "advanced-api-port",
          "aria-pressed": "true",
          "aria-label": "Hide API key port",
          title: "Hide API key port"
        }
      },
      labelContent: content
    }),
    "advanced-app-panel": content => C.Section({
      attributes: {
        id: "advanced-app-panel",
        role: "tabpanel",
        "aria-labelledby": "advanced-app-tab",
        hidden: ""
      },
      content
    }),
    "anytype-installation-heading-section": content => C.Section({
      attributes: {
        "aria-labelledby": "anytype-installation-heading"
      },
      content
    }),
    "anytype-installation-heading": () => C.Heading({
      id: "anytype-installation-heading",
      title: "Anytype Folder",
      description: "Choose the Anytype data folder that contains custom.css."
    }),
    "advanced-app-tab-section": content => C.Section({
      attributes: {
        "aria-labelledby": "app-general-heading"
      },
      content
    }),
    "app-general-heading": () => C.Heading({ id: "app-general-heading", title: "General" }),
    "advanced-app-tab-list": content => C.Section({
      attributes: { class: "app-general-settings" },
      content
    }),
    "start-on-login": () => C.ToggleSetting({
      id: "start-on-login",
      label: "Launch at startup",
      className: "native-accent-control settings-row accessibility-row"
    }),
    "autoplay-videos": () => C.ToggleSetting({
        id: "autoplay-videos",
        label: "Autoplay videos",
        className: "native-accent-control settings-row accessibility-row"
      }),
      "updates-heading-section": content => C.Section({
      attributes: {
        "aria-labelledby": "updates-heading"
      },
      content
    }),
    "updates-heading": () => C.Heading({
      id: "updates-heading",
      title: "Updates",
      description: "Choose how BlobUI checks for and installs updates."
    }),
    "updates-heading-list": content => C.Group({
      content
    }),
    "automatic-updates-enabled": () => C.ToggleSetting({
      id: "automatic-updates-enabled",
      label: "Install updates automatically",
      className: "native-accent-control settings-row accessibility-row"
    }),
    "automatic-checks-enabled": () => C.ToggleSetting({
      id: "automatic-checks-enabled",
      label: "Check for updates automatically",
      className: "native-accent-control settings-row accessibility-row"
    }),
    "check-updates-button": content => C.Button({
      attributes: {
        id: "check-updates-button",
        class: "banner-image-button update-check-button",
        disabled: ""
      },
      content
    }),
    "backup-heading-section": content => C.Section({
      attributes: { "aria-labelledby": "backup-heading" },
      content
    }),
    "backup-heading": () => C.Heading({
      id: "backup-heading",
      title: "Backup and import"
    }),
    "create-blob-backup-button": content => C.Button({
      attributes: {
        id: "create-blob-backup-button",
        class: "banner-image-button blob-backup-button",
        disabled: ""
      },
      content
    }),
    "import-blob-backup-button": content => C.Button({
      attributes: {
        id: "import-blob-backup-button",
        class: "banner-image-button blob-backup-button",
        disabled: ""
      },
      content
    }),
    "app-backup-box": content => C.Group({
      attributes: { class: "settings-list app-backup-box" }, content
    }),
    "app-export-row": content => C.SettingsRow({
      attributes: { class: "settings-row app-backup-row" },
      label: "Export settings",
      description: { text: "Create a dated copy of the complete BlobUI folder, including settings and calculations.", attributes: { class: "app-backup-description" } },
      content
    }),
    "app-import-row": content => C.SettingsRow({
      attributes: { class: "settings-row app-backup-row" },
      label: "Import settings",
      description: { text: "Replace the current BlobUI folder with an exported backup.", attributes: { class: "app-backup-description" } },
      content
    }),
    "app-uninstall-heading": () => C.Heading({ id: "app-uninstall-heading", title: "Uninstall" }),
    "uninstall-button": content => C.Button({
      attributes: {
        id: "uninstall-button",
        class: "banner-image-button danger-button"
      },
      content
    }),
  });
})(window);
