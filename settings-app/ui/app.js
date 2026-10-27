(function () {
  "use strict";

  // Retained only to read and save colors from earlier settings versions.
  const legacyAccents = [
    { key: "native", label: "Anytype", value: "Default", swatch: "native", mode: "native" },
    { key: "blob-pink", label: "Blob Pink", value: "Blob Pink", swatch: "#DE50DE", mode: "preset" },
    { key: "grey", label: "Grey", value: "Grey", swatch: "#888888", darkSwatch: "#a8a8a8", mode: "preset" },
    { key: "yellow", label: "Yellow", value: "Yellow", swatch: "#c49a0e", darkSwatch: "#ecd91a", mode: "preset" },
    { key: "amber", label: "Amber", value: "Amber", swatch: "#c07830", darkSwatch: "#f0a820", mode: "preset" },
    { key: "red", label: "Red", value: "Red", swatch: "#d04520", darkSwatch: "#f25040", mode: "preset" },
    { key: "pink", label: "Pink", value: "Pink", swatch: "#d4267a", darkSwatch: "#f040a0", mode: "preset" },
    { key: "purple", label: "Purple", value: "Purple", swatch: "#9b3fe0", darkSwatch: "#c870e8", mode: "preset" },
    { key: "blue", label: "Blue", value: "Blue", swatch: "#4f6af0", darkSwatch: "#6878ee", mode: "preset" },
    { key: "sky", label: "Sky", value: "Sky", swatch: "#1a8ac4", darkSwatch: "#40b8f5", mode: "preset" },
    { key: "teal", label: "Teal", value: "Teal", swatch: "#009688", darkSwatch: "#20c8b8", mode: "preset" },
    { key: "green", label: "Green", value: "Green", swatch: "#4aad00", darkSwatch: "#4ecc4e", mode: "preset" },
  ];

  const el = (id) => document.getElementById(id);
  const plane = el("color-plane");
  const hueInput = el("accent-hue");
  const saturationInput = el("accent-saturation");
  const brightnessInput = el("accent-brightness");
  const includeSelections = el("include-selections");
  const nativeAccent = el("native-accent");
  const swatch = el("accent-swatch");
  const accentValue = el("accent-value");
  const fontFamily = el("font-family");
  const codeFontFamily = el("code-font-family");
  const bannerImageInput = el("banner-image-input");
  const chooseBannerImage = el("choose-banner-image");
  const resetBannerImage = el("reset-banner-image");
  const spacePreviewImage = el("space-preview-image");
  const reducedMotion = el("reduced-motion");
  const tabHoverEffect = el("tab-hover-effect");
  const linkHoverEffect = el("link-hover-effect");
  const objectMentionHoverEffect = el("object-mention-hover-effect");
  const kanbanViewStyle = el("kanban-view-style");
  const kanbanSelect = el("kanban-select");
  const calendarColorsEnabled = el("calendar-colors-enabled");
  let calendarApiAvailability = "checking";
  let calendarApiRequest = 0;
  const collapsedCategories = el("collapsed-categories");
  const preferencesNav = el("preferences-nav");
  const accessibilityNav = el("accessibility-nav");
  const advancedNav = el("advanced-nav");
  const preferencesPage = el("preferences-page");
  const accessibilityPage = el("accessibility-page");
  const advancedPage = el("advanced-page");
  const kanbanPicker = window.BlobUI.Components.Picker({ root: kanbanSelect });
  function closeKanbanStyleMenu(restoreFocus = false) { kanbanPicker.close(restoreFocus); }
  function syncKanbanStyleSelection(value) {
    const selected = ["default", "balanced", "compact"].includes(value) ? value : "default";
    kanbanPicker.setValue(selected);
    return selected;
  }
  const advancedTabs = [el("advanced-api-tab"), el("advanced-app-tab"), el("advanced-experiments-tab")];
  const advancedTabController = window.BlobUI.Components.Tabs({
    container: el("advanced-tabs"),
    buttons: advancedTabs,
    panels: [el("advanced-api-panel"), el("advanced-app-panel"), el("advanced-experiments-panel")],
    onSelect(index) { if (index !== 0) resetAdvancedVisibility(); },
  });
  function selectAdvancedTab(index) { advancedTabController.select(index); }
  const installDirectory = el("install-directory");
  const startOnLogin = el("start-on-login");
  const autoplayVideos = el("autoplay-videos");
  function applyAutoplay(value) {
    if (autoplayVideos) autoplayVideos.checked = value !== false;
    window.BlobUI.VideoPlayer?.setAutoplay(value !== false);
  }
  const automaticUpdatesEnabled = el("automatic-updates-enabled");
  const automaticChecksEnabled = el("automatic-checks-enabled");
  const checkUpdatesButton = el("check-updates-button");
  const updateNowButton = el("update-now-button");
  const createBlobBackupButton = el("create-blob-backup-button");
  const importBlobBackupButton = el("import-blob-backup-button");
  const blobBackupStatus = el("blob-backup-status");
  const sidebarActions = el("sidebar-actions");
  const advancedApiKey = el("advanced-api-key");
  const advancedApiIp = el("advanced-api-ip");
  const advancedApiPort = el("advanced-api-port");
  const apiKeyRemovalBackdrop = el("api-key-removal-backdrop");
  const apiKeyRemovalClose = el("api-key-removal-close");
  const apiKeyRemovalUndo = el("api-key-removal-undo");
  const apiKeyRemovalSave = el("api-key-removal-save");
  const apiKeyRemovalModal = window.BlobUI.Components.Modal({
    backdrop: apiKeyRemovalBackdrop,
    dialog: el("api-key-removal-dialog"),
    initialFocus: apiKeyRemovalClose,
    fallbackFocus: el("page-title"),
    inertTargets: [el("settings-shell"), el("title-drag-region")],
  });
  const discardConfirmationModal = window.BlobUI.Components.Modal({
    backdrop: el("discard-confirmation-backdrop"),
    dialog: el("discard-confirmation-dialog"),
    initialFocus: el("discard-confirmation-close"),
    fallbackFocus: el("page-title"),
    inertTargets: [el("settings-shell"), el("title-drag-region")],
  });
  const releaseNotesContent = el("release-notes-content");
  const releaseNotesPrevious = el("release-notes-previous");
  const releaseNotesAfter = el("release-notes-after");
  let releaseNotesVersion = null;
  let releaseNotesCurrentVersion = null;
  let releaseNotesLatestVersion = null;
  let releaseNotesLatestVersionRequest = 0;
  let displayedReleaseNotes = null;
  let previousReleaseNotes = null;
  let newerReleaseNotes = [];
  let releaseNotesHistoryRequest = 0;
  let releaseNotesNavigationBusy = false;
  let acknowledgeReleaseNotesOnClose = true;
  let releaseNotesLoading = false;
  let releaseNotesLoaded = false;
  let releaseNotesFailed = false;
  const releaseNotesModal = window.BlobUI.Components.Modal({
    backdrop: el("release-notes-backdrop"),
    dialog: el("release-notes-dialog"),
    initialFocus: el("release-notes-dialog"),
    fallbackFocus: el("help-button"),
    inertTargets: [el("settings-shell"), el("title-drag-region")],
    onClose: () => {
      window.BlobUI.VideoPlayer?.destroyTree(releaseNotesContent);
      const version = releaseNotesVersion;
      releaseNotesVersion = null;
      releaseNotesCurrentVersion = null;
      releaseNotesLatestVersion = null;
      releaseNotesLatestVersionRequest += 1;
      updateReleaseNotesHeading("");
      if (!version || !acknowledgeReleaseNotesOnClose) {
        acknowledgeReleaseNotesOnClose = true;
        return;
      }
      invoke("acknowledge_release_notes", { version }).then((acknowledged) => {
        if (acknowledged) runAutomaticUpdateCheck();
        else {
          releaseNotesFailed = true;
          setStatus("BlobUI could not confirm that these release notes were dismissed. Restart Settings to try again.", "error");
        }
      }).catch((error) => {
        releaseNotesFailed = true;
        setStatus("BlobUI could not save the release notes status. " + (error.message || String(error)), "error");
      });
    },
  });
  const helpErrorModal = window.BlobUI.Components.Modal({
    backdrop: el("help-error-backdrop"), dialog: el("help-error-dialog"),
    initialFocus: el("help-error-close"), fallbackFocus: el("help-button"),
    inertTargets: [el("settings-shell"), el("title-drag-region")],
  });
  const keybindModal = window.BlobUI.Components.Modal({
    backdrop: el("keybinds-backdrop"), dialog: el("keybinds-dialog"),
    initialFocus: () => el("keybind-list").querySelector("button"), fallbackFocus: el("help-button"),
    inertTargets: [el("settings-shell"), el("title-drag-region")],
    onClose: () => {
      shortcutCaptureAction = null;
      invoke("set_shortcut_capture", { active: false }).catch(() => {});
    },
  });
  window.BlobUI.openDiscardConfirmation = () => {
    if (!saveBusy && !isInstallingUpdate()) discardConfirmationModal.open();
  };
  window.BlobUI.closeDiscardConfirmation = (options) => discardConfirmationModal.close(options);
  const settingsShell = el("settings-shell");
  const SAVED_KEY_MASK = "••••••••";
  const pageTitle = el("page-title");
  const saveButton = el("save-button");
  const discardButton = el("discard-button");
  const saveLabel = saveButton.querySelector(".save-label");
  const statusMessage = el("status-message");
  const logButton = el("log-button");
  const logPopup = el("log-popup");
  const logEntries = el("log-entries");
  const pageContent = document.querySelector(".page-content");
  const logs = [];
  const root = document.documentElement;

  const visibilityFields = Object.create(null);

  function resetVisibilityField(inputId) {
    const field = visibilityFields[inputId];
    if (!field) return;
    field.reset();
  }

  function resetAdvancedVisibility() {
    if (hasSavedApiKey && !advancedKeyEdited) advancedApiKey.value = SAVED_KEY_MASK;
    ["advanced-api-key", "advanced-api-ip", "advanced-api-port"].forEach(resetVisibilityField);
  }

  function bindVisibilityField(inputId, buttonId, fieldId, showLabel, hideLabel, defaultVisible, hooks = {}) {
    visibilityFields[inputId] = window.BlobUI.Components.Visibility({
      input: el(inputId),
      button: el(buttonId),
      field: el(fieldId),
      showLabel,
      hideLabel,
      defaultVisible,
      ...hooks,
    });
    return visibilityFields[inputId];
  }

  bindVisibilityField("connection-key", "connection-key-visibility", "connection-key-field", "Show API key", "Hide API key", false);
  bindVisibilityField("connection-port", "connection-port-visibility", "connection-port-field", "Show port", "Hide port", true);
  bindVisibilityField("advanced-api-key", "advanced-api-key-visibility", "advanced-api-key-field", "Show Anytype API key", "Hide Anytype API key", false, {
    shouldReveal: () => hasSavedApiKey && !advancedKeyEdited && advancedPage.hidden === false,
    reveal: () => invoke("reveal_api_key"),
    onHide: () => {
      if (hasSavedApiKey && !advancedKeyEdited) advancedApiKey.value = SAVED_KEY_MASK;
    },
    onRevealError: () => setStatus("Could not reveal the saved API key. Try entering it again.", "error"),
    onBusyChange: (busy) => { el("advanced-api-key-visibility").disabled = busy || saveBusy || isInstallingUpdate() || !draft; },
  });
  bindVisibilityField("advanced-api-ip", "advanced-api-ip-visibility", "advanced-api-ip-field", "Show API key IP", "Hide API key IP", true);
  bindVisibilityField("advanced-api-port", "advanced-api-port-visibility", "advanced-api-port-field", "Show API key port", "Hide API key port", true);

  let draft = null;
  let initialDraft = null;
  let saveBusy = false;
  let hasSavedApiKey = false;
  let advancedKeyEdited = false;
  let initialApiDraft = { ip: "", port: "" };
  let currentApiIp = "";
  let currentApiPort = 31009;
  let updaterDraft = null;
  let initialUpdaterSettings = null;
  let updaterState = null;
  let updaterSettingsLoaded = false;
  let updaterBusyOperation = "";
  let blobFolderBusy = false;
  let blobRestoreReloadFailed = false;
  let uninstallBusy = false;
  let selectedBlobBackupDirectory = null;
  let pendingAutomaticUpdate = false;
  let connectionBusy = false;
  let simulationActive = false;
  let hsv = { h: 300, s: 64, v: 87 };
  let lastCustomAccent = null;
  let lastTheme = "";
  let defaultBannerPreview = "";
  let bannerReadGeneration = 0;
  let bannerReadPending = false;
  const editSession = window.BlobUI.createEditSession({
    read: readEditSnapshot,
    restore: restoreEditSnapshot,
    equals: editSnapshotsEqual,
  });

  window.BlobUI.discardChanges = function discardChanges() {
    if (saveBusy || isInstallingUpdate() || !draft) return;
    editSession.discard();
  };

  const scrollbars = [];
  function setupScrollbar(container, contentSelector) {
    const controller = window.BlobUI.Scrollbar.attach(container, contentSelector);
    scrollbars.push(controller);
    return controller.update;
  }
  window.addEventListener("unload", () => scrollbars.forEach(controller => controller.destroy()));

  setupScrollbar(el("sidebar-nav"), ".nav-item");
  setupScrollbar(el("keybinds-content"), "#keybind-list");
  const updatePageScrollbar = setupScrollbar(pageContent, ".settings-content");
  const updateReleaseNotesScrollbar = setupScrollbar(el("release-notes-dialog"), "#release-notes-content");
  const updateConnectionScrollbar = setupScrollbar(el("connection-scroll"), ".connection-content");
  const updateOnboardingScrollbar = setupScrollbar(el("onboarding-scroll"), ".onboarding-options");
  const updateLogsScrollbar = setupScrollbar(logEntries, ".log-entry");
  window.addEventListener("resize", () => {
    updatePageScrollbar();
    updateOnboardingScrollbar();
    updateConnectionScrollbar();
  });

  function invoke(name, args) {
    const api = window.__TAURI__ && window.__TAURI__.core;
    if (!api || typeof api.invoke !== "function") {
      return Promise.reject(new Error("The settings app bridge is unavailable. Launch BlobUI Settings using blobui-settings.exe."));
    }
    return api.invoke(name, args || {});
  }

  function themeSwatch(preset) {
    if (preset.swatch === "native") return "#377aff";
    return root.dataset.theme === "light" ? preset.swatch : (preset.darkSwatch || preset.swatch);
  }

  function setAccentPreview(value) {
    const candidate = String(value || "").trim();
    if (candidate && window.CSS && CSS.supports("color", candidate)) {
      root.style.setProperty("--accent", candidate);
    }
  }

  function previewForPreset(preset) {
    if (preset.key === "native") return "#377aff";
    if (preset.key === "blob-pink") return "#de50de";
    const cssNames = { grey: "var(--color-grey)", yellow: "var(--color-yellow)", amber: "var(--color-amber, var(--color-orange))", red: "var(--color-red)", pink: "var(--color-pink)", purple: "var(--color-purple)", blue: "var(--color-blue)", sky: "var(--color-sky, var(--color-ice))", teal: "var(--color-teal)", green: "var(--color-green, var(--color-lime))" };
    return cssNames[preset.key] || preset.value;
  }

  function toHexColor(value) {
    const match = String(value).match(/^#([\da-f]{3}|[\da-f]{6})$/i);
    if (!match) return null;
    if (match[1].length === 3) return "#" + match[1].split("").map((part) => part + part).join("").toLowerCase();
    return "#" + match[1].toLowerCase();
  }

  function hsvToHex(h, s, v) {
    const saturation = s / 100;
    const value = v / 100;
    const channel = (n) => {
      const k = (n + h / 60) % 6;
      return Math.round(255 * (value - value * saturation * Math.max(0, Math.min(k, 4 - k, 1))));
    };
    return "#" + [channel(5), channel(3), channel(1)].map((n) => n.toString(16).padStart(2, "0")).join("");
  }

  function hexToHsv(hex) {
    const channels = [1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255);
    const [r, g, b] = channels;
    const max = Math.max(...channels);
    const min = Math.min(...channels);
    const delta = max - min;
    let h = 0;
    if (delta) {
      h = max === r ? ((g - b) / delta) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
      h = (h * 60 + 360) % 360;
    }
    return { h, s: max ? delta / max * 100 : 0, v: max * 100 };
  }

  function colorPickerValue(raw) {
    const hex = toHexColor(raw);
    if (hex) return hex;
    const value = String(raw).trim();
    const rgb = value.match(/^rgba?\(\s*([^()]+)\s*\)$/i);
    if (rgb && isCoreSupportedCustomAccent(value)) {
      const channels = rgb[1].split(/[,/\s]+/).filter(Boolean).slice(0, 3);
      return "#" + channels.map((part) => Math.round(part.endsWith("%") ? Number(part.slice(0, -1)) / 100 * 255 : Number(part)).toString(16).padStart(2, "0")).join("");
    }
    const legacy = legacyAccents.find((item) => item.value.toLowerCase() === value.toLowerCase() || previewForPreset(item).toLowerCase() === value.toLowerCase());
    return legacy ? themeSwatch(legacy) : "#de50de";
  }

  function renderPicker(syncColor) {
    if (!draft) return;
    const native = draft.accentMode === "native";
    const value = native ? "#377aff" : draft.accentMode === "preset"
      ? previewForPreset(legacyAccents.find((item) => item.key === draft.accentPreset)) : draft.accentColor;
    if (syncColor) hsv = hexToHsv(colorPickerValue(value));
    hueInput.value = String(Math.round(hsv.h) % 360);
    hueInput.setAttribute("aria-valuetext", hueInput.value + " degrees");
    saturationInput.value = String(Math.round(hsv.s));
    brightnessInput.value = String(Math.round(hsv.v));
    plane.style.setProperty("--picker-hue", hsvToHex(hsv.h, 100, 100));
    el("color-cursor").style.left = hsv.s + "%";
    el("color-cursor").style.top = (100 - hsv.v) + "%";
    swatch.style.background = value;
    nativeAccent.checked = native;
    el("color-editor").classList.toggle("native-active", native);
    setAccentPreview(value);
  }

  function chooseCustomColor() {
    if (!draft || saveBusy || isInstallingUpdate()) return;
    const value = hsvToHex(hsv.h, hsv.s, hsv.v);
    draft.accentMode = "custom";
    draft.accentPreset = "";
    draft.accentColor = value;
    accentValue.value = draft.accentColor;
    nativeAccent.checked = false;
    accentValue.classList.toggle("invalid", value === "#ffffff");
    el("accent-error").textContent = value === "#ffffff" ? "Pure white is reserved. Choose a slightly darker color." : "";
    renderPicker(false);
    markDirty();
  }

  function isCoreSupportedCustomAccent(raw) {
    const value = String(raw || "").trim();
    if (/^#(?:[\da-f]{3}|[\da-f]{6})$/i.test(value)) return !/^#(?:f{3}|f{6})$/i.test(value);
    if ([
      "var(--color-system-accent-100)",
      "var(--color-grey)", "var(--color-yellow)", "var(--color-amber, var(--color-orange))",
      "var(--color-red)", "var(--color-pink)", "var(--color-purple)", "var(--color-blue)",
      "var(--color-sky, var(--color-ice))", "var(--color-teal)", "var(--color-green, var(--color-lime))",
    ].some((token) => token.toLowerCase() === value.toLowerCase())) return true;

    function inRange(component, maximum) {
      const percent = component.endsWith("%");
      const number = percent ? component.slice(0, -1) : component;
      if (!/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(number)) return false;
      const parsed = Number(number);
      return Number.isFinite(parsed) && parsed >= 0 && parsed <= (percent ? 100 : maximum);
    }

    const match = value.match(/^(?:rgb|rgba)\(\s*([^()]+)\s*\)$/i);
    if (!match) return false;
    const body = match[1].trim();
    let channels;
    let alpha = "";
    if (body.includes(",")) {
      if (body.includes("/")) return false;
      const parts = body.split(",").map((part) => part.trim());
      if (parts.length !== 3 && parts.length !== 4) return false;
      channels = parts.slice(0, 3);
      alpha = parts[3] || "";
    } else {
      const parts = body.match(/^(\S+)\s+(\S+)\s+(\S+)(?:\s*\/\s*(\S+))?$/);
      if (!parts) return false;
      channels = parts.slice(1, 4);
      alpha = parts[4] || "";
    }
    if (new Set(channels.map((part) => part.endsWith("%"))).size > 1) return false;
    if (!channels.every((part) => inRange(part, 255))) return false;
    if (alpha && !inRange(alpha, 1)) return false;
    const white = channels.every((part) => Number(part.replace(/%$/, "")) === (part.endsWith("%") ? 100 : 255));
    return !white;
  }

  function normalizeForUi(settings) {
    const value = String(settings.accentColor || "#DE50DE").trim();
    const lowered = value.toLowerCase();
    const tokens = {
      "default": "native",
      "var(--color-system-accent-100)": "native",
      "blob pink": "blob-pink",
      "#de50de": "blob-pink",
      "grey": "grey", "var(--color-grey)": "grey",
      "yellow": "yellow", "var(--color-yellow)": "yellow",
      "amber": "amber", "var(--color-amber, var(--color-orange))": "amber",
      "red": "red", "var(--color-red)": "red",
      "pink": "pink", "var(--color-pink)": "pink",
      "purple": "purple", "var(--color-purple)": "purple",
      "blue": "blue", "var(--color-blue)": "blue",
      "sky": "sky", "var(--color-sky, var(--color-ice))": "sky",
      "teal": "teal", "var(--color-teal)": "teal",
      "green": "green", "var(--color-green, var(--color-lime))": "green",
    };
    if (isCoreSupportedCustomAccent(value)) {
      return { accentMode: lowered === "var(--color-system-accent-100)" ? "native" : "custom", accentPreset: "", accentColor: value };
    }
    const presetKey = tokens[lowered];
    if (presetKey) {
      const preset = legacyAccents.find((item) => item.key === presetKey);
      return {
        accentMode: preset.mode,
        accentPreset: preset.mode === "preset" ? preset.key : "",
        accentColor: preset.value,
      };
    }
    return { accentMode: "custom", accentPreset: "", accentColor: value };
  }

  function settingsEqual(left, right) {
    function accentIdentity(settings) {
      if (settings.accentMode === "native") return "native";
      const preset = legacyAccents.find((item) => item.key === settings.accentPreset);
      const value = settings.accentMode === "preset" && preset
        ? previewForPreset(preset) : settings.accentColor;
      return toHexColor(value) || String(value).trim().toLowerCase();
    }
    return !!left && !!right && left.fontFamily === right.fontFamily && left.codeFontFamily === right.codeFontFamily && accentIdentity(left) === accentIdentity(right) && left.includeSelections === right.includeSelections && left.bannerImage === right.bannerImage && left.reducedMotion === right.reducedMotion && left.tabHoverEffect === right.tabHoverEffect && left.linkHoverEffect === right.linkHoverEffect && left.objectMentionHoverEffect === right.objectMentionHoverEffect && left.kanbanViewStyle === right.kanbanViewStyle && left.collapsedCategories === right.collapsedCategories && left.calendarColorsEnabled === right.calendarColorsEnabled && left.settingsV21Enabled === right.settingsV21Enabled;
  }

  function updaterSettingsEqual(left, right) {
    return !!left && !!right
      && left.installDirectory === right.installDirectory
      && left.startOnLogin === right.startOnLogin
      && left.autoplayVideos === right.autoplayVideos
      && left.automaticUpdatesEnabled === right.automaticUpdatesEnabled
      && left.automaticChecksEnabled === right.automaticChecksEnabled;
  }

  function copyUpdaterSettings(settings) {
    const directory = String(settings && settings.installDirectory || "");
    const displayDirectory = directory.startsWith("\\\\?\\UNC\\")
      ? "\\\\" + directory.slice(8)
      : directory.startsWith("\\\\?\\") ? directory.slice(4) : directory;
    return {
      installDirectory: displayDirectory,
      startOnLogin: !!(settings && settings.startOnLogin),
      autoplayVideos: settings?.autoplayVideos !== false,
      automaticUpdatesEnabled: !!(settings && settings.automaticUpdatesEnabled),
      automaticChecksEnabled: !!(settings && settings.automaticChecksEnabled),
    };
  }

  function assignUpdaterSettings(settings) {
    updaterDraft = copyUpdaterSettings(settings);
    initialUpdaterSettings = { ...updaterDraft };
    installDirectory.value = updaterDraft.installDirectory;
    startOnLogin.checked = updaterDraft.startOnLogin;
    applyAutoplay(updaterDraft.autoplayVideos);
    automaticUpdatesEnabled.checked = updaterDraft.automaticUpdatesEnabled;
    automaticChecksEnabled.checked = updaterDraft.automaticChecksEnabled;
    updaterSettingsLoaded = true;
    updateSaveButton();
  }

  function renderUpdaterState() {
    const state = updaterState || {};
    const updateAvailable = state.updateAvailable === true;
    const simulated = state.status === "simulated";
    const currentVersion = String(state.currentVersion || "");
    const latestVersion = String(state.latestVersion || "");
    el("blobui-version").textContent = currentVersion ? "BlobUI " + currentVersion : "BlobUI version unavailable";
    el("update-version").textContent = currentVersion
      ? "Current version " + currentVersion + (updateAvailable && latestVersion ? " · Latest version " + latestVersion : "")
      : "";
    const fallbackMessage = state.status === "checking" ? "Checking for updates…"
      : state.status === "updating" ? "Downloading the update. Settings will close and reopen automatically."
      : simulated ? "Preview update available. No files have been changed."
      : updateAvailable ? (latestVersion ? "Version " + latestVersion + " is available." : "An update is available.")
      : state.status === "current" ? "You're up to date."
      : state.status === "updated" ? "Update completed."
      : state.status === "error" ? "Could not check for updates."
      : "Check for updates to see if a newer version is available.";
    el("update-status").textContent = String(state.message || fallbackMessage);
    el("update-status").classList.toggle("error", state.status === "error");
    checkUpdatesButton.textContent = updaterBusyOperation === "checking" ? "Checking…"
      : updaterBusyOperation === "updating" ? "Updating…"
      : updateAvailable ? "Update now"
      : state.status === "current" || state.status === "updated" ? "You're up to date"
      : "Check for updates";
    checkUpdatesButton.disabled = !updaterSettingsLoaded || !window.appReady || !!updaterBusyOperation || saveBusy || blobFolderBusy || blobRestoreReloadFailed;
    updateNowButton.hidden = !updateAvailable;
    sidebarActions.classList.toggle("has-update", updateAvailable);
    updateNowButton.disabled = !!updaterBusyOperation || saveBusy || blobFolderBusy || blobRestoreReloadFailed || editSession.isDirty();
    updateNowButton.title = simulated ? "Preview update only" : "Update now";
    updateNowButton.setAttribute("aria-label", simulated ? "Preview update only" : "Update now");
    renderBlobBackupControls();
  }

  function isInstallingUpdate() {
    return updaterBusyOperation === "updating";
  }

  function renderBlobBackupControls() {
    const settingsDirty = !!draft && editSession.isDirty();
    const updaterDirty = !!updaterDraft && !!initialUpdaterSettings
      && !updaterSettingsEqual(updaterDraft, initialUpdaterSettings);
    const disabled = !window.appReady || !updaterSettingsLoaded || saveBusy || !!updaterBusyOperation
      || blobFolderBusy || blobRestoreReloadFailed || uninstallBusy || settingsDirty || updaterDirty;
    createBlobBackupButton.disabled = disabled;
    importBlobBackupButton.disabled = disabled;
    el("uninstall-button").disabled = !window.appReady || saveBusy || !!updaterBusyOperation || blobFolderBusy || blobRestoreReloadFailed || uninstallBusy;
  }

  function maybeApplyPendingAutomaticUpdate() {
    if (!pendingAutomaticUpdate || saveBusy || updaterBusyOperation || blobFolderBusy || blobRestoreReloadFailed) return;
    if (!initialUpdaterSettings || !initialUpdaterSettings.automaticUpdatesEnabled) {
      pendingAutomaticUpdate = false;
      return;
    }
    if (simulationActive || (updaterState && updaterState.status === "simulated")) {
      pendingAutomaticUpdate = false;
      return;
    }
    if (!updaterState || updaterState.updateAvailable !== true || updaterState.automaticUpdateBlocked === true || editSession.isDirty()) return;
    pendingAutomaticUpdate = false;
    applyUpdate(true);
  }

  function mergeUpdaterState(state) {
    if (state && typeof state === "object") updaterState = { ...(updaterState || {}), ...state };
    renderUpdaterState();
  }

  async function loadUpdaterState() {
    try {
      const state = await invoke("get_updater_state");
      assignUpdaterSettings(state || {});
      mergeUpdaterState(state || {});
    } catch (error) {
      updaterState = { status: "error", message: error.message || String(error) };
      if (!updaterSettingsLoaded) {
        assignUpdaterSettings({
          installDirectory: installDirectory.value || "",
          startOnLogin: false,
          automaticUpdatesEnabled: false,
          automaticChecksEnabled: false,
        });
      }
      renderUpdaterState();
    }
  }

  function runAutomaticUpdateCheck() {
    if (!window.appReady || !initialUpdaterSettings || !initialUpdaterSettings.automaticChecksEnabled
      || releaseNotesLoading || releaseNotesModal.isOpen() || releaseNotesFailed) return;
    checkForUpdates(true);
  }

  function updateReleaseNotesHeading(version) {
    const currentTag = el("release-notes-current-tag");
    const latestTag = el("release-notes-latest-tag");
    el("release-notes-version").textContent = version ? "Version " + version : "";
    currentTag.hidden = !version || version !== releaseNotesCurrentVersion;
    latestTag.hidden = !version || version !== releaseNotesLatestVersion;
  }

  function loadLatestReleaseNotesVersion() {
    const request = ++releaseNotesLatestVersionRequest;
    releaseNotesLatestVersion = null;
    updateReleaseNotesHeading(displayedReleaseNotes && displayedReleaseNotes.version);
    invoke("get_latest_release_notes").then((notes) => {
      if (request !== releaseNotesLatestVersionRequest || !notes || typeof notes.version !== "string") return;
      releaseNotesLatestVersion = notes.version;
      updateReleaseNotesHeading(displayedReleaseNotes && displayedReleaseNotes.version);
    }).catch(() => {});
  }

  async function renderReleaseNotes(notes, historical = false) {
    historical = historical || !notes.version.startsWith("Dev+");
    updateReleaseNotesHeading(notes.version);
    const mediaManifest = window.BlobUI.Markdown.parseMediaManifest
      ? window.BlobUI.Markdown.parseMediaManifest(notes.imageManifest)
      : { images: window.BlobUI.Markdown.parseImageManifest(notes.imageManifest), videos: {} };
    const manifest = mediaManifest.images;
    const entries = Object.entries(manifest);
    const results = await Promise.allSettled(entries.map(async ([alias, image]) => {
      const args = { source: image.src };
      const src = historical
        ? await invoke("get_release_note_history_image", { ...args, tag: notes.tag || notes.version })
        : await invoke("get_release_note_image", args);
      return [alias, { src, alt: image.alt }];
    }));
    const images = Object.create(null);
    for (const result of results) if (result.status === "fulfilled") images[result.value[0]] = result.value[1];
    const videos = Object.create(null);
    const videoResults = await Promise.allSettled(Object.entries(mediaManifest.videos).map(async ([alias, video]) => {
      videos[alias] = { alt: video.alt };
      const args = { source: video.src };
      videos[alias].src = historical
        ? await invoke("get_release_note_history_video", { ...args, tag: notes.tag || notes.version })
        : await invoke("get_release_note_video", args);
    }));
    if (window.BlobUI.Markdown.mount) window.BlobUI.Markdown.mount(releaseNotesContent, notes.markdown, { images, videos });
    else releaseNotesContent.innerHTML = window.BlobUI.Markdown.render(notes.markdown, { images });
    el("release-notes-image-warning").hidden = [...results, ...videoResults].every((result) => result.status === "fulfilled");
    el("release-notes-dialog").scrollTop = 0;
    updateReleaseNotesScrollbar();
  }

  function prefetchPreviousReleaseNotes() {
    const request = ++releaseNotesHistoryRequest;
    const version = displayedReleaseNotes && displayedReleaseNotes.version;
    previousReleaseNotes = null;
    releaseNotesPrevious.disabled = true;
    releaseNotesPrevious.title = "Checking for earlier release notes…";
    if (!version || (typeof navigator !== "undefined" && navigator.onLine === false)) {
      releaseNotesPrevious.title = "Release history is unavailable while offline";
      updateReleaseNotesNavigation();
      return;
    }
    invoke("get_adjacent_release_notes", { version, older: true }).then((notes) => {
      if (request !== releaseNotesHistoryRequest) return;
      previousReleaseNotes = notes || null;
      releaseNotesPrevious.title = previousReleaseNotes ? "Load earlier release notes" : "No earlier release notes are available";
      updateReleaseNotesNavigation();
    }).catch(() => {
      if (request !== releaseNotesHistoryRequest) return;
      previousReleaseNotes = null;
      releaseNotesPrevious.title = "Release history is unavailable while offline";
      updateReleaseNotesNavigation();
    });
  }

  function updateReleaseNotesNavigation() {
    const offline = typeof navigator !== "undefined" && navigator.onLine === false;
    releaseNotesPrevious.disabled = releaseNotesNavigationBusy || offline || !previousReleaseNotes;
    releaseNotesAfter.disabled = releaseNotesNavigationBusy || offline || newerReleaseNotes.length === 0;
    releaseNotesAfter.hidden = newerReleaseNotes.length === 0;
  }

  async function displayReleaseNotes(notes, historical = false) {
    await renderReleaseNotes(notes, historical);
    displayedReleaseNotes = notes;
    prefetchPreviousReleaseNotes();
    updateReleaseNotesNavigation();
  }

  releaseNotesPrevious.addEventListener("click", async () => {
    if (!previousReleaseNotes || releaseNotesPrevious.disabled || !displayedReleaseNotes || releaseNotesNavigationBusy) return;
    const older = previousReleaseNotes;
    const newer = displayedReleaseNotes;
    releaseNotesNavigationBusy = true;
    updateReleaseNotesNavigation();
    newerReleaseNotes.push(newer);
    try {
      await displayReleaseNotes(older, true);
    } catch (error) {
      newerReleaseNotes.pop();
      displayedReleaseNotes = newer;
      setStatus("Could not display the previous changelog. " + (error.message || String(error)), "error");
    } finally {
      releaseNotesNavigationBusy = false;
      updateReleaseNotesNavigation();
    }
  });

  releaseNotesAfter.addEventListener("click", async () => {
    if (!newerReleaseNotes.length || releaseNotesAfter.disabled || releaseNotesNavigationBusy) return;
    const newer = newerReleaseNotes.pop();
    const current = displayedReleaseNotes;
    releaseNotesNavigationBusy = true;
    updateReleaseNotesNavigation();
    try {
      await displayReleaseNotes(newer, newer.version !== releaseNotesVersion);
    } catch (error) {
      newerReleaseNotes.push(newer);
      displayedReleaseNotes = current;
      setStatus("Could not display the newer changelog. " + (error.message || String(error)), "error");
    } finally {
      releaseNotesNavigationBusy = false;
      updateReleaseNotesNavigation();
    }
  });

  window.addEventListener("offline", () => {
    releaseNotesHistoryRequest += 1;
    updateReleaseNotesNavigation();
  });
  window.addEventListener("online", () => {
    if (releaseNotesModal.isOpen()) prefetchPreviousReleaseNotes();
    updateReleaseNotesNavigation();
  });

  async function showReleaseNotesOrContinue() {
    if (!window.appReady || releaseNotesLoading || releaseNotesLoaded) return false;
    releaseNotesLoading = true;
    let continueWithAutomaticCheck = false;
    try {
      const notes = await invoke("get_release_notes");
      releaseNotesLoaded = true;
      if (!notes) {
        continueWithAutomaticCheck = true;
        return false;
      }
      if (typeof notes.version !== "string" || !notes.version.trim()
        || typeof notes.markdown !== "string" || typeof notes.imageManifest !== "string") {
        throw new TypeError("The release notes payload is incomplete.");
      }
      releaseNotesVersion = String(notes.version || "");
      releaseNotesCurrentVersion = releaseNotesVersion;
      releaseNotesLatestVersion = null;
      newerReleaseNotes = [];
      await displayReleaseNotes(notes);
      releaseNotesFailed = false;
      releaseNotesModal.open();
      if (!releaseNotesModal.isOpen()) throw new Error("The release notes dialog could not be opened.");
      loadLatestReleaseNotesVersion();
      return true;
    } catch (error) {
      releaseNotesVersion = null;
      releaseNotesCurrentVersion = null;
      releaseNotesLatestVersion = null;
      releaseNotesLoaded = false;
      releaseNotesFailed = true;
      setStatus("Could not display the release notes. " + (error.message || String(error)), "error");
      return false;
    } finally {
      releaseNotesLoading = false;
      if (continueWithAutomaticCheck) runAutomaticUpdateCheck();
    }
  }

  async function showLatestReleaseNotes() {
    try {
      const notes = await invoke("get_latest_release_notes");
      if (!notes) return showHelpError("The latest changelog is unavailable.");
      if (typeof notes.version !== "string" || typeof notes.markdown !== "string" || typeof notes.imageManifest !== "string") {
        throw new TypeError("The latest changelog is incomplete.");
      }
      releaseNotesVersion = notes.version;
      releaseNotesCurrentVersion = String((updaterState && updaterState.currentVersion) || "");
      releaseNotesLatestVersion = notes.version;
      newerReleaseNotes = [];
      await displayReleaseNotes(notes);
      acknowledgeReleaseNotesOnClose = false;
      releaseNotesModal.open();
    } catch (error) {
      setStatus("Could not display the latest changelog. " + (error.message || String(error)), "error");
      showHelpError("The latest changelog is unavailable.");
    }
  }

  async function checkForUpdates(automatic = false) {
    if (automatic && (simulationActive || releaseNotesLoading || releaseNotesModal.isOpen() || releaseNotesFailed)) return;
    if (!updaterSettingsLoaded || !window.appReady || updaterBusyOperation) return;
    updaterBusyOperation = "checking";
    mergeUpdaterState({ status: "checking", message: "Checking for updates…" });
    renderUpdaterState();
    try {
      const state = await invoke("check_for_updates");
      mergeUpdaterState(state || {});
      if (automatic && initialUpdaterSettings && initialUpdaterSettings.automaticUpdatesEnabled
        && state && state.updateAvailable === true && state.status !== "simulated") {
        if (state.automaticUpdateBlocked === true) {
          pendingAutomaticUpdate = false;
        } else if (saveBusy || bannerReadPending || editSession.isDirty()) {
          pendingAutomaticUpdate = true;
          mergeUpdaterState({ message: "An update is ready. Save or undo your pending settings to install it automatically." });
        } else {
          await applyUpdate(true);
        }
      }
    } catch (error) {
      mergeUpdaterState({ status: "error", message: error.message || String(error) });
      if (!automatic) setStatus("Could not check for updates. " + (error.message || String(error)), "error");
    } finally {
      updaterBusyOperation = "";
      renderUpdaterState();
      maybeApplyPendingAutomaticUpdate();
    }
  }

  async function applyUpdate(fromAutomaticCheck = false) {
    if (blobFolderBusy || blobRestoreReloadFailed) return;
    if (!updaterState || updaterState.updateAvailable !== true) return;
    if (updaterState.status === "simulated") {
      simulationActive = false;
      mergeUpdaterState({
        status: "updated",
        updateAvailable: false,
        latestVersion: updaterState.currentVersion || null,
        message: "Preview complete. No files were changed.",
      });
      return;
    }
    if (updaterBusyOperation && !(fromAutomaticCheck && updaterBusyOperation === "checking")) return;
    if (saveBusy || bannerReadPending || editSession.isDirty()) {
      if (fromAutomaticCheck) pendingAutomaticUpdate = true;
      else el("update-status").textContent = "Save or undo your pending settings before installing the update.";
      return;
    }
    updaterBusyOperation = "updating";
    mergeUpdaterState({ status: "updating", message: "Downloading the update. Settings will close and reopen automatically." });
    renderUpdaterState();
    updateSaveButton();
    try {
      mergeUpdaterState(await invoke("apply_update", { automatic: fromAutomaticCheck }) || {});
    } catch (error) {
      mergeUpdaterState({ status: "error", message: error.message || String(error), updateAvailable: true });
      setStatus("Could not install the update. " + (error.message || String(error)), "error");
    } finally {
      updaterBusyOperation = "";
      renderUpdaterState();
      updateSaveButton();
    }
  }

  async function simulateUpdate() {
    if (!window.appReady || updaterBusyOperation) return;
    try {
      simulationActive = true;
      const state = await invoke("simulate_update");
      mergeUpdaterState({ ...(state || {}), status: "simulated", updateAvailable: true });
    } catch (error) {
      simulationActive = false;
      setStatus("Could not preview an update. " + (error.message || String(error)), "error");
    }
  }

  function setStatus(message, kind, record = true) {
    statusMessage.textContent = message;
    if (!record) return;
    addLog(message, kind);
  }

  function addLog(message, kind) {
    const level = kind === "error" || kind === "warning" ? kind : "info";
    const entry = { message: String(message), level, time: new Date().toLocaleTimeString() };
    logs.push(entry);
    const row = document.createElement("p");
    row.className = "log-entry " + level;
    row.textContent = formatLog(entry);
    logEntries.appendChild(row);
    updateLogsScrollbar();
    logEntries.scrollTop = logEntries.scrollHeight;
    const severity = logs.some((item) => item.level === "error") ? "error"
      : logs.some((item) => item.level === "warning") ? "warning" : "info";
    logButton.dataset.severity = severity;
    logButton.hidden = logs.length === 0;
    el("log-label").textContent = severity === "error" ? "Errors and logs" : severity === "warning" ? "Warnings and logs" : "Logs";
  }

  function formatLog(entry) {
    return "[" + entry.time + "] " + entry.level.toUpperCase() + " " + entry.message;
  }

  function closeLogs(restoreFocus = false) {
    logPopup.hidden = true;
    logButton.setAttribute("aria-expanded", "false");
    if (restoreFocus) logButton.focus();
  }

  const shortcutActions = [
    ["openSettings", "Open Settings from Anytype"],
    ["rebuildCss", "Rebuild custom.css"],
    ["showOnboarding", "Show setup guide"],
    ["simulateUpdate", "Preview an update"],
    ["quitSettings", "Quit BlobUI Settings"],
  ];
  let shortcutBindings = [];
  let shortcutCaptureAction = null;
  function showHelpError(message = "This help content could not be found.") {
    el("help-error-message").textContent = message;
    helpErrorModal.open();
  }
  function formatShortcut(binding) {
    const modifiers = [];
    if (binding.ctrl) modifiers.push("Ctrl");
    if (binding.alt) modifiers.push("Alt");
    if (binding.shift) modifiers.push("Shift");
    if (binding.meta) modifiers.push("Win");
    const label = binding.code.startsWith("Key") ? binding.code.slice(3)
      : binding.code.startsWith("Digit") ? binding.code.slice(5)
      : binding.code.startsWith("Numpad") ? binding.code.replace("Numpad", "Numpad ")
      : binding.code;
    modifiers.push(label);
    return modifiers.join("+");
  }
  async function loadShortcutBindings() {
    try {
      const loaded = await invoke("get_keyboard_shortcuts");
      if (!Array.isArray(loaded)) throw new TypeError("Shortcut settings were invalid.");
      shortcutBindings = loaded;
      renderShortcutBindings();
    } catch (error) {
      el("keybind-status").textContent = error.message || String(error);
      el("keybind-status").classList.add("error");
    }
  }
  function renderShortcutBindings() {
    const list = el("keybind-list");
    window.BlobUI.Components.destroyTree(list);
    list.replaceChildren();
    shortcutActions.forEach(([action, label]) => {
      const binding = shortcutBindings.find((item) => item.action === action);
      const row = document.createElement("div");
      row.className = "keybind-row";
      const name = document.createElement("span");
      name.className = "keybind-label";
      name.id = `keybind-label-${action}`;
      name.textContent = label;
      const button = window.BlobUI.Components.Button({ onClick: () => beginShortcutCapture(action) }).element;
      button.className = "keybind-button";
      button.type = "button";
      button.dataset.shortcutAction = action;
      button.setAttribute("aria-labelledby", `${name.id} keybind-button-${action}`);
      button.id = `keybind-button-${action}`;
      button.setAttribute("aria-pressed", String(shortcutCaptureAction === action));
      if (shortcutCaptureAction === action || !binding) {
        button.textContent = shortcutCaptureAction === action ? "Press a shortcut…" : "Unavailable";
      } else {
        const shortcut = formatShortcut(binding);
        const symbols = { Shift: "⇧", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→" };
        button.setAttribute("aria-label", shortcut);
        shortcut.split("+").forEach((key) => {
          const keycap = document.createElement("kbd");
          keycap.className = "keybind-keycap";
          keycap.setAttribute("aria-hidden", "true");
          keycap.textContent = symbols[key] || key;
          button.append(keycap);
        });
      }
      row.append(name, button);
      list.append(row);
    });
  }
  async function beginShortcutCapture(action) {
    shortcutCaptureAction = action;
    el("keybind-status").classList.remove("error");
    el("keybind-status").textContent = "Press a key combination. Press Escape to cancel.";
    renderShortcutBindings();
    el("keybind-list").querySelector(`[data-shortcut-action="${action}"]`).focus();
    try { await invoke("set_shortcut_capture", { active: true }); }
    catch (error) {
      shortcutCaptureAction = null;
      renderShortcutBindings();
      el("keybind-status").textContent = error.message || String(error);
      el("keybind-status").classList.add("error");
    }
  }
  async function finishShortcutCapture(event) {
    if (!shortcutCaptureAction) return;
    if (["Control", "Alt", "Shift", "Meta", "OS"].includes(event.key)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (event.key === "Tab") {
      el("keybind-status").textContent = "Tab is reserved for moving through the dialog.";
      el("keybind-status").classList.add("error");
      el("keybind-list").querySelector(`[data-shortcut-action="${shortcutCaptureAction}"]`).focus();
      return;
    }
    const action = shortcutCaptureAction;
    shortcutCaptureAction = null;
    await invoke("set_shortcut_capture", { active: false }).catch(() => {});
    if (event.key === "Escape") {
      el("keybind-status").classList.remove("error");
      el("keybind-status").textContent = "Shortcut capture canceled.";
      renderShortcutBindings();
      el("keybind-list").querySelector(`[data-shortcut-action="${action}"]`).focus();
      return;
    }
    const next = {
      action,
      code: event.code,
      ctrl: event.ctrlKey,
      alt: event.altKey,
      shift: event.shiftKey,
      meta: event.metaKey,
    };
    const conflict = shortcutBindings.find((item) => item.action !== action && item.code === next.code
      && item.ctrl === next.ctrl && item.alt === next.alt && item.shift === next.shift && item.meta === next.meta);
    if (!next.code || conflict) {
      el("keybind-status").textContent = conflict ? "That shortcut is already assigned to another BlobUI action." : "That key cannot be used as a shortcut.";
      el("keybind-status").classList.add("error");
      renderShortcutBindings();
      el("keybind-list").querySelector(`[data-shortcut-action="${action}"]`).focus();
      return;
    }
    const updated = shortcutBindings.map((item) => item.action === action ? next : item);
    try {
      shortcutBindings = await invoke("save_keyboard_shortcuts", { shortcuts: updated });
      el("keybind-status").textContent = "Shortcut saved.";
      el("keybind-status").classList.remove("error");
    } catch (error) {
      el("keybind-status").textContent = error.message || String(error);
      el("keybind-status").classList.add("error");
    }
    renderShortcutBindings();
    el("keybind-list").querySelector(`[data-shortcut-action="${action}"]`).focus();
  }
  document.addEventListener("keydown", finishShortcutCapture, true);

  function connectionHasChanges() {
    const keyChanged = hasSavedApiKey ? advancedKeyEdited : advancedApiKey.value.trim() !== "";
    return advancedApiIp.value.trim() !== initialApiDraft.ip
      || advancedApiPort.value.trim() !== initialApiDraft.port
      || keyChanged;
  }

  function readEditSnapshot() {
    return {
      formulas: window.BlobUI.FormulaPage.read(),
      settings: draft ? { ...draft } : null,
      updaterSettings: updaterDraft ? { ...updaterDraft } : null,
      connection: {
        inputIp: advancedApiIp.value.trim(),
        inputPort: advancedApiPort.value.trim(),
        apiIp: currentApiIp,
        apiPort: currentApiPort,
        hasSavedApiKey,
        keyChanged: hasSavedApiKey ? advancedKeyEdited : advancedApiKey.value.trim() !== "",
      },
    };
  }

  function readConnectionBaseline() {
    return {
      inputIp: initialApiDraft.ip,
      inputPort: initialApiDraft.port,
      apiIp: currentApiIp,
      apiPort: currentApiPort,
      hasSavedApiKey,
      keyChanged: false,
    };
  }

  function editSnapshotsEqual(left, right) {
    if (!left || !right || !settingsEqual(left.settings, right.settings)) return false;
    if (JSON.stringify(left.formulas) !== JSON.stringify(right.formulas)) return false;
    if (left.updaterSettings || right.updaterSettings) {
      if (!updaterSettingsEqual(left.updaterSettings, right.updaterSettings)) return false;
    }
    const a = left.connection;
    const b = right.connection;
    return !!a && !!b
      && a.inputIp === b.inputIp
      && a.inputPort === b.inputPort
      && a.apiIp === b.apiIp
      && a.apiPort === b.apiPort
      && a.hasSavedApiKey === b.hasSavedApiKey
      && a.keyChanged === b.keyChanged;
  }

  function clearValidationErrors() {
    [fontFamily, codeFontFamily, accentValue, advancedApiKey, advancedApiIp, advancedApiPort]
      .forEach((input) => input.classList.remove("invalid"));
    el("font-error").textContent = "";
    el("accent-error").textContent = "";
    el("advanced-api-error").textContent = "";
  }

  function restoreEditSnapshot(snapshot) {
    if (!snapshot || !snapshot.settings || !snapshot.connection) return;
    window.BlobUI.FormulaPage.restore(snapshot.formulas);

    invalidateBannerRead();
    assignDraftToForm(snapshot.settings);
    if (snapshot.updaterSettings) {
      updaterDraft = { ...snapshot.updaterSettings };
      installDirectory.value = updaterDraft.installDirectory;
      startOnLogin.checked = updaterDraft.startOnLogin;
      applyAutoplay(updaterDraft.autoplayVideos);
      automaticUpdatesEnabled.checked = updaterDraft.automaticUpdatesEnabled;
      automaticChecksEnabled.checked = updaterDraft.automaticChecksEnabled;
    }
    lastCustomAccent = snapshot.settings.accentMode === "native" ? null : {
      accentMode: snapshot.settings.accentMode,
      accentPreset: snapshot.settings.accentPreset,
      accentColor: snapshot.settings.accentColor,
    };

    const connection = snapshot.connection;
    currentApiIp = String(connection.apiIp || "");
    currentApiPort = Number(connection.apiPort) || 31009;
    hasSavedApiKey = connection.hasSavedApiKey === true;
    advancedKeyEdited = false;
    advancedApiKey.value = hasSavedApiKey ? SAVED_KEY_MASK : "";
    advancedApiKey.placeholder = hasSavedApiKey ? "" : "Paste a read-only API v2 key";
    advancedApiIp.value = connection.inputIp;
    advancedApiPort.value = connection.inputPort;
    initialApiDraft = { ip: connection.inputIp, port: connection.inputPort };
    el("advanced-api-key-hint").textContent = hasSavedApiKey
      ? "A key is saved. Leave the masked value unchanged to keep it, or clear it to remove it."
      : "Enter a new read-only Anytype API v2 key.";
    ["advanced-api-key", "advanced-api-ip", "advanced-api-port"].forEach(resetVisibilityField);
    clearValidationErrors();
    updateSaveButton();
    if (window.appReady) setStatus("No unsaved changes.", "info", false);
  }

  function assignAdvancedConnection(state) {
    currentApiIp = String(state.apiIp || "").trim();
    currentApiPort = Number(state.apiPort) || 31009;
    hasSavedApiKey = state.hasSavedApiKey === true;
    advancedKeyEdited = false;
    advancedApiKey.value = hasSavedApiKey ? SAVED_KEY_MASK : "";
    advancedApiKey.placeholder = hasSavedApiKey ? "" : "Paste a read-only API v2 key";
    advancedApiIp.value = currentApiIp === "127.0.0.1" ? "" : currentApiIp;
    advancedApiPort.value = currentApiPort === 31009 ? "" : String(currentApiPort);
    resetAdvancedVisibility();
    initialApiDraft = { ip: advancedApiIp.value.trim(), port: advancedApiPort.value.trim() };
    el("advanced-api-key-hint").textContent = hasSavedApiKey
      ? "A key is saved. Leave the masked value unchanged to keep it, or clear it to remove it."
      : "Enter a new read-only Anytype API v2 key.";
    updateSaveButton();
  }

  function openKeyRemovalConfirmation() {
    apiKeyRemovalModal.open();
  }

  function updateSaveButton() {
    const dirty = editSession.isDirty();
    const prototypeDirty = JSON.stringify(window.BlobUI.FormulaPage.read()) !== JSON.stringify(editSession.getBaseline().formulas);
    const settingsLocked = saveBusy || isInstallingUpdate() || blobFolderBusy || blobRestoreReloadFailed;
    if (settingsLocked || !draft) closeKanbanStyleMenu();
    saveButton.hidden = !draft || !dirty;
    saveButton.disabled = settingsLocked || bannerReadPending || !draft || (!window.appReady && !prototypeDirty) || !dirty;
    discardButton.hidden = !draft || !dirty;
    discardButton.disabled = settingsLocked || bannerReadPending || !draft || !dirty;
    document.querySelectorAll("input").forEach((control) => {
      control.disabled = settingsLocked || !draft || (!!draft && draft.accentMode === "native" && [hueInput, saturationInput, brightnessInput, accentValue].includes(control));
    });
    window.BlobUI.FormulaPage.setLocked(settingsLocked || !draft, dirty);
    el("settings-v21-enabled").disabled = settingsLocked || !draft;
    calendarColorsEnabled.disabled = settingsLocked || !draft || !window.appReady || calendarApiAvailability !== "available";
    kanbanViewStyle.disabled = settingsLocked || !draft;
    if ((settingsLocked || !draft) && document.activeElement === kanbanViewStyle) el("page-title").focus();
    bannerImageInput.disabled = settingsLocked || bannerReadPending || !draft || !window.appReady;
    chooseBannerImage.disabled = settingsLocked || bannerReadPending || !draft || !window.appReady;
    resetBannerImage.disabled = settingsLocked || bannerReadPending || !draft || !window.appReady;
    [installDirectory, startOnLogin, autoplayVideos, automaticUpdatesEnabled, automaticChecksEnabled].forEach((control) => {
      control.disabled = settingsLocked || !draft || !updaterSettingsLoaded || !window.appReady;
    });
    installDirectory.disabled = settingsLocked || !updaterSettingsLoaded;
    Object.keys(visibilityFields).forEach((inputId) => {
      visibilityFields[inputId].button.disabled = settingsLocked || !draft;
    });
    ["discard-confirmation-close", "discard-confirmation-undo", "api-key-removal-close", "api-key-removal-undo", "api-key-removal-save"]
      .forEach((id) => { el(id).disabled = settingsLocked; });
    ["onboarding-welcome", "onboarding-no-api-option", "onboarding-api-option", "connection-key-visibility", "connection-port-visibility", "connection-back", "connection-confirm"]
      .forEach((id) => { el(id).disabled = isInstallingUpdate() || connectionBusy; });
    saveButton.classList.toggle("busy", saveBusy);
    saveLabel.textContent = saveBusy ? "Saving…" : "Save changes";
    apiKeyRemovalSave.disabled = settingsLocked;
    apiKeyRemovalUndo.disabled = settingsLocked;
    apiKeyRemovalClose.disabled = settingsLocked;
    renderUpdaterState();
    renderBlobBackupControls();
    maybeApplyPendingAutomaticUpdate();
  }

  function applyFontFamilies() {
    const font = fontFamily.value.trim();
    const codeFont = codeFontFamily.value.trim();
    const validFamily = /^[\p{L}\p{N}][\p{L}\p{N} _-]{0,79}$/u;
    const interfaceFamily = validFamily.test(font) ? font : "Inter";
    const codeFamily = validFamily.test(codeFont) ? codeFont : "Consolas";
    root.style.setProperty("--font-interface", '"' + interfaceFamily + '", "Segoe UI", sans-serif');
    root.style.setProperty("--font-code", '"' + codeFamily + '", "Cascadia Code", monospace');
  }

  function assignDraftToForm(settings) {
    const accent = normalizeForUi(settings);
    draft = {
      fontFamily: String(settings.fontFamily || "Inter"),
      codeFontFamily: String(settings.codeFontFamily || "Consolas"),
      accentColor: accent.accentColor,
      accentMode: accent.accentMode,
      accentPreset: accent.accentPreset,
      includeSelections: settings.includeSelections !== false,
      bannerImage: String(settings.bannerImage || ""),
      reducedMotion: settings.reducedMotion === true,
      tabHoverEffect: settings.tabHoverEffect === true,
      linkHoverEffect: settings.linkHoverEffect === true,
      objectMentionHoverEffect: settings.objectMentionHoverEffect === true,
      kanbanViewStyle: ["default", "balanced", "compact"].includes(settings.kanbanViewStyle) ? settings.kanbanViewStyle : "default",
      collapsedCategories: settings.collapsedCategories === true,
      calendarColorsEnabled: settings.calendarColorsEnabled !== false,
      settingsV21Enabled: settings.settingsV21Enabled === true,
    };
    fontFamily.value = draft.fontFamily;
    codeFontFamily.value = draft.codeFontFamily;
    accentValue.value = draft.accentColor;
    reducedMotion.checked = draft.reducedMotion;
    root.dataset.reducedMotion = String(draft.reducedMotion);
    includeSelections.checked = draft.includeSelections;
    root.dataset.includeSelections = String(draft.includeSelections);
    tabHoverEffect.checked = draft.tabHoverEffect;
    linkHoverEffect.checked = draft.linkHoverEffect;
    objectMentionHoverEffect.checked = draft.objectMentionHoverEffect;
    closeKanbanStyleMenu();
    syncKanbanStyleSelection(draft.kanbanViewStyle);
    collapsedCategories.checked = draft.collapsedCategories;
    calendarColorsEnabled.checked = draft.calendarColorsEnabled;
    el("settings-v21-enabled").checked = draft.settingsV21Enabled;
    window.BlobUI.SettingsV21?.setEnabled(draft.settingsV21Enabled);
    renderPicker(true);
    applyFontFamilies();
    syncBannerPreview();
    initialDraft = { ...draft };
    updateSaveButton();
  }

  async function refreshCalendarApiAvailability() {
    const request = ++calendarApiRequest;
    calendarApiAvailability = "checking";
    const help = el("calendar-colors-help");
    help.textContent = "Checking the Anytype API connection...";
    updateSaveButton();
    try {
      const status = await invoke("get_calendar_api_availability");
      if (request !== calendarApiRequest) return;
      calendarApiAvailability = ["available", "missing"].includes(status) ? status : "unavailable";
    } catch (_) {
      if (request !== calendarApiRequest) return;
      calendarApiAvailability = "unavailable";
    }
    help.textContent = calendarApiAvailability === "available"
      ? "Press Alt+R to rebuild, then Ctrl+R in Anytype to apply calendar color changes."
      : calendarApiAvailability === "missing"
        ? "Add an Anytype API key in Advanced Settings to change Calendar Colors."
        : "The Anytype API is unavailable. Check the connection in Advanced Settings. Your saved choice is preserved.";
    updateSaveButton();
  }

  async function loadState() {
    let state = null;
    let stateError = null;
    try {
      state = await invoke("get_app_state");
      window.appReady = !!state.ready;
      applyTheme(state.theme);
      defaultBannerPreview = String(state.defaultBannerPreview || "");
      assignAdvancedConnection(state);
      void refreshCalendarApiAvailability();
      assignDraftToForm(state.settings || {});
      if (state.apiPort) el("connection-port").value = state.apiPort === 31009 ? "" : String(state.apiPort);
      resetVisibilityField("connection-port");
    } catch (error) {
      window.appReady = false;
      stateError = error;
    }

    await loadUpdaterState();
    await window.BlobUI.FormulaPage.load(invoke);
    if (!stateError) editSession.commit();
    if (stateError) {
      setStatus(stateError.message || String(stateError), "error");
      updateSaveButton();
      return;
    }

    await loadShortcutBindings();
    invoke("get_anytype_version").then((version) => {
      el("anytype-version").textContent = typeof version === "string" && version.trim() ? version : "Anytype unknown";
    }).catch(() => { el("anytype-version").textContent = "Anytype unknown"; });
    updateSaveButton();
    const needsOnboarding = state.ready && state.onboardingComplete === false;
    if (needsOnboarding) showOnboarding();
    if (!state.ready) {
      setStatus(state.message || "Run scripts/Install.ps1 once to set up BlobUI before saving.", "error");
    } else if (state.hotkeyError) {
      setStatus(state.hotkeyError, "error");
    } else {
      setStatus(state.message || "Settings loaded from this device.");
    }
    if (state.ready && !needsOnboarding) await showReleaseNotesOrContinue();
  }

  async function reloadTargetAppState({ reloadFormulas = false } = {}) {
    let state;
    try {
      state = await invoke("get_app_state");
    } catch (error) {
      window.appReady = false;
      updateSaveButton();
      throw error;
    }
    window.appReady = !!state.ready;
    applyTheme(state.theme);
    defaultBannerPreview = String(state.defaultBannerPreview || "");
    assignAdvancedConnection(state);
    void refreshCalendarApiAvailability();
    assignDraftToForm(state.settings || {});
    if (state.apiPort) el("connection-port").value = state.apiPort === 31009 ? "" : String(state.apiPort);
    resetVisibilityField("connection-port");
    await loadUpdaterState();
    if (reloadFormulas) {
      const loaded = await window.BlobUI.FormulaPage.load(invoke, { discover: false });
      if (!loaded) {
        blobRestoreReloadFailed = true;
        updateSaveButton();
        throw new Error("The imported calculations could not be reloaded. Settings are locked to protect the restored folder.");
      }
    }
    blobRestoreReloadFailed = false;
    clearValidationErrors();
    editSession.commit();
    updateSaveButton();
  }

  function applyTheme(theme) {
    const next = theme === "light" || theme === "dark" ? theme : (window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark");
    root.dataset.theme = next;
    if (next !== lastTheme) {
      lastTheme = next;
      if (draft) renderPicker(true);
    }
  }

  function syncBannerPreview() {
    const preview = draft && draft.bannerImage ? draft.bannerImage : defaultBannerPreview;
    if (preview) spacePreviewImage.src = preview;
    else spacePreviewImage.removeAttribute("src");
    chooseBannerImage.textContent = draft && draft.bannerImage ? "Choose a different image" : "Choose image";
    resetBannerImage.hidden = !(draft && draft.bannerImage);
  }

  function setBannerError(message) {
    el("banner-error").textContent = message;
    el("banner-image-status").textContent = "";
  }

  function invalidateBannerRead() {
    bannerReadGeneration += 1;
    bannerReadPending = false;
    updateSaveButton();
  }

  function handleBannerFileChange() {
    if (isInstallingUpdate()) return;
    bannerReadGeneration += 1;
    const generation = bannerReadGeneration;
    bannerReadPending = false;
    updateSaveButton();
    const file = bannerImageInput.files && bannerImageInput.files[0];
    bannerImageInput.value = "";
    if (!file) return;
    const allowedTypes = new Set(["image/png", "image/jpeg", "image/webp"]);
    if (!allowedTypes.has(String(file.type || "").toLowerCase())) {
      setBannerError("Choose a PNG, JPEG, or WebP image.");
      return;
    }
    if (!Number.isFinite(file.size) || file.size < 1 || file.size > 2 * 1024 * 1024) {
      setBannerError("Banner images must be 2 MB or smaller.");
      return;
    }
    bannerReadPending = true;
    updateSaveButton();
    const fail = (message) => {
      if (generation !== bannerReadGeneration) return;
      bannerReadPending = false;
      setBannerError(message);
      updateSaveButton();
    };
    const reader = new FileReader();
    reader.onerror = () => fail("Could not read the selected image. Try another file.");
    reader.onload = async () => {
      if (generation !== bannerReadGeneration) return;
      const dataUrl = typeof reader.result === "string" ? reader.result : "";
      const prefix = "data:" + String(file.type).toLowerCase() + ";base64,";
      const payload = dataUrl.startsWith(prefix) ? dataUrl.slice(prefix.length) : "";
      const maxEncodedLength = 4 * Math.ceil((2 * 1024 * 1024) / 3);
      if (!payload || payload.length > maxEncodedLength || !/^[A-Za-z0-9+/]*={0,2}$/.test(payload)) {
        fail("Could not read the selected image. Try another file.");
        return;
      }
      const format = { "image/png": "png", "image/jpeg": "jpeg", "image/webp": "webp" }[String(file.type).toLowerCase()];
      let rawBytes;
      try { rawBytes = atob(payload); }
      catch { fail("Could not read the selected image. Try another file."); return; }
      const matchesFormat = format === "png"
        ? rawBytes.length >= 8 && [137, 80, 78, 71, 13, 10, 26, 10].every((byte, index) => rawBytes.charCodeAt(index) === byte)
        : format === "jpeg"
          ? rawBytes.length >= 3 && rawBytes.charCodeAt(0) === 255 && rawBytes.charCodeAt(1) === 216 && rawBytes.charCodeAt(2) === 255
          : rawBytes.length >= 12 && rawBytes.slice(0, 4) === "RIFF" && rawBytes.slice(8, 12) === "WEBP";
      if (!matchesFormat) {
        fail("The selected file does not match its PNG, JPEG, or WebP image type.");
        return;
      }
      let decoded;
      try { decoded = await createImageBitmap(file); }
      catch { fail("The selected file could not be decoded as an image."); return; }
      const validDimensions = decoded.width > 0 && decoded.height > 0
        && decoded.width <= 8192 && decoded.height <= 8192
        && decoded.width * decoded.height <= 20_000_000;
      if (typeof decoded.close === "function") decoded.close();
      if (generation !== bannerReadGeneration) return;
      if (!validDimensions) {
        fail("Banner images must be no larger than 8192 × 8192 pixels or 20 megapixels.");
        return;
      }
      bannerReadPending = false;
      draft.bannerImage = dataUrl;
      el("banner-error").textContent = "";
      el("banner-image-status").textContent = "Custom banner selected.";
      syncBannerPreview();
      updateSaveButton();
      markDirty();
    };
    reader.readAsDataURL(file);
  }

  function showSettingsPage(page) {
    closeKanbanStyleMenu();
    const isFormulas = page === "formulas";
    const isAccessibility = page === "accessibility";
    const isAdvanced = page === "advanced";
    if (!isAdvanced && hasSavedApiKey && !advancedKeyEdited) {
      advancedApiKey.value = SAVED_KEY_MASK;
      resetVisibilityField("advanced-api-key");
    }
    if (!isAccessibility && !isAdvanced && !isFormulas && draft) void refreshCalendarApiAvailability();
    preferencesPage.hidden = isAccessibility || isAdvanced || isFormulas;
    el("formulas-page").hidden = !isFormulas;
    el("formulas-nav").classList.toggle("active", isFormulas);
    el("formulas-nav").setAttribute("aria-current", isFormulas ? "page" : "false");
    accessibilityPage.hidden = !isAccessibility;
    advancedPage.hidden = !isAdvanced;
    if (isAdvanced) advancedTabController.refresh();
    else if (!isAccessibility && !isFormulas) window.BlobUI.SettingsV21?.refreshTabs();
    pageTitle.textContent = isFormulas ? "Kanban View Formulas" : isAccessibility ? "Accessibility" : isAdvanced ? "Advanced Settings" : "Preferences";
    preferencesNav.classList.toggle("active", !isAccessibility && !isAdvanced && !isFormulas);
    accessibilityNav.classList.toggle("active", isAccessibility);
    advancedNav.classList.toggle("active", isAdvanced);
    preferencesNav.setAttribute("aria-current", !isAccessibility && !isAdvanced && !isFormulas ? "page" : "false");
    accessibilityNav.setAttribute("aria-current", isAccessibility ? "page" : "false");
    advancedNav.setAttribute("aria-current", isAdvanced ? "page" : "false");
    pageContent.scrollTop = 0;
    updatePageScrollbar();
    if (isFormulas) window.requestAnimationFrame(() => window.BlobUI.FormulaPage.refresh());
  }

  preferencesNav.addEventListener("click", () => showSettingsPage("preferences"));
  accessibilityNav.addEventListener("click", () => showSettingsPage("accessibility"));
  advancedNav.addEventListener("click", () => showSettingsPage("advanced"));
  showSettingsPage("preferences");

  async function refreshTheme() {
    try {
      const state = await invoke("get_app_state");
      applyTheme(state.theme);
    } catch (_) { /* Keep the last known mode while Anytype is not readable. */ }
  }

  function isValidIpv4(value) {
    const parts = value.split(".");
    return parts.length === 4 && parts.every((part) => /^\d{1,3}$/.test(part) && Number(part) <= 255);
  }

  function isValidIpAddress(value) {
    const address = value.trim();
    if (!address) return true;
    if (!address.includes(":")) return isValidIpv4(address);
    if (address.includes(":::") || address.split("::").length > 2) return false;
    let groups = address;
    let ipv4Tail = false;
    if (address.includes(".")) {
      const lastColon = address.lastIndexOf(":");
      if (lastColon < 0 || !isValidIpv4(address.slice(lastColon + 1))) return false;
      groups = address.slice(0, lastColon) + ":v4";
      ipv4Tail = true;
    }
    const compressed = groups.includes("::");
    const halves = compressed ? groups.split("::") : [groups];
    if (halves.length > 2) return false;
    const parts = halves.flatMap((half) => half ? half.split(":") : []);
    const validGroups = parts.every((part) => part === "v4" || /^[\da-f]{1,4}$/i.test(part));
    if (!validGroups) return false;
    const groupCount = parts.filter((part) => part !== "v4").length + (ipv4Tail ? 2 : 0);
    return compressed ? groupCount < 8 : groupCount === 8;
  }

  function saveStatus(result, connectionOnly) {
    if (result.calendarStatus === "off") {
      setStatus("Saved. Press Ctrl+R in Anytype to apply. Calendar Colors are off.");
      return;
    }
    if (result.calendarStatus === "disabled") {
      setStatus(connectionOnly
        ? "Connection saved. Press Ctrl+R in Anytype to apply. API features are disabled."
        : "Saved. Press Ctrl+R in Anytype to apply. API features are disabled.");
    } else if (result.calendarStatus === "cached") {
      setStatus(connectionOnly
        ? "Connection saved. Calendar data kept its last saved values; try saving again later."
        : "Saved. Press Ctrl+R in Anytype to apply; calendar colors kept their last saved values.", "info");
    } else {
      setStatus(connectionOnly
        ? "Connection saved. Press Ctrl+R in Anytype to apply. Calendar colors refreshed."
        : "Saved. Press Ctrl+R in Anytype to apply. Calendar colors refreshed.");
    }
  }

  function acceptAdvancedConnectionDraft(connection) {
    hasSavedApiKey = connection.enabled;
    advancedKeyEdited = false;
    advancedApiKey.value = hasSavedApiKey ? SAVED_KEY_MASK : "";
    advancedApiKey.placeholder = hasSavedApiKey ? "" : "Paste a read-only API v2 key";
    currentApiIp = advancedApiIp.value.trim();
    currentApiPort = advancedApiPort.value.trim() ? Number(advancedApiPort.value.trim()) : 31009;
    initialApiDraft = { ip: advancedApiIp.value.trim(), port: advancedApiPort.value.trim() };
    el("advanced-api-key-hint").textContent = hasSavedApiKey
      ? "A key is saved. Leave the masked value unchanged to keep it, or clear it to remove it."
      : "Enter a new read-only Anytype API v2 key.";
    resetAdvancedVisibility();
  }

  async function save(options = {}) {
    if (!draft || saveBusy || isInstallingUpdate() || blobFolderBusy || blobRestoreReloadFailed) return;
    // Let the formula page report missing connection/setup requirements inline.
    if (!window.appReady) {
      const formulas = window.BlobUI.FormulaPage.read();
      if (JSON.stringify(formulas) === JSON.stringify(editSession.getBaseline().formulas)) return;
      saveBusy = true;
      updateSaveButton();
      try {
        await window.BlobUI.FormulaPage.save(invoke, formulas);
        editSession.commit({ formulas });
        setStatus("Formulas saved. Press Ctrl+R in Anytype to apply.", "info", false);
      } catch (_) {
        setStatus("Kanban formulas could not be saved. Your edits are kept. Review the formula page.", "error", false);
      } finally {
        saveBusy = false;
        updateSaveButton();
      }
      return;
    }
    if (bannerReadPending) {
      setStatus("Wait for the selected banner image to finish loading before saving.", "warning");
      return;
    }
    draft.fontFamily = fontFamily.value.trim();
    draft.codeFontFamily = codeFontFamily.value.trim();
    draft.accentColor = accentValue.value.trim();
    draft.accentPreset = draft.accentMode === "preset" ? draft.accentPreset : "";
    if (updaterDraft) {
      updaterDraft.installDirectory = installDirectory.value;
      updaterDraft.startOnLogin = startOnLogin.checked;
      updaterDraft.autoplayVideos = autoplayVideos.checked;
      updaterDraft.automaticUpdatesEnabled = automaticUpdatesEnabled.checked;
      updaterDraft.automaticChecksEnabled = automaticChecksEnabled.checked;
    }

    const familyPattern = /^[\p{L}\p{N}][\p{L}\p{N} _-]{0,79}$/u;
    let invalid = false;
    for (const [input, message, valid] of [
      [fontFamily, el("font-error"), familyPattern.test(draft.fontFamily)],
      [codeFontFamily, el("font-error"), familyPattern.test(draft.codeFontFamily)],
    ]) {
      input.classList.toggle("invalid", !valid);
      if (!valid) { message.textContent = "Use a font name with letters, numbers, spaces, underscores, or hyphens."; invalid = true; }
    }
    const safeAccentSyntax = draft.accentColor.length > 0 && draft.accentColor.length <= 128 && !/[\u0000-\u001f;{}<>"']/u.test(draft.accentColor);
    const preset = legacyAccents.find((item) => item.key === draft.accentPreset);
    const supportedAccent = draft.accentMode === "native"
      ? draft.accentPreset === "" && ["Default", "var(--color-system-accent-100)"].includes(draft.accentColor)
      : draft.accentMode === "preset"
        ? !!preset && draft.accentColor === preset.value
      : draft.accentMode === "custom" && draft.accentPreset === "" && isCoreSupportedCustomAccent(draft.accentColor);
    accentValue.classList.toggle("invalid", !safeAccentSyntax || !supportedAccent);
    el("accent-error").textContent = !safeAccentSyntax || !supportedAccent ? "Use a 3 or 6 digit hex color, a supported rgb()/rgba() value, or an Anytype color variable. Pure white is reserved." : "";
    invalid = invalid || !safeAccentSyntax || !supportedAccent;

    const settingsChanged = !settingsEqual(draft, initialDraft);
    const submittedFormulas = window.BlobUI.FormulaPage.read();
    const formulasChanged = JSON.stringify(submittedFormulas) !== JSON.stringify(editSession.getBaseline().formulas);
    const updaterSettingsChanged = !!updaterDraft && !updaterSettingsEqual(updaterDraft, initialUpdaterSettings);
    if (updaterSettingsChanged) {
      const directory = installDirectory.value.trim();
      const validDirectory = directory.length > 0 && directory.length <= 1024 && !/[\u0000-\u001f\u007f]/.test(directory);
      installDirectory.classList.toggle("invalid", !validDirectory);
      el("updater-settings-error").textContent = validDirectory ? "" : "Enter a valid Anytype data folder.";
      if (!validDirectory) invalid = true;
      else updaterDraft.installDirectory = directory;
    } else {
      installDirectory.classList.remove("invalid");
      el("updater-settings-error").textContent = "";
    }
    const connectionChanged = connectionHasChanges();
    const updaterPathChanged = updaterSettingsChanged
      && installDirectory.value.trim() !== String(initialUpdaterSettings.installDirectory || "").trim();
    const apiIp = advancedApiIp.value.trim();
    const apiPort = advancedApiPort.value.trim();
    const apiKey = advancedApiKey.value.trim();
    const keepExistingKey = hasSavedApiKey && !advancedKeyEdited;
    advancedApiIp.classList.toggle("invalid", !isValidIpAddress(apiIp));
    if (apiIp && !isValidIpAddress(apiIp)) {
      el("advanced-api-error").textContent = "Enter a valid IPv4 or IPv6 address, or leave the field blank to use this PC.";
      invalid = true;
    }
    if (apiPort && (!/^\d+$/.test(apiPort) || Number(apiPort) < 1 || Number(apiPort) > 65535)) {
      advancedApiPort.classList.add("invalid");
      el("advanced-api-error").textContent = "Port must be a whole number from 1 to 65535.";
      invalid = true;
    } else {
      advancedApiPort.classList.remove("invalid");
    }
    if (!keepExistingKey && apiKey && (apiKey.length > 4096 || /[\x00-\x1f\x7f]/.test(apiKey))) {
      advancedApiKey.classList.add("invalid");
      el("advanced-api-error").textContent = "Enter a valid Anytype API v2 key.";
      invalid = true;
    } else {
      advancedApiKey.classList.remove("invalid");
    }
    if (invalid) {
      setStatus("Review the highlighted fields before saving.", "error");
      return;
    }

    if (updaterPathChanged && (settingsChanged || connectionChanged || formulasChanged)) {
      setStatus("Save or undo your appearance and Anytype API changes before changing the Anytype data folder.", "warning", false);
      return;
    }

    const advancedSaveWithEmptyKey = connectionChanged || (!advancedPage.hidden && el("advanced-experiments-panel").hidden && settingsChanged);
    const apiKeyWillBeEmpty = !keepExistingKey && !apiKey;
    if (advancedSaveWithEmptyKey && apiKeyWillBeEmpty && !options.confirmKeyRemoval) {
      openKeyRemovalConfirmation();
      return;
    }

    if (!settingsChanged && !updaterSettingsChanged && !connectionChanged && !formulasChanged && !options.forceRebuild) return;

    const submittedSettings = { ...draft };
    const submittedUpdaterSettings = updaterSettingsChanged ? { ...updaterDraft } : null;
    const submittedConnection = connectionChanged ? {
      enabled: keepExistingKey || apiKey.length > 0,
      apiKey: keepExistingKey ? "" : apiKey,
      keepExistingKey,
      ip: apiIp || null,
      port: apiPort ? Number(apiPort) : null,
    } : null;
    saveBusy = true;
    updateSaveButton();
    setStatus(updaterSettingsChanged && !settingsChanged && !connectionChanged
      ? "Saving App Settings…"
      : connectionChanged && !settingsChanged && !updaterSettingsChanged
      ? "Saving your Anytype connection and rebuilding custom.css…"
      : "Saving your settings and rebuilding custom.css…", "working");
    let connectionSaved = false;
    let generalSettingsSaved = false;
    let formulasSaved = false;
    let updaterSettingsSaved = false;
    try {
      let result = null;
      if (submittedConnection) {
        result = await invoke("configure_connection", { connection: submittedConnection });
        connectionSaved = true;
        acceptAdvancedConnectionDraft(submittedConnection);
        editSession.commit({ connection: readConnectionBaseline() });
        window.BlobUI.FormulaPage.reload();
      }
      if (formulasChanged) {
        await window.BlobUI.FormulaPage.save(invoke, submittedFormulas);
        editSession.commit({ formulas: submittedFormulas });
        formulasSaved = true;
      }
      if (settingsChanged || options.forceRebuild) {
        result = await invoke("save_settings", { settings: submittedSettings });
        generalSettingsSaved = settingsChanged;
        if (settingsChanged) {
          initialDraft = submittedSettings;
          editSession.commit({ settings: submittedSettings });
        }
      }
      if (submittedUpdaterSettings) {
        const savedUpdaterState = await invoke("save_updater_settings", { settings: submittedUpdaterSettings });
        updaterSettingsSaved = true;
        updaterDraft = { ...submittedUpdaterSettings };
        initialUpdaterSettings = { ...submittedUpdaterSettings };
        installDirectory.value = updaterDraft.installDirectory;
        editSession.commit({ updaterSettings: { ...initialUpdaterSettings } });
        if (updaterPathChanged) await reloadTargetAppState();
        else mergeUpdaterState(savedUpdaterState || {});
      }
      if (settingsChanged || options.forceRebuild) {
        saveStatus(result || { calendarStatus: "disabled" }, !settingsChanged);
      } else if (updaterSettingsChanged && connectionChanged) {
        setStatus("App Settings and Anytype connection saved.", "info");
      } else if (updaterSettingsChanged) {
        setStatus(updaterPathChanged
          ? "App Settings saved. Loaded settings from the selected Anytype data folder."
          : "App Settings saved.", "info");
      } else if (formulasChanged && !connectionChanged) {
        setStatus("Formulas saved. Press Ctrl+R in Anytype to apply.", "info", false);
      } else {
        saveStatus(result || { calendarStatus: "disabled" }, true);
      }
    } catch (error) {
      const message = error.message || String(error);
      setStatus(connectionSaved && settingsChanged
        ? "Connection saved. Other settings could not be saved: " + message
        : connectionSaved || generalSettingsSaved || updaterSettingsSaved || formulasSaved
          ? "Some settings were saved. Other changes could not be saved: " + message
          : message, "error");
    } finally {
      saveBusy = false;
      void refreshCalendarApiAvailability();
      updateSaveButton();
    }
  }

  hueInput.addEventListener("input", () => {
    if (isInstallingUpdate()) return;
    hsv.h = Number(hueInput.value);
    chooseCustomColor();
  });
  chooseBannerImage.addEventListener("click", () => {
    if (!chooseBannerImage.disabled && !isInstallingUpdate()) {
      invalidateBannerRead();
      bannerImageInput.click();
    }
  });
  bannerImageInput.addEventListener("change", handleBannerFileChange);
  resetBannerImage.addEventListener("click", () => {
    if (!draft || saveBusy || isInstallingUpdate() || !window.appReady) return;
    draft.bannerImage = "";
    el("banner-error").textContent = "";
    el("banner-image-status").textContent = "Default banner selected.";
    syncBannerPreview();
    markDirty();
  });
  saturationInput.addEventListener("input", () => {
    if (isInstallingUpdate()) return;
    hsv.s = Number(saturationInput.value);
    chooseCustomColor();
  });
  brightnessInput.addEventListener("input", () => {
    if (isInstallingUpdate()) return;
    hsv.v = Number(brightnessInput.value);
    chooseCustomColor();
  });
  function pickAtPointer(event) {
    const bounds = plane.getBoundingClientRect();
    hsv.s = Math.max(0, Math.min(100, (event.clientX - bounds.left) / bounds.width * 100));
    hsv.v = 100 - Math.max(0, Math.min(100, (event.clientY - bounds.top) / bounds.height * 100));
    chooseCustomColor();
  }
  plane.addEventListener("pointerdown", (event) => {
    if (!draft || saveBusy || isInstallingUpdate() || nativeAccent.checked || event.button !== 0) return;
    event.preventDefault();
    plane.setPointerCapture(event.pointerId);
    saturationInput.focus({ preventScroll: true });
    pickAtPointer(event);
  });
  plane.addEventListener("pointermove", (event) => {
    if (plane.hasPointerCapture(event.pointerId) && !saveBusy && !isInstallingUpdate() && !nativeAccent.checked) pickAtPointer(event);
  });
  plane.addEventListener("pointerup", (event) => {
    if (plane.hasPointerCapture(event.pointerId)) plane.releasePointerCapture(event.pointerId);
  });
  nativeAccent.addEventListener("change", () => {
    if (!draft || saveBusy || isInstallingUpdate()) return;
    if (nativeAccent.checked) {
      lastCustomAccent = { accentMode: draft.accentMode, accentPreset: draft.accentPreset, accentColor: draft.accentColor };
      Object.assign(draft, { accentMode: "native", accentPreset: "", accentColor: "Default" });
    } else {
      Object.assign(draft, lastCustomAccent || { accentMode: "custom", accentPreset: "", accentColor: "#de50de" });
    }
    accentValue.value = draft.accentColor;
    accentValue.classList.remove("invalid");
    el("accent-error").textContent = "";
    renderPicker(true);
    markDirty();
  });
  accentValue.addEventListener("input", () => {
    if (!draft || saveBusy || isInstallingUpdate()) return;
    const raw = accentValue.value.trim();
    Object.assign(draft, normalizeForUi({ accentColor: raw }));
    // Empty input stays invalid until the user supplies a color.
    if (!raw) Object.assign(draft, { accentMode: "custom", accentPreset: "", accentColor: "" });
    if (draft.accentMode !== "custom") accentValue.value = draft.accentColor;
    if (isCoreSupportedCustomAccent(raw) || draft.accentMode !== "custom") renderPicker(true);
    accentValue.classList.remove("invalid");
    el("accent-error").textContent = "";
    markDirty();
  });
  fontFamily.addEventListener("input", () => {
    if (!draft || isInstallingUpdate()) return;
    draft.fontFamily = fontFamily.value.trim();
    applyFontFamilies();
    el("font-error").textContent = "";
    fontFamily.classList.remove("invalid");
    markDirty();
  });
  codeFontFamily.addEventListener("input", () => {
    if (!draft || isInstallingUpdate()) return;
    draft.codeFontFamily = codeFontFamily.value.trim();
    applyFontFamilies();
    el("font-error").textContent = "";
    codeFontFamily.classList.remove("invalid");
    markDirty();
  });
  advancedApiKey.addEventListener("input", () => {
    if (!draft || saveBusy || isInstallingUpdate()) return;
    advancedKeyEdited = true;
    advancedApiKey.classList.remove("invalid");
    el("advanced-api-error").textContent = "";
    markDirty();
  });
  advancedApiIp.addEventListener("input", () => {
    if (!draft || saveBusy || isInstallingUpdate()) return;
    advancedApiIp.classList.remove("invalid");
    el("advanced-api-error").textContent = "";
    markDirty();
  });
  advancedApiPort.addEventListener("input", () => {
    if (!draft || saveBusy || isInstallingUpdate()) return;
    advancedApiPort.classList.remove("invalid");
    el("advanced-api-error").textContent = "";
    markDirty();
  });
  for (const [control, property] of [
    [includeSelections, "includeSelections"],
    [reducedMotion, "reducedMotion"],
    [tabHoverEffect, "tabHoverEffect"],
    [linkHoverEffect, "linkHoverEffect"],
    [objectMentionHoverEffect, "objectMentionHoverEffect"],
    [collapsedCategories, "collapsedCategories"],
    [calendarColorsEnabled, "calendarColorsEnabled"],
  ]) {
    control.addEventListener("change", () => {
      if (!draft || saveBusy || isInstallingUpdate()) return;
      if (property === "calendarColorsEnabled" && calendarColorsEnabled.disabled) {
        control.checked = draft[property];
        return;
      }
      draft[property] = control.checked;
      if (property === "reducedMotion") root.dataset.reducedMotion = String(draft.reducedMotion);
      if (property === "includeSelections") root.dataset.includeSelections = String(draft.includeSelections);
      markDirty();
    });
  }
  let folderPickerBusy = false;
  kanbanViewStyle.addEventListener("change", () => {
    if (!draft || saveBusy || isInstallingUpdate()) return;
    if (!["default", "balanced", "compact"].includes(kanbanViewStyle.value)) return;
    syncKanbanStyleSelection(kanbanViewStyle.value);
    draft.kanbanViewStyle = kanbanViewStyle.value;
    markDirty();
  });
  async function selectAnytypeFolder() {
    if (folderPickerBusy || saveBusy || isInstallingUpdate() || !updaterDraft) return;
    folderPickerBusy = true;
    installDirectory.setAttribute("aria-busy", "true");
    try {
      const selected = await invoke("select_anytype_folder", { currentDirectory: installDirectory.value });
      if (!selected || saveBusy || isInstallingUpdate()) return;
      installDirectory.value = selected;
      updaterDraft.installDirectory = selected;
      installDirectory.classList.remove("invalid");
      el("updater-settings-error").textContent = "";
      if (!window.appReady) {
        await invoke("recover_install_directory", { installDirectory: selected });
        initialUpdaterSettings = { ...updaterDraft };
        await loadState();
        return;
      }
      markDirty();
    } catch (error) {
      el("updater-settings-error").textContent = error.message || String(error);
    } finally {
      folderPickerBusy = false;
      installDirectory.removeAttribute("aria-busy");
    }
  }
  installDirectory.addEventListener("click", selectAnytypeFolder);
  installDirectory.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectAnytypeFolder();
    }
  });
  installDirectory.addEventListener("input", () => {
    if (!updaterDraft || saveBusy || isInstallingUpdate()) return;
    updaterDraft.installDirectory = installDirectory.value;
    installDirectory.classList.remove("invalid");
    el("updater-settings-error").textContent = "";
    markDirty();
  });
  for (const [control, property] of [
    [startOnLogin, "startOnLogin"],
    [autoplayVideos, "autoplayVideos"],
    [automaticUpdatesEnabled, "automaticUpdatesEnabled"],
    [automaticChecksEnabled, "automaticChecksEnabled"],
  ]) {
    control?.addEventListener("change", () => {
      if (!updaterDraft || saveBusy || isInstallingUpdate()) return;
      updaterDraft[property] = control.checked;
      if (property === "autoplayVideos") applyAutoplay(control.checked);
      markDirty();
    });
  }
  checkUpdatesButton.addEventListener("click", () => {
    if (updaterState && updaterState.updateAvailable === true) applyUpdate();
    else checkForUpdates(false);
  });
  updateNowButton.addEventListener("click", () => applyUpdate());
  const externalLinkController = window.BlobUI.ExternalLinks.attach(document, {
    onError(error) {
      setStatus("Could not open the link in your default browser. " + (error.message || String(error)), "error");
    },
  });
  window.addEventListener("unload", () => externalLinkController.destroy(), { once: true });
  const blobImportModal = window.BlobUI.Components.Modal({
    backdrop: el("blob-import-backdrop"), dialog: el("blob-import-dialog"),
    initialFocus: el("blob-import-cancel"), fallbackFocus: el("import-blob-backup-button"),
    inertTargets: [el("settings-shell"), el("title-drag-region")],
    onClose: () => { selectedBlobBackupDirectory = null; },
  });
  const uninstallModal = window.BlobUI.Components.Modal({
    backdrop: el("uninstall-backdrop"), dialog: el("uninstall-dialog"),
    initialFocus: el("uninstall-cancel"), fallbackFocus: el("uninstall-button"),
    inertTargets: [el("settings-shell"), el("title-drag-region")],
  });
  el("uninstall-button").addEventListener("click", () => {
    if (saveBusy || updaterBusyOperation || blobFolderBusy || uninstallBusy || !window.appReady) return;
    el("uninstall-error").textContent = "";
    uninstallModal.open();
  });
  ["uninstall-close", "uninstall-cancel"].forEach((id) => el(id).addEventListener("click", () => uninstallModal.close()));
  el("uninstall-confirm").addEventListener("click", async () => {
    if (saveBusy || updaterBusyOperation || blobFolderBusy || uninstallBusy) return;
    uninstallBusy = true;
    renderBlobBackupControls();
    const button = el("uninstall-confirm");
    button.disabled = true;
    ["uninstall-close", "uninstall-cancel"].forEach((id) => { el(id).disabled = true; });
    try { await invoke("uninstall_blobui"); }
    catch (error) { el("uninstall-error").textContent = String(error); }
    finally {
      uninstallBusy = false;
      renderBlobBackupControls();
      button.disabled = false;
      ["uninstall-close", "uninstall-cancel"].forEach((id) => { el(id).disabled = false; });
    }
  });
  function setBlobBackupStatus(message, isError = false) {
    blobBackupStatus.textContent = message;
    blobBackupStatus.classList.toggle("error", isError);
  }
  createBlobBackupButton.addEventListener("click", async () => {
    if (createBlobBackupButton.disabled) return;
    blobFolderBusy = true;
    updateSaveButton();
    try {
      const destination = await invoke("select_blob_backup_destination", {
        currentDirectory: installDirectory.value || null,
      });
      if (!destination) return;
      const backup = await invoke("create_blob_backup", { destinationDirectory: destination });
      setBlobBackupStatus("Backup exported: " + backup);
    } catch (error) {
      setBlobBackupStatus(error.message || String(error), true);
    } finally {
      blobFolderBusy = false;
      updateSaveButton();
    }
  });
  importBlobBackupButton.addEventListener("click", async () => {
    if (importBlobBackupButton.disabled) return;
    blobFolderBusy = true;
    updateSaveButton();
    try {
      const backup = await invoke("select_blob_backup_folder", {
        currentDirectory: installDirectory.value || null,
      });
      if (!backup) return;
      selectedBlobBackupDirectory = backup;
      el("blob-import-path").textContent = backup;
      el("blob-import-error").textContent = "";
      blobImportModal.open();
    } catch (error) {
      setBlobBackupStatus(error.message || String(error), true);
    } finally {
      blobFolderBusy = false;
      updateSaveButton();
    }
  });
  ["blob-import-close", "blob-import-cancel"].forEach((id) => {
    el(id).addEventListener("click", () => blobImportModal.close());
  });
  el("blob-import-confirm").addEventListener("click", async () => {
    if (!selectedBlobBackupDirectory || blobFolderBusy || saveBusy || updaterBusyOperation || uninstallBusy) return;
    const backup = selectedBlobBackupDirectory;
    blobFolderBusy = true;
    el("blob-import-confirm").disabled = true;
    ["blob-import-close", "blob-import-cancel"].forEach((id) => { el(id).disabled = true; });
    el("blob-import-error").textContent = "";
    updateSaveButton();
    let restored = false;
    try {
      await invoke("import_blob_backup", { backupDirectory: backup });
      restored = true;
      await reloadTargetAppState({ reloadFormulas: true });
      blobImportModal.close({ restoreFocus: false });
      setBlobBackupStatus("Backup imported: " + backup);
    } catch (error) {
      const message = error.message || String(error);
      if (restored) {
        blobRestoreReloadFailed = true;
        updateSaveButton();
        el("blob-import-error").textContent = "The backup was restored, but Settings could not reload it. Restart Settings before editing. " + message;
      } else {
        el("blob-import-error").textContent = message;
      }
    } finally {
      blobFolderBusy = false;
      el("blob-import-confirm").disabled = false;
      ["blob-import-close", "blob-import-cancel"].forEach((id) => { el(id).disabled = false; });
      updateSaveButton();
    }
  });
  saveButton.addEventListener("click", save);
  discardButton.addEventListener("click", () => window.BlobUI.openDiscardConfirmation());
  el("discard-confirmation-close").addEventListener("click", () => window.BlobUI.closeDiscardConfirmation());
  el("discard-confirmation-undo").addEventListener("click", () => {
    if (saveBusy || isInstallingUpdate()) return;
    window.BlobUI.discardChanges();
    window.BlobUI.closeDiscardConfirmation();
  });
  function discardAllFromApiRemovalModal() {
    if (saveBusy || isInstallingUpdate()) return;
    window.BlobUI.discardChanges();
    apiKeyRemovalModal.close();
  }
  apiKeyRemovalClose.addEventListener("click", discardAllFromApiRemovalModal);
  apiKeyRemovalUndo.addEventListener("click", discardAllFromApiRemovalModal);
  apiKeyRemovalSave.addEventListener("click", () => {
    if (saveBusy || isInstallingUpdate()) return;
    apiKeyRemovalModal.close({ restoreFocus: false });
    save({ confirmKeyRemoval: true });
  });
  logButton.addEventListener("click", () => {
    logPopup.hidden = !logPopup.hidden;
    logButton.setAttribute("aria-expanded", String(!logPopup.hidden));
    if (!logPopup.hidden) {
      updateLogsScrollbar();
      el("copy-logs").focus();
    }
  });
  const helpButton = el("help-button");
  const helpPopup = el("help-popup");
  function closeHelp(restoreFocus = false) {
    helpPopup.hidden = true;
    helpButton.setAttribute("aria-expanded", "false");
    if (restoreFocus) helpButton.focus();
  }
  helpButton.addEventListener("click", () => {
    helpPopup.hidden = !helpPopup.hidden;
    helpButton.setAttribute("aria-expanded", String(!helpPopup.hidden));
    if (!helpPopup.hidden) el("help-whats-new").focus();
  });
  document.addEventListener("pointerdown", (event) => {
    if (!el("help-control").contains(event.target)) closeHelp();
  });
  document.addEventListener("focusin", (event) => {
    if (!el("help-control").contains(event.target)) closeHelp();
  });
  el("help-whats-new").addEventListener("click", () => { closeHelp(); showLatestReleaseNotes(); });
  el("help-documentation").addEventListener("click", () => { closeHelp(); showDocumentation(); });
  el("help-keybinds").addEventListener("click", async () => {
    closeHelp();
    el("keybind-status").textContent = "";
    el("keybind-status").classList.remove("error");
    await loadShortcutBindings();
    keybindModal.open();
  });
  el("help-error-close").addEventListener("click", () => helpErrorModal.close());
  el("keybinds-close").addEventListener("click", () => keybindModal.close());
  document.addEventListener("pointerdown", (event) => {
    if (!el("log-control").contains(event.target)) closeLogs();
  });
  document.addEventListener("focusin", (event) => {
    if (!el("log-control").contains(event.target)) closeLogs();
  });
  el("copy-logs").addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(logs.map(formatLog).join("\n"));
      el("copy-status").textContent = "All logs copied.";
      el("copy-logs").title = "All logs copied";
    } catch (error) {
      setStatus("Could not copy logs. " + (error.message || String(error)), "error");
      el("copy-status").textContent = "Could not copy logs.";
    }
  });

  async function windowAction(command) {
    try {
      await invoke(command);
    } catch (error) {
      setStatus(error.message || String(error), "error");
    }
  }
  el("window-minimize").addEventListener("click", () => windowAction("minimize_window"));
  el("window-close").addEventListener("click", () => windowAction("hide_window"));
  el("open-author").addEventListener("click", () => windowAction("open_author_profile"));
  el("open-repository").addEventListener("click", () => windowAction("open_project_repository"));
  const onboarding = el("onboarding");
  const onboardingWelcome = el("onboarding-welcome");
  const onboardingApi = el("onboarding-api");
  const documentation = el("documentation");
  const documentationBack = el("documentation-back");
  const apiOptions = [el("onboarding-no-api-option"), el("onboarding-api-option")];
  // Add future pages here. Every page advances through the same transition;
  // reaching the end always reveals the main app.
  const onboardingSteps = [
    { page: onboardingWelcome, focus: onboardingWelcome },
    { page: onboardingApi, focus: el("onboarding-api-title") },
    { page: el("onboarding-connection"), focus: el("connection-title"), when: () => onboarding.dataset.apiChoice === "api" },
  ];
  let onboardingStep = 0;
  let onboardingPreviousFocus = null;
  let onboardingTransition = null;
  let documentationPreviousFocus = null;
  let documentationTransition = null;

  function resetDocumentationTransition() {
    window.clearTimeout(documentationTransition);
    documentationTransition = null;
    documentation.classList.remove("leaving");
    settingsShell.classList.remove("onboarding-entering");
  }

  function hideDocumentationImmediately() {
    const previousFocus = documentationPreviousFocus;
    resetDocumentationTransition();
    documentation.hidden = true;
    documentationBack.hidden = true;
    settingsShell.inert = false;
    documentationPreviousFocus = null;
    return previousFocus;
  }

  function showDocumentation() {
    if (connectionBusy || !onboarding.hidden || !documentation.hidden) return;
    resetDocumentationTransition();
    closeLogs();
    documentationPreviousFocus = document.activeElement;
    settingsShell.inert = true;
    documentation.hidden = false;
    documentationBack.hidden = false;
    documentationBack.focus();
    window.BlobUI.DocsUI.load();
  }

  function closeDocumentation() {
    if (documentation.hidden || documentationTransition !== null) return;
    documentation.classList.add("leaving");
    const reducedMotion = root.dataset.reducedMotion === "true"
      || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    documentationTransition = window.setTimeout(() => {
      documentationTransition = null;
      documentation.classList.remove("leaving");
      documentation.hidden = true;
      documentationBack.hidden = true;
      settingsShell.inert = false;
      if (documentationPreviousFocus) documentationPreviousFocus.focus();
      documentationPreviousFocus = null;
    }, reducedMotion ? 0 : 320);
  }

  documentationBack.addEventListener("click", closeDocumentation);

  function resetOnboardingTransition() {
    window.clearTimeout(onboardingTransition);
    onboardingTransition = null;
    onboardingSteps.forEach(({ page }) => page.classList.remove("leaving"));
    settingsShell.classList.remove("onboarding-entering");
  }

  function showOnboardingStep(index) {
    onboardingStep = index;
    onboardingSteps.forEach(({ page }, position) => { page.hidden = position !== index; });
    updateOnboardingScrollbar();
    updateConnectionScrollbar();
    onboardingSteps[index].focus.focus();
  }

  function showOnboarding() {
    if (connectionBusy || isInstallingUpdate()) return;
    const documentationFocus = documentation.hidden ? null : hideDocumentationImmediately();
    if (onboarding.hidden) onboardingPreviousFocus = documentationFocus || document.activeElement;
    resetOnboardingTransition();
    closeLogs();
    apiOptions.forEach((item) => item.setAttribute("aria-pressed", "false"));
    delete onboarding.dataset.apiChoice;
    el("connection-key").value = "";
    resetVisibilityField("connection-key");
    resetVisibilityField("connection-port");
    el("connection-error").textContent = "";
    el("onboarding-choice-error").textContent = "";
    settingsShell.inert = true;
    root.classList.add("onboarding-active");
    onboarding.hidden = false;
    showOnboardingStep(0);
  }

  function closeOnboarding() {
    if (connectionBusy || isInstallingUpdate()) return;
    el("connection-key").value = "";
    resetVisibilityField("connection-key");
    resetOnboardingTransition();
    onboarding.hidden = true;
    settingsShell.inert = false;
    root.classList.remove("onboarding-active");
    settingsShell.classList.add("onboarding-entering");
    if (onboardingPreviousFocus) onboardingPreviousFocus.focus();
    showReleaseNotesOrContinue();
  }

  function advanceOnboarding() {
    if (isInstallingUpdate() || onboarding.hidden || onboardingTransition !== null) return;
    const current = onboardingSteps[onboardingStep].page;
    current.classList.add("leaving");
    onboardingTransition = window.setTimeout(() => {
      onboardingTransition = null;
      current.classList.remove("leaving");
      let next = onboardingStep + 1;
      while (next < onboardingSteps.length && onboardingSteps[next].when && !onboardingSteps[next].when()) next += 1;
      if (next < onboardingSteps.length) {
        showOnboardingStep(next);
      } else {
        closeOnboarding();
      }
    }, window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 320);
  }

  onboardingWelcome.addEventListener("click", advanceOnboarding);
  function setConnectionBusy(busy) {
    connectionBusy = busy;
    apiOptions.forEach((option) => { option.disabled = busy; });
    el("connection-confirm").disabled = busy;
    el("connection-back").disabled = busy;
    el("connection-key").disabled = busy;
    el("connection-port").disabled = busy;
    el("connection-key-visibility").disabled = busy;
    el("connection-port-visibility").disabled = busy;
    el("connection-confirm").textContent = busy ? "Connecting…" : "Confirm";
  }

  async function applyConnection(enabled, apiKey = "", port = null) {
    if (isInstallingUpdate() || blobFolderBusy || blobRestoreReloadFailed) return;
    const advancedWasDirty = connectionHasChanges();
    const appliedPort = port || 31009;
    setConnectionBusy(true);
    el("connection-error").textContent = "";
    try {
      const result = await invoke("configure_connection", { connection: {
        enabled,
        apiKey,
        keepExistingKey: false,
        ip: currentApiIp || null,
        port,
      } });
      window.appReady = true;
      currentApiPort = appliedPort;
      hasSavedApiKey = enabled;
      if (!advancedWasDirty) {
        assignAdvancedConnection({ apiIp: currentApiIp, apiPort: currentApiPort, hasSavedApiKey });
      } else {
        initialApiDraft = {
          ip: currentApiIp === "127.0.0.1" ? "" : currentApiIp,
          port: currentApiPort === 31009 ? "" : String(currentApiPort),
        };
        if (!advancedKeyEdited) {
          advancedApiKey.value = hasSavedApiKey ? SAVED_KEY_MASK : "";
          advancedApiKey.placeholder = hasSavedApiKey ? "" : "Paste a read-only API v2 key";
        }
        el("advanced-api-key-hint").textContent = hasSavedApiKey
          ? "A key is saved. Leave the masked value unchanged to keep it, or clear it to remove it."
          : "Enter a new read-only Anytype API v2 key.";
      }
      resetAdvancedVisibility();
      editSession.commit({ connection: readConnectionBaseline() });
      updateSaveButton();
      const calendarPending = enabled && result && result.calendarStatus === "cached";
      setStatus(calendarPending
        ? "Anytype connected. Calendar data could not refresh yet; try saving again later."
        : enabled ? "Anytype connected. Press Ctrl+R in Anytype to reload CSS." : "API features disabled. Press Ctrl+R in Anytype to reload CSS.", "info");
      el("connection-key").value = "";
      resetVisibilityField("connection-key");
      advanceOnboarding();
    } catch (error) {
      if (enabled) {
        el("connection-error").textContent = error.message || String(error);
      } else {
        setStatus(error.message || String(error), "error");
        el("onboarding-choice-error").textContent = "Could not apply this choice. Check your BlobUI installation and try again.";
      }
    } finally {
      setConnectionBusy(false);
      void refreshCalendarApiAvailability();
    }
  }

  apiOptions.forEach((option) => option.addEventListener("click", () => {
    if (connectionBusy || isInstallingUpdate() || onboarding.hidden || onboardingSteps[onboardingStep].page !== onboardingApi || onboardingTransition !== null) return;
    apiOptions.forEach((item) => item.setAttribute("aria-pressed", String(item === option)));
    el("onboarding-choice-error").textContent = "";
    const enabled = option === apiOptions[1];
    onboarding.dataset.apiChoice = enabled ? "api" : "no-api";
    if (enabled) advanceOnboarding();
    else applyConnection(false, "", currentApiPort);
  }));

  el("connection-back").addEventListener("click", () => {
    if (connectionBusy || isInstallingUpdate() || onboardingTransition !== null) return;
    resetOnboardingTransition();
    showOnboardingStep(onboardingSteps.findIndex(({ page }) => page === onboardingApi));
  });
  el("connection-form").addEventListener("submit", (event) => {
    event.preventDefault();
    if (connectionBusy || isInstallingUpdate() || onboardingTransition !== null || onboarding.hidden) return;
    const key = el("connection-key").value.trim();
    const portText = el("connection-port").value.trim();
    if (!key || key.length > 4096 || /[\x00-\x1f\x7f]/.test(key)) {
      el("connection-error").textContent = "Enter your Anytype API v2 key.";
      el("connection-key").focus();
      return;
    }
    if (portText && (!/^\d+$/.test(portText) || Number(portText) < 1 || Number(portText) > 65535)) {
      el("connection-error").textContent = "Port must be a whole number from 1 to 65535.";
      el("connection-port").focus();
      return;
    }
    applyConnection(true, key, portText ? Number(portText) : null);
  });

  document.addEventListener("keydown", (event) => {
    if (event.defaultPrevented) return;
    if (event.key === "Escape" && kanbanViewStyle.getAttribute("aria-expanded") === "true") {
      event.preventDefault();
      closeKanbanStyleMenu(true);
      return;
    }
    if (blobFolderBusy || blobRestoreReloadFailed || blobImportModal.isOpen()
      || apiKeyRemovalModal.isOpen() || discardConfirmationModal.isOpen() || keybindModal.isOpen()) return;
    const matches = (action) => {
      const binding = shortcutBindings.find((item) => item.action === action);
      return binding && event.code === binding.code && event.ctrlKey === binding.ctrl
        && event.altKey === binding.alt && event.shiftKey === binding.shift && event.metaKey === binding.meta;
    };
    if (matches("rebuildCss")) {
      event.preventDefault();
      if (!event.repeat) save({ forceRebuild: true });
      return;
    }
    if (matches("showOnboarding")) {
      event.preventDefault();
      if (!event.repeat) showOnboarding();
      return;
    }
    if (matches("simulateUpdate")) {
      event.preventDefault();
      if (!event.repeat) simulateUpdate();
      return;
    }
    if (event.key === "Escape" && !onboarding.hidden) {
      event.preventDefault();
      closeOnboarding();
      return;
    }
    if (event.key === "Escape" && !logPopup.hidden) {
      event.preventDefault();
      closeLogs(true);
    }
    if (matches("quitSettings")) {
      event.preventDefault();
      if (!event.repeat) windowAction("quit_app");
    }
  });
  el("title-drag-region").addEventListener("pointerdown", (event) => {
    if (event.button === 0 && !event.target.closest("button")) {
      windowAction("start_window_drag");
    }
  });

  function markDirty() {
    if (!draft || isInstallingUpdate()) return;
    updateSaveButton();
    if (!saveBusy && window.appReady) {
      setStatus(editSession.isDirty()
        ? "Unsaved changes are ready to apply."
        : "No unsaved changes.", "info", false);
    }
  }

  const eventApi = window.__TAURI__ && window.__TAURI__.event;
  if (eventApi && typeof eventApi.listen === "function") {
    eventApi.listen("blobui-rebuild-custom-css", () => save({ forceRebuild: true })).catch((error) => {
      setStatus(error.message || String(error), "error");
    });
    eventApi.listen("blobui-show-onboarding", () => showOnboarding()).catch((error) => {
      setStatus(error.message || String(error), "error");
    });
  }

  window.BlobUI.AppReload = window.BlobUI.ReloadController.create();
  window.BlobUI.FormulaPage.init({ navigate: showSettingsPage, onChange: markDirty, onInfo: message => addLog(message, "info"), onBusyChange(value) { saveBusy = value; updateSaveButton(); } });
  window.BlobUI.SettingsV21?.init({ showSettingsPage, selectAdvancedTab });
  el("settings-v21-enabled").addEventListener("change", () => {
    if (!draft || saveBusy || isInstallingUpdate()) return;
    draft.settingsV21Enabled = el("settings-v21-enabled").checked;
    window.BlobUI.SettingsV21?.setEnabled(draft.settingsV21Enabled);
    markDirty();
  });
  window.appReady = false;
  loadState();
  window.setInterval(refreshTheme, 2000);
  window.setInterval(() => {
    if (initialUpdaterSettings && initialUpdaterSettings.automaticChecksEnabled && !simulationActive) return checkForUpdates(true);
  }, 5 * 60 * 1000);
  window.matchMedia("(prefers-color-scheme: light)").addEventListener("change", refreshTheme);
}());
