(function (root) {
  "use strict";
  const UI = root.BlobUI, doc = root.document, C = UI.Components;
  const copy = value => JSON.parse(JSON.stringify(value));
  const defaults = () => ({ version: 1, types: [] });
  const key = type => JSON.stringify([type.spaceId, type.key || type.typeKey]);
  const targetKey = target => UI.ViewCatalog.targetIdentity(target);
  const make = UI.CatalogBrowser.node;
  let state = defaults(), catalog = { types: [], views: [] }, invoke, api, loading = true, locked = false;
  let browser, modalBrowser, modal, shell, dialog, content, controls, title, close, message, empty, list, page;
  let pickers = [], scrollController;
  let hasCatalog = false, discoveryError = false;
  let records = [], active, activeRow, screen = "type", generation = 0, discovering = false, discovered = false, loadFailed = false;
  function button(label, onClick, className = "formula-button") {
    return C.Button({ label, onClick, attributes: { class: className } }).element;
  }
  function changed() { api.onChange(); }
  function entry(type, create = false) {
    let value = state.types.find(item => key(item) === key(type));
    if (!value && create) { value = { spaceId: type.spaceId, typeKey: type.key, default: { mode: "inherit" }, overrides: [] }; state.types.push(value); }
    return value;
  }
  function label(rule, type) {
    if (!rule || rule.mode === "inherit") return "Use Calendar Color";
    if (rule.mode === "none") return "No color";
    return (type.properties || []).find(property => property.key === rule.key)?.name || "Unavailable property";
  }
  function cleanContent() { pickers.forEach(picker => picker.destroy()); pickers = []; dialog?.querySelectorAll(".calendar-property-menu").forEach(menu => menu.remove()); scrollController?.destroy(); scrollController = null; modalBrowser?.destroy(); modalBrowser = null; C.destroyTree(content); content.replaceChildren(); if (controls) { C.destroyTree(controls); controls.replaceChildren(); } }
  function setScreen(value, heading) {
    cleanContent(); content.getAnimations?.().forEach(animation => animation.cancel()); screen = value; title.textContent = heading; dialog.dataset.screen = value;
    close.setAttribute("aria-label", "Close color rules");
  }
  function finishScreen() { scrollController?.destroy(); scrollController = UI.Scrollbar.attach(content); close.focus({ preventScroll: true }); }
  function typeScreen() {
    if (!active) return;
    setScreen("type", active.name);
    const saved = entry(active);
    const addRow = (name, override, remove) => {
      const wrapper = make("div", "calendar-rule-row");
      const caption = make("span", "calendar-rule-name", name);
      const pickerShell = C.PropertyPickerShell({ label: "Use Calendar Color", showValue: true, accessibleLabel: name + " color property", menuLabel: name + " color property" });
      const root = pickerShell.element, trigger = pickerShell.trigger, menu = pickerShell.menu;
      root.classList.add("calendar-property-picker"); menu.classList.add("calendar-property-menu");
      dialog.append(menu);
      const nameGroup = make("div", "calendar-rule-name-group"); nameGroup.append(caption);
      if (remove) {
        const removeButton = C.IconButton({ accessibleLabel: "Remove override", onClick: remove, attributes: { class: "neutral-icon-button calendar-remove", title: "Remove override" }, content: [C.DeleteIcon()] }).element;
        nameGroup.append(removeButton);
      }
      wrapper.append(nameGroup, root);
      content.append(wrapper);
      const options = [{ value: "inherit", label: override ? "Use type default" : "Use Calendar Color" }, { value: "none", label: "No color" }];
      const properties = (active.properties || []).filter(p => ["select", "checkbox"].includes(p.format));
      const counts = new Map(); properties.forEach(p => counts.set(p.name, (counts.get(p.name) || 0) + 1));
      properties.sort((a,b) => a.name.localeCompare(b.name) || a.key.localeCompare(b.key)).forEach(p => options.push({ value: "property:" + p.key, label: p.name,
        description: counts.get(p.name) > 1 ? p.key : undefined }));
      const current = override?.rule || entry(active)?.default || { mode: "inherit" };
      const value = current.mode === "property" ? "property:" + current.key : current.mode;
      if (current.mode === "property" && !properties.some(p => p.key === current.key)) options.push({ value, label: "Unavailable property", description: current.key });
      const picker = C.Picker({ root, menu, options, value, onChange(value) {
        const rule = value.startsWith("property:") ? { mode: "property", key: value.slice(9) } : { mode: value };
        if (override) override.rule = rule; else entry(active, true).default = rule;
        changed();
      } });
      const place = () => {
        const rect = trigger.getBoundingClientRect(), bounds = dialog.getBoundingClientRect();
        const width = menu.offsetWidth || 150;
        menu.style.left = Math.max(8, Math.min(rect.left - bounds.left, bounds.width - width - 8)) + "px";
        menu.style.top = rect.bottom - bounds.top + 6 + "px";
        menu.style.maxHeight = Math.max(120, window.innerHeight - rect.bottom - 24) + "px";
      };
      trigger.addEventListener("pointerdown", place); trigger.addEventListener("keydown", place); trigger.addEventListener("click", place);
      const dismiss = () => picker.close(); content.addEventListener("scroll", dismiss);
      pickers.push({ destroy() { content.removeEventListener("scroll", dismiss); picker.destroy(); }, close: () => picker.close() });
    };
    addRow("Default", null);
    for (const override of saved?.overrides || []) {
      const view = UI.ViewCatalog.findViewForTarget(catalog.views, override.target);
      addRow(view ? view.name : "Unavailable calendar", override,
        () => { saved.overrides = saved.overrides.filter(item => item !== override); changed(); typeScreen(); });
    }
    const add = button("Add override", calendarsScreen); add.disabled = locked || loading || !discovered; content.append(add);
    if (!(active.properties || []).length) content.append(make("p", "input-hint", "This type has no available single-select or checkbox properties. Multi-select properties are not supported."));
    finishScreen();
  }
  function calendarsScreen() {
    setScreen("calendars", "Choose calendar view");
    const toolbar = UI.CatalogBrowser.toolbar("calendar-override", "Search calendar views"); controls.append(toolbar); toolbar.classList.add("calendar-sticky-toolbar");
    const rows = [], saved = entry(active);
    const views = catalog.views.filter(view => view.target.spaceId === active.spaceId
      && !(saved?.overrides || []).some(item => targetKey(item.target) === targetKey(view.target)));
    const results = make("div", "calendar-view-results");
    views.sort((a,b) => a.name.localeCompare(b.name) || a.source.localeCompare(b.source)).forEach(view => {
      const result = UI.CatalogBrowser.row({ name: view.name, metadata: view.source + " · " + view.space, onSelect() {
        if (locked) return;
        entry(active, true).overrides.push({ target: copy(view.target), rule: { mode: "inherit" } }); changed(); modal.close();
      } });
      rows.push({ ...result, view }); results.append(result.item);
    });
    if (!rows.length) results.append(make("p", "input-hint", "No more accessible calendar views in this space."));
    const empty = make("p", "input-hint"); empty.hidden = true; content.append(results, empty);
    modalBrowser = UI.CatalogBrowser.attach({ prefix: "calendar-override", toolbar, records: () => rows, empty, onReload: async () => { await discover(); if (screen === "calendars") calendarsScreen(); } });
    modalBrowser.refresh(); finishScreen();
  }
  function render() {
    if (!list) return;
    C.destroyTree(list); list.replaceChildren(); records = [];
    const types = catalog.types.slice();
    // A saved rule is unavailable only after a successful discovery proves it
    // missing. An empty catalog during startup or a failed request proves nothing.
    if (hasCatalog) for (const saved of state.types) if (!types.some(type => key(type) === key(saved))) types.push({ spaceId: saved.spaceId, key: saved.typeKey, name: "Unavailable type", space: catalog.types.find(type => type.spaceId === saved.spaceId)?.space || saved.spaceId, properties: [], unavailable: true });
    types.sort((a,b) => a.name.localeCompare(b.name) || a.space.localeCompare(b.space) || a.key.localeCompare(b.key));
    for (const type of types) {
      const row = UI.CatalogBrowser.row({ name: type.name, metadata: type.space,
        warning: type.unavailable ? "Saved rules are retained until you remove them." : "",
        onSelect() { if (locked || loading) return; active = type; activeRow = row.row; typeScreen(); modal.open(); finishScreen(); } });
      if (type.unavailable) row.item.append(button("Remove saved rules", () => { state.types = state.types.filter(saved => key(saved) !== key(type)); changed(); render(); }));
      records.push({ ...row, view: { name: type.name, space: type.space, target: { spaceId: type.spaceId } } }); list.append(row.item);
    }
    if (!types.length) UI.CatalogBrowser.empty(list, { loading: !loadFailed && !discoveryError && (loading || discovering || !hasCatalog), loadingLabel: "Loading object types…", emptyLabel: loadFailed ? "Calendar rules could not be loaded." : discoveryError ? "Could not load object types. Reload to retry." : "No accessible object types found." });
    browser.refresh(); syncLocks();
  }
  function syncLocks() {
    browser?.setLocked(locked || loading || loadFailed, discovering);
    const open = doc.getElementById("open-calendar-colors");
    if (open) open.disabled = locked || loading || loadFailed;
    page?.querySelectorAll(".formula-view-row,.formula-view .formula-button").forEach(button => { button.disabled = locked || loading || loadFailed; });
    if (content) content.querySelectorAll("button,input").forEach(button => { button.disabled = locked || loading || loadFailed; });
  }
  async function discover() {
    if (!invoke || discovering || locked || loading) return;
    const request = ++generation; discovering = true; discoveryError = false; if (!hasCatalog) render(); syncLocks();
    try {
      const result = await invoke("discover_calendar_colors");
      if (request !== generation) return;
      if (!result || !Array.isArray(result.types) || !Array.isArray(result.views)) throw Error("No calendar catalog was returned.");
      catalog = result; discovered = true; hasCatalog = true; message.textContent = "";
      if (active) active = catalog.types.find(type => key(type) === key(active)) || { ...active, properties: [], unavailable: true };
      (result.info || []).forEach(text => api.onInfo?.(text));
      if (result.info?.length) message.textContent = "Some metadata could not be read. Check Logs and reload to retry.";
    } catch (error) { if (request === generation) { discoveryError = true; UI.CatalogBrowser.status(message, String(error.message || error) + " Your saved rules are kept."); } }
    finally { if (request === generation) { discovering = false; render(); } }
  }
  UI.CalendarPage = {
    init(options) {
      api = options; page = doc.getElementById("calendar-colors-page");
      const toolbar = UI.CatalogBrowser.toolbar("calendar", "Search object types");
      message = make("p", "field-error"); message.setAttribute("role", "status"); message.setAttribute("aria-live", "polite"); message.id = "calendar-rules-message";
      list = make("div"); list.id = "calendar-type-list"; empty = make("p", "input-hint"); empty.hidden = true;
      page.append(toolbar, message, list, empty);
      browser = UI.CatalogBrowser.attach({ prefix: "calendar", toolbar, records: () => records, empty, onReload: discover });
      title = make("h2"); title.id = "calendar-rule-title";
      close = button("×", () => modal.close(), "neutral-icon-button calendar-modal-close");
      close.setAttribute("aria-label", "Close color rules");
      controls = make("div", "calendar-modal-controls");
      content = make("div", "calendar-modal-content");
      const header = make("div", "calendar-modal-header"); header.append(title, close);
      shell = C.DialogShell({ attributes: { id: "calendar-rule-backdrop" }, dialogAttributes: { id: "calendar-rule-dialog", class: "confirm-dialog calendar-rule-dialog", "aria-labelledby": title.id }, content: [header, controls, content] });
      doc.body.append(shell.element); dialog = shell.dialog;
      modal = shell.attach({ initialFocus: () => close, fallbackFocus: () => activeRow?.isConnected ? activeRow : doc.getElementById("open-calendar-colors"),
        inertTargets: [doc.getElementById("settings-shell"), doc.getElementById("title-drag-region")].filter(Boolean),
        onClose() {
          if (screen === "calendars" && active) { typeScreen(); modal.open(); return; }
          cleanContent(); active = null; screen = "type";
        } });
      doc.getElementById("open-calendar-colors").addEventListener("click", () => { api.navigate("calendar-colors"); doc.getElementById("page-title")?.focus({ preventScroll: true }); if (!discovered) void discover(); });
      UI.AppReload?.register(() => !page.hidden ? discover() : undefined);
      render();
    },
    read: () => copy(state),
    restore(value) { state = copy(value || defaults()); if (modal?.isOpen()) { screen = "type"; modal.close({ restoreFocus: false }); } render(); },
    async load(bridge) {
      invoke = bridge; generation++; discovering = false; discovered = false; hasCatalog = false; discoveryError = false; catalog = { types: [], views: [] }; loading = true; loadFailed = false;
      if (modal?.isOpen()) { screen = "type"; modal.close({ restoreFocus: false }); }
      try {
        const result = await bridge("load_calendar_rules");
          if (result && (result.version !== 1 || !Array.isArray(result.types))) throw Error("Unsupported calendar rules.");
        state = copy(result || defaults()); message.textContent = "";
      } catch (error) { loadFailed = true; message.textContent = "Could not load calendar rules. Reload Settings to retry. " + String(error.message || error); }
      loading = false; render(); return !loadFailed;
    },
    async save(bridge, configuration, settings) {
      const result = await bridge("save_calendar_rules", { configuration, settings });
      UI.CatalogBrowser.status(message, "Calendar rules saved. Press Ctrl+R in Anytype to apply.", true);
      (result?.info || []).forEach(text => api.onInfo?.(text)); return result;
    },
    setLocked(value) {
      const wasLocked = locked; locked = value; syncLocks();
      if (wasLocked && !locked && !loading && !loadFailed && !discovered && page && !page.hidden) void discover();
    },
    refresh() { if (!discovered) return discover(); },
    reload() { generation++; discovering = false; discovered = false; render(); if (!page.hidden) return discover(); }
  };
})(window);
