(function (root) {
  "use strict";
  const UI = root.BlobUI, doc = root.document, C = UI.Components;
  const copy = value => JSON.parse(JSON.stringify(value));
  const defaults = () => ({ version: 1, types: [] });
  const key = type => JSON.stringify([type.spaceId, type.key || type.typeKey]);
  const targetKey = target => UI.ViewCatalog.targetIdentity(target);
  const make = UI.CatalogBrowser.node;
  let state = defaults(), catalog = { types: [], views: [] }, invoke, api, loading = true, locked = false;
  let browser, modalBrowser, modal, shell, dialog, content, title, close, message, empty, list, page;
  let records = [], active, activeRow, screen = "type", picker, generation = 0, discovering = false, discovered = false, loadFailed = false;
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
  function cleanContent() { picker?.destroy(); picker = null; modalBrowser?.destroy(); modalBrowser = null; C.destroyTree(content); content.replaceChildren(); }
  function setScreen(value, heading) {
    cleanContent(); screen = value; title.textContent = heading;
    close.setAttribute("aria-label", value === "type" ? "Close color rules" : "Back to type rules");
  }
  function finishScreen() { (content.querySelector("button:not(:disabled),input:not(:disabled)") || close).focus({ preventScroll: true }); }
  function typeScreen() {
    if (!active) return;
    setScreen("type", active.name);
    const saved = entry(active);
    const addRow = (name, description, action, remove) => {
      const wrapper = make("div", "calendar-rule-row");
      const row = UI.CatalogBrowser.row({ name, metadata: description, onSelect: action }); wrapper.append(row.item);
      if (remove) wrapper.append(button("Remove override", remove, "formula-button calendar-remove"));
      content.append(wrapper);
    };
    addRow("Default", label(saved?.default, active), () => propertyScreen(null));
    for (const override of saved?.overrides || []) {
      const view = UI.ViewCatalog.findViewForTarget(catalog.views, override.target);
      addRow(view ? view.name : "Unavailable calendar", (view ? view.source + " · " : "") + (override.rule.mode === "inherit" ? "Use type default" : label(override.rule, active)),
        () => propertyScreen(override), () => { saved.overrides = saved.overrides.filter(item => item !== override); changed(); typeScreen(); });
    }
    const add = button("Add override", calendarsScreen); add.disabled = locked || loading || !discovered; content.append(add);
    if (!(active.properties || []).length) content.append(make("p", "input-hint", "This type has no available single-select or checkbox properties. Multi-select properties are not supported."));
    finishScreen();
  }
  function propertyScreen(override) {
    setScreen("property", override ? "Calendar color property" : "Default color property");
    content.append(button("Back", typeScreen));
    const root = make("div", "kanban-select calendar-property-picker");
    const trigger = button("Choose property", null, "kanban-select-trigger");
    trigger.setAttribute("aria-haspopup", "listbox"); trigger.setAttribute("aria-expanded", "false");
    const text = make("span"); text.dataset.pickValue = ""; trigger.replaceChildren(text);
    const menu = make("div", "kanban-select-menu"); menu.setAttribute("role", "listbox"); menu.setAttribute("aria-label", "Color property"); menu.hidden = true;
    root.append(trigger, menu); content.append(root);
    const options = [{ value: "inherit", label: override ? "Use type default" : "Use Calendar Color" }, { value: "none", label: "No color" }];
    const properties = (active.properties || []).filter(p => ["select", "checkbox"].includes(p.format));
    const counts = new Map(); properties.forEach(p => counts.set(p.name, (counts.get(p.name) || 0) + 1));
    properties.sort((a,b) => a.name.localeCompare(b.name) || a.key.localeCompare(b.key)).forEach(p => options.push({ value: "property:" + p.key, label: p.name,
      description: (p.format === "checkbox" ? "Checkbox" : "Single-select") + (counts.get(p.name) > 1 ? " · " + p.key : "") }));
    const current = override?.rule || entry(active)?.default || { mode: "inherit" };
    const value = current.mode === "property" ? "property:" + current.key : current.mode;
    if (current.mode === "property" && !properties.some(p => p.key === current.key)) options.push({ value, label: "Unavailable property", description: current.key });
    picker = C.Picker({ root, options, value, onChange(value) {
      const rule = value.startsWith("property:") ? { mode: "property", key: value.slice(9) } : { mode: value };
      if (override) override.rule = rule; else entry(active, true).default = rule;
      changed(); typeScreen();
    } });
    content.append(make("p", "input-hint", "Select uses Anytype's option color. Checkbox uses green when checked and gray when unchecked."));
    picker.open();
  }
  function calendarsScreen() {
    setScreen("calendars", "Choose calendar view");
    content.append(button("Back", typeScreen));
    const toolbar = UI.CatalogBrowser.toolbar("calendar-override", "Search calendar views"); content.append(toolbar);
    const rows = [], saved = entry(active);
    const views = catalog.views.filter(view => view.target.spaceId === active.spaceId
      && !(saved?.overrides || []).some(item => targetKey(item.target) === targetKey(view.target)));
    const results = make("div", "calendar-view-results");
    views.sort((a,b) => a.name.localeCompare(b.name) || a.source.localeCompare(b.source)).forEach(view => {
      const result = UI.CatalogBrowser.row({ name: view.name, metadata: view.source + " · " + view.space, onSelect() {
        if (locked) return;
        entry(active, true).overrides.push({ target: copy(view.target), rule: { mode: "inherit" } }); changed(); typeScreen();
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
    for (const saved of state.types) if (!types.some(type => key(type) === key(saved))) types.push({ spaceId: saved.spaceId, key: saved.typeKey, name: "Unavailable type", space: saved.spaceId, properties: [], unavailable: true });
    types.sort((a,b) => a.name.localeCompare(b.name) || a.space.localeCompare(b.space) || a.key.localeCompare(b.key));
    for (const type of types) {
      const row = UI.CatalogBrowser.row({ name: type.name, metadata: type.space,
        warning: type.unavailable ? "Saved rules are retained until you remove them." : "",
        onSelect() { if (locked || loading) return; active = type; activeRow = row.row; typeScreen(); modal.open(); finishScreen(); } });
      if (type.unavailable) row.item.append(button("Remove saved rules", () => { state.types = state.types.filter(saved => key(saved) !== key(type)); changed(); render(); }));
      records.push({ ...row, view: { name: type.name, space: type.space, target: { spaceId: type.spaceId } } }); list.append(row.item);
    }
    if (!types.length) UI.CatalogBrowser.empty(list, { loading: loading || discovering, loadingLabel: "Loading object types…", emptyLabel: "No accessible object types found." });
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
    const request = ++generation; discovering = true; syncLocks();
    try {
      const result = await invoke("discover_calendar_colors");
      if (request !== generation) return;
      if (!result || !Array.isArray(result.types) || !Array.isArray(result.views)) throw Error("No calendar catalog was returned.");
      catalog = result; discovered = true; message.textContent = "";
      if (active) active = catalog.types.find(type => key(type) === key(active)) || { ...active, properties: [], unavailable: true };
      (result.info || []).forEach(text => api.onInfo?.(text));
      if (result.info?.length) message.textContent = "Some metadata could not be read. Check Logs and reload to retry.";
    } catch (error) { if (request === generation) UI.CatalogBrowser.status(message, String(error.message || error) + " Your saved rules are kept."); }
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
      close = button("×", () => screen === "type" ? modal.close() : typeScreen(), "neutral-icon-button calendar-modal-close");
      close.setAttribute("aria-label", "Close color rules");
      content = make("div", "calendar-modal-content");
      const header = make("div", "calendar-modal-header"); header.append(title, close);
      shell = C.DialogShell({ attributes: { id: "calendar-rule-backdrop" }, dialogAttributes: { id: "calendar-rule-dialog", class: "confirm-dialog calendar-rule-dialog", "aria-labelledby": title.id }, content: [header, content] });
      doc.body.append(shell.element); dialog = shell.dialog;
      modal = shell.attach({ initialFocus: () => close, fallbackFocus: () => activeRow?.isConnected ? activeRow : doc.getElementById("open-calendar-colors"),
        inertTargets: [doc.getElementById("settings-shell"), doc.getElementById("title-drag-region")].filter(Boolean),
        onClose() { cleanContent(); active = null; screen = "type"; } });
      // Escape from a sub-dialog returns to the type's rules without losing edits.
      doc.addEventListener("keydown", event => {
        if (event.key === "Escape" && modal.isOpen() && screen !== "type" && !event.defaultPrevented) {
          event.preventDefault(); event.stopImmediatePropagation(); typeScreen();
        }
      }, true);
      shell.element.addEventListener("pointerdown", event => {
        if (event.target === shell.element && screen !== "type") {
          event.preventDefault(); event.stopImmediatePropagation(); typeScreen();
        }
      }, true);
      doc.getElementById("open-calendar-colors").addEventListener("click", () => { api.navigate("calendar-colors"); doc.getElementById("page-title")?.focus({ preventScroll: true }); if (!discovered) void discover(); });
      UI.AppReload?.register(() => !page.hidden ? discover() : undefined);
      render();
    },
    read: () => copy(state),
    restore(value) { state = copy(value || defaults()); if (modal?.isOpen()) modal.close({ restoreFocus: false }); render(); },
    async load(bridge) {
      invoke = bridge; generation++; discovering = false; discovered = false; catalog = { types: [], views: [] }; loading = true; loadFailed = false;
      if (modal?.isOpen()) modal.close({ restoreFocus: false });
      try {
        const result = await bridge("load_calendar_rules");
          if (result && (result.version !== 1 || !Array.isArray(result.types))) throw Error("Unsupported calendar rules.");
        state = copy(result || defaults()); message.textContent = "";
      } catch (error) { loadFailed = true; message.textContent = "Could not load calendar rules. Reload Settings to retry. " + String(error.message || error); }
      loading = false; render(); return !loadFailed;
    },
    async save(bridge, configuration) {
      const result = await bridge("save_calendar_rules", { configuration });
      UI.CatalogBrowser.status(message, "Calendar rules saved. Press Ctrl+R in Anytype to apply.", true);
      (result?.info || []).forEach(text => api.onInfo?.(text)); return result;
    },
    setLocked(value) {
      const wasLocked = locked; locked = value; syncLocks();
      if (wasLocked && !locked && !loading && !loadFailed && !discovered && page && !page.hidden) void discover();
    },
    refresh() { if (!discovered) return discover(); },
    reload() { generation++; discovering = false; discovered = false; catalog = { types: [], views: [] }; render(); if (!page.hidden) return discover(); }
  };
})(window);
