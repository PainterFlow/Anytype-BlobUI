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
  const nativeAccent = el("native-accent");
  const swatch = el("accent-swatch");
  const accentValue = el("accent-value");
  const fontFamily = el("font-family");
  const codeFontFamily = el("code-font-family");
  const reducedMotion = el("reduced-motion");
  const tabHoverEffect = el("tab-hover-effect");
  const linkHoverEffect = el("link-hover-effect");
  const objectMentionHoverEffect = el("object-mention-hover-effect");
  const preferencesNav = el("preferences-nav");
  const accessibilityNav = el("accessibility-nav");
  const preferencesPage = el("preferences-page");
  const accessibilityPage = el("accessibility-page");
  const pageTitle = el("page-title");
  const saveButton = el("save-button");
  const saveLabel = saveButton.querySelector(".save-label");
  const statusMessage = el("status-message");
  const logButton = el("log-button");
  const logPopup = el("log-popup");
  const logEntries = el("log-entries");
  const pageContent = document.querySelector(".page-content");
  const logs = [];
  const root = document.documentElement;

  let draft = null;
  let initialDraft = null;
  let saveBusy = false;
  let hsv = { h: 300, s: 64, v: 87 };
  let lastCustomAccent = null;
  let lastTheme = "";
  function setupScrollbar(container, contentSelector) {
    let hideTimer = null;
    function update() {
      const canScroll = container.scrollHeight > container.clientHeight;
      container.classList.toggle("has-overflow", canScroll);
      if (canScroll) return;
      container.classList.remove("is-scrolling");
      if (hideTimer !== null) {
        window.clearTimeout(hideTimer);
        hideTimer = null;
      }
    }
    container.addEventListener("scroll", () => {
      update();
      if (!container.classList.contains("has-overflow")) return;
      container.classList.add("is-scrolling");
      if (hideTimer !== null) window.clearTimeout(hideTimer);
      hideTimer = window.setTimeout(() => {
        container.classList.remove("is-scrolling");
        hideTimer = null;
      }, 900);
    }, { passive: true });
    if (typeof ResizeObserver === "function") {
      const observer = new ResizeObserver(update);
      observer.observe(container);
      const content = container.querySelector(contentSelector);
      if (content) observer.observe(content);
    }
    update();
    return update;
  }
  const updatePageScrollbar = setupScrollbar(pageContent, ".settings-content");
  const updateConnectionScrollbar = setupScrollbar(el("connection-scroll"), ".connection-content");
  const updateOnboardingScrollbar = setupScrollbar(el("onboarding-scroll"), ".onboarding-options");
  window.addEventListener("resize", () => {
    updatePageScrollbar();
    updateOnboardingScrollbar();
    updateConnectionScrollbar();
  });

  function invoke(name, args) {
    const api = window.__TAURI__ && window.__TAURI__.core;
    if (!api || typeof api.invoke !== "function") {
      return Promise.reject(new Error("The settings app bridge is unavailable. Launch this window from scripts/Settings.ps1."));
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
    if (!draft || saveBusy) return;
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
    return !!left && !!right && left.fontFamily === right.fontFamily && left.codeFontFamily === right.codeFontFamily && accentIdentity(left) === accentIdentity(right) && left.reducedMotion === right.reducedMotion && left.tabHoverEffect === right.tabHoverEffect && left.linkHoverEffect === right.linkHoverEffect && left.objectMentionHoverEffect === right.objectMentionHoverEffect;
  }

  function setStatus(message, kind, record = true) {
    statusMessage.textContent = message;
    if (!record) return;
    const level = kind === "error" || kind === "warning" ? kind : "info";
    const entry = { message: String(message), level, time: new Date().toLocaleTimeString() };
    logs.push(entry);
    const row = document.createElement("p");
    row.className = "log-entry " + level;
    row.textContent = formatLog(entry);
    logEntries.appendChild(row);
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

  function updateSaveButton() {
    const dirty = !settingsEqual(draft, initialDraft);
    saveButton.hidden = !draft || !dirty;
    saveButton.disabled = saveBusy || !draft || !window.appReady || !dirty;
    document.querySelectorAll("input").forEach((control) => {
      control.disabled = saveBusy || !draft || (!!draft && draft.accentMode === "native" && [hueInput, saturationInput, brightnessInput, accentValue].includes(control));
    });
    saveButton.classList.toggle("busy", saveBusy);
    saveLabel.textContent = saveBusy ? "Saving…" : "Save changes";
  }

  function applyFontFamilies() {
    const font = fontFamily.value.trim();
    const codeFont = codeFontFamily.value.trim();
    const validFamily = /^[\p{L}\p{N}][\p{L}\p{N} _-]{0,79}$/u;
    const interfaceFamily = validFamily.test(font) ? font : "Libron";
    const codeFamily = validFamily.test(codeFont) ? codeFont : "Consolas";
    root.style.setProperty("--font-interface", '"' + interfaceFamily + '", "Segoe UI", sans-serif');
    root.style.setProperty("--font-code", '"' + codeFamily + '", "Cascadia Code", monospace');
  }

  function assignDraftToForm(settings) {
    const accent = normalizeForUi(settings);
    draft = {
      fontFamily: String(settings.fontFamily || "Libron"),
      codeFontFamily: String(settings.codeFontFamily || "Consolas"),
      accentColor: accent.accentColor,
      accentMode: accent.accentMode,
      accentPreset: accent.accentPreset,
      reducedMotion: settings.reducedMotion === true,
      tabHoverEffect: settings.tabHoverEffect === true,
      linkHoverEffect: settings.linkHoverEffect === true,
      objectMentionHoverEffect: settings.objectMentionHoverEffect === true,
    };
    fontFamily.value = draft.fontFamily;
    codeFontFamily.value = draft.codeFontFamily;
    accentValue.value = draft.accentColor;
    reducedMotion.checked = draft.reducedMotion;
    tabHoverEffect.checked = draft.tabHoverEffect;
    linkHoverEffect.checked = draft.linkHoverEffect;
    objectMentionHoverEffect.checked = draft.objectMentionHoverEffect;
    renderPicker(true);
    applyFontFamilies();
    initialDraft = { ...draft };
    updateSaveButton();
  }

  async function loadState() {
    try {
      const state = await invoke("get_app_state");
      window.appReady = !!state.ready;
      applyTheme(state.theme);
      assignDraftToForm(state.settings || {});
      if (state.apiPort) el("connection-port").value = state.apiPort === 31009 ? "" : String(state.apiPort);
      if (state.ready && state.onboardingComplete === false) showOnboarding();
      if (!state.ready) {
        setStatus(state.message || "Run scripts/Install.ps1 once to set up BlobUI before saving.", "error");
      } else if (state.hotkeyError) {
        setStatus(state.hotkeyError, "error");
      } else {
        setStatus(state.message || "Settings loaded from this device.");
      }
    } catch (error) {
      window.appReady = false;
      setStatus(error.message || String(error), "error");
      updateSaveButton();
    }
  }

  function applyTheme(theme) {
    const next = theme === "light" || theme === "dark" ? theme : (window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark");
    root.dataset.theme = next;
    if (next !== lastTheme) {
      lastTheme = next;
      if (draft) renderPicker(true);
    }
  }

  function showSettingsPage(page) {
    const isAccessibility = page === "accessibility";
    preferencesPage.hidden = isAccessibility;
    accessibilityPage.hidden = !isAccessibility;
    pageTitle.textContent = isAccessibility ? "Accessibility" : "Preferences";
    preferencesNav.classList.toggle("active", !isAccessibility);
    accessibilityNav.classList.toggle("active", isAccessibility);
    preferencesNav.setAttribute("aria-current", isAccessibility ? "false" : "page");
    accessibilityNav.setAttribute("aria-current", isAccessibility ? "page" : "false");
    pageContent.scrollTop = 0;
    updatePageScrollbar();
  }

  preferencesNav.addEventListener("click", () => showSettingsPage("preferences"));
  accessibilityNav.addEventListener("click", () => showSettingsPage("accessibility"));
  showSettingsPage("preferences");

  async function refreshTheme() {
    try {
      const state = await invoke("get_app_state");
      applyTheme(state.theme);
    } catch (_) { /* Keep the last known mode while Anytype is not readable. */ }
  }

  async function save() {
    if (!draft || saveBusy || !window.appReady) return;
    draft.fontFamily = fontFamily.value.trim();
    draft.codeFontFamily = codeFontFamily.value.trim();
    draft.accentColor = accentValue.value.trim();
    draft.accentPreset = draft.accentMode === "preset" ? draft.accentPreset : "";

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
    if (invalid) {
      setStatus("Review the highlighted fields before saving.", "error");
      return;
    }

    const submittedSettings = { ...draft };
    saveBusy = true;
    updateSaveButton();
    setStatus("Saving your settings and rebuilding custom.css…", "working");
    try {
      const result = await invoke("save_settings", { settings: submittedSettings });
      initialDraft = submittedSettings;
      if (result.calendarStatus === "disabled") {
        setStatus("Saved. Press Ctrl+R in Anytype to apply. API features are disabled.");
      } else if (result.calendarStatus === "cached") {
        setStatus("Saved. Press Ctrl+R in Anytype to apply; calendar colors kept their last saved values.", "warning");
      } else {
        setStatus("Saved. Press Ctrl+R in Anytype to apply. Calendar colors refreshed.");
      }
    } catch (error) {
      setStatus(error.message || String(error), "error");
    } finally {
      saveBusy = false;
      updateSaveButton();
    }
  }

  hueInput.addEventListener("input", () => {
    hsv.h = Number(hueInput.value);
    chooseCustomColor();
  });
  saturationInput.addEventListener("input", () => {
    hsv.s = Number(saturationInput.value);
    chooseCustomColor();
  });
  brightnessInput.addEventListener("input", () => {
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
    if (!draft || saveBusy || nativeAccent.checked || event.button !== 0) return;
    event.preventDefault();
    plane.setPointerCapture(event.pointerId);
    saturationInput.focus({ preventScroll: true });
    pickAtPointer(event);
  });
  plane.addEventListener("pointermove", (event) => {
    if (plane.hasPointerCapture(event.pointerId) && !saveBusy && !nativeAccent.checked) pickAtPointer(event);
  });
  plane.addEventListener("pointerup", (event) => {
    if (plane.hasPointerCapture(event.pointerId)) plane.releasePointerCapture(event.pointerId);
  });
  nativeAccent.addEventListener("change", () => {
    if (!draft || saveBusy) return;
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
    if (!draft || saveBusy) return;
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
    if (!draft) return;
    draft.fontFamily = fontFamily.value.trim();
    applyFontFamilies();
    el("font-error").textContent = "";
    fontFamily.classList.remove("invalid");
    markDirty();
  });
  codeFontFamily.addEventListener("input", () => {
    if (!draft) return;
    draft.codeFontFamily = codeFontFamily.value.trim();
    applyFontFamilies();
    el("font-error").textContent = "";
    codeFontFamily.classList.remove("invalid");
    markDirty();
  });
  for (const [control, property] of [
    [reducedMotion, "reducedMotion"],
    [tabHoverEffect, "tabHoverEffect"],
    [linkHoverEffect, "linkHoverEffect"],
    [objectMentionHoverEffect, "objectMentionHoverEffect"],
  ]) {
    control.addEventListener("change", () => {
      if (!draft || saveBusy) return;
      draft[property] = control.checked;
      markDirty();
    });
  }
  saveButton.addEventListener("click", save);
  logButton.addEventListener("click", () => {
    logPopup.hidden = !logPopup.hidden;
    logButton.setAttribute("aria-expanded", String(!logPopup.hidden));
    if (!logPopup.hidden) el("copy-logs").focus();
  });
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
  const settingsShell = el("settings-shell");
  const apiOptions = [el("onboarding-no-api-option"), el("onboarding-api-option")];
  // Add future pages here. Every page advances through the same transition;
  // reaching the end always reveals the main app.
  const onboardingSteps = [
    { page: onboardingWelcome, focus: onboardingWelcome },
    { page: onboardingApi, focus: el("onboarding-api-title") },
    { page: el("onboarding-connection"), focus: el("connection-title"), when: () => onboarding.dataset.apiChoice === "api" },
  ];
  let connectionBusy = false;
  let onboardingStep = 0;
  let onboardingPreviousFocus = null;
  let onboardingTransition = null;

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
    if (connectionBusy) return;
    if (onboarding.hidden) onboardingPreviousFocus = document.activeElement;
    resetOnboardingTransition();
    closeLogs();
    apiOptions.forEach((item) => item.setAttribute("aria-pressed", "false"));
    delete onboarding.dataset.apiChoice;
    el("connection-key").value = "";
    el("connection-error").textContent = "";
    el("onboarding-choice-error").textContent = "";
    settingsShell.inert = true;
    root.classList.add("onboarding-active");
    onboarding.hidden = false;
    showOnboardingStep(0);
  }

  function closeOnboarding() {
    if (connectionBusy) return;
    el("connection-key").value = "";    resetOnboardingTransition();
    onboarding.hidden = true;
    settingsShell.inert = false;
    root.classList.remove("onboarding-active");
    if (onboardingPreviousFocus) onboardingPreviousFocus.focus();
  }

  function advanceOnboarding() {
    if (onboarding.hidden || onboardingTransition !== null) return;
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
        settingsShell.classList.add("onboarding-entering");
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
    el("connection-confirm").textContent = busy ? "Connecting…" : "Confirm";
  }

  async function applyConnection(enabled, apiKey = "", port = null) {
    setConnectionBusy(true);
    el("connection-error").textContent = "";
    try {
      const result = await invoke("configure_connection", { connection: { enabled, apiKey, port } });
      window.appReady = true;
      updateSaveButton();
      const calendarPending = enabled && result && result.calendarStatus === "cached";
      setStatus(calendarPending
        ? "Anytype connected. Calendar data could not refresh yet; try saving again later."
        : enabled ? "Anytype connected. Press Ctrl+R in Anytype to reload CSS." : "API features disabled. Press Ctrl+R in Anytype to reload CSS.", calendarPending ? "warning" : "info");
      el("connection-key").value = "";
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
    }
  }

  apiOptions.forEach((option) => option.addEventListener("click", () => {
    if (connectionBusy || onboarding.hidden || onboardingSteps[onboardingStep].page !== onboardingApi || onboardingTransition !== null) return;
    apiOptions.forEach((item) => item.setAttribute("aria-pressed", String(item === option)));
    el("onboarding-choice-error").textContent = "";
    const enabled = option === apiOptions[1];
    onboarding.dataset.apiChoice = enabled ? "api" : "no-api";
    if (enabled) advanceOnboarding();
    else applyConnection(false);
  }));

  el("open-connection").addEventListener("click", () => {
    showOnboarding();
    showOnboardingStep(onboardingSteps.findIndex(({ page }) => page === onboardingApi));
  });

  el("connection-back").addEventListener("click", () => {
    if (connectionBusy || onboardingTransition !== null) return;
    resetOnboardingTransition();
    showOnboardingStep(onboardingSteps.findIndex(({ page }) => page === onboardingApi));
  });
  el("connection-form").addEventListener("submit", (event) => {
    event.preventDefault();
    if (connectionBusy || onboardingTransition !== null || onboarding.hidden) return;
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
    if (event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey
      && typeof event.key === "string" && event.key.toLowerCase() === "r") {
      event.preventDefault();
      if (!event.repeat) return save();
      return;
    }
    if (event.key === "F8" && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      event.preventDefault();
      if (!event.repeat) showOnboarding();
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
    if (event.ctrlKey && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "q") {
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
    if (!draft) return;
    updateSaveButton();
    if (!saveBusy && window.appReady) {
      setStatus(settingsEqual(draft, initialDraft)
        ? "No unsaved changes."
        : "Unsaved changes are ready to apply.", "info", false);
    }
  }

  const eventApi = window.__TAURI__ && window.__TAURI__.event;
  if (eventApi && typeof eventApi.listen === "function") {
    eventApi.listen("blobui-rebuild-custom-css", () => save()).catch((error) => {
      setStatus(error.message || String(error), "error");
    });
  }

  window.appReady = false;
  loadState();
  window.setInterval(refreshTheme, 2000);
  window.matchMedia("(prefers-color-scheme: light)").addEventListener("change", refreshTheme);
}());
