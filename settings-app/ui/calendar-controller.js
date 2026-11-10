const copy = value => JSON.parse(JSON.stringify(value));
const defaults = () => ({ version: 1, types: [] });
const key = type => JSON.stringify([type.spaceId, type.key || type.typeKey]);
const targetKey = target => window.BlobUI.ViewCatalog.targetIdentity(target);

export function createCalendarController() {
  let state = defaults();
  let catalog = { types: [], views: [] };
  let api = {}, invoke, revision = null, loading = true, loadFailed = false;
  let locked = false, discovering = false, discovered = false, hasCatalog = false;
  let discoveryError = false, discoveryRequested = false, discoveryForceRequested = false;
  let generation = 0, status = "", modal = false;
  let screen = "type", active = null, shown = false, loadEpoch = 0, disposed = false, unregisterReload;
  const listeners = new Set();
  let snapshot;

  function publish() {
    snapshot = { state, catalog, loading, loadFailed, locked, discovering, discovered,
      hasCatalog, discoveryError, status, modal, screen, active };
    listeners.forEach(listener => listener());
  }
  function entry(type, create = false) {
    let value = state.types.find(item => key(item) === key(type));
    if (!value && create) {
      value = { spaceId: type.spaceId, typeKey: type.key, default: { mode: "inherit" }, overrides: [] };
      state.types.push(value);
    }
    return value;
  }
  function changed() { api.onChange?.(); publish(); }
  function close() { modal = false; active = null; screen = "type"; publish(); }
  function open(type) { if (locked || loading || loadFailed) return; active = type; screen = "type"; modal = true; publish(); }
  function setRule(override, value) {
    const rule = value.startsWith("property:") ? { mode: "property", key: value.slice(9) } : { mode: value };
    if (override) override.rule = rule;
    else entry(active, true).default = rule;
    changed();
  }
  function options(override) {
    const properties = (active?.properties || []).filter(property => ["select", "multi_select", "checkbox"].includes(property.format))
      .sort((a, b) => a.name.localeCompare(b.name) || a.key.localeCompare(b.key));
    const counts = new Map(); properties.forEach(property => counts.set(property.name, (counts.get(property.name) || 0) + 1));
    const result = [{ value: "inherit", label: override ? "Use type default" : "Use Calendar Color" }, { value: "none", label: "No color" }];
    properties.forEach(property => result.push({ value: "property:" + property.key, label: property.name,
      description: [counts.get(property.name) > 1 ? property.key : "", property.format === "multi_select" ? "Uses the first selected tag's color" : ""].filter(Boolean).join(" · ") || undefined }));
    const current = override?.rule || entry(active)?.default || { mode: "inherit" };
    const value = current.mode === "property" ? "property:" + current.key : current.mode;
    if (current.mode === "property" && !properties.some(property => property.key === current.key)) result.push({ value, label: "Unavailable property", description: current.key });
    return { options: result, value };
  }
  async function discover(forceRefresh = false) {
    if (!invoke || disposed) return;
    if (loading || locked) {
      discoveryRequested = true;
      discoveryForceRequested ||= forceRefresh;
      return;
    }
    if (discovering) return;
    const request = ++generation;
    forceRefresh ||= discoveryForceRequested;
    discoveryRequested = false;
    discoveryForceRequested = false;
    discovering = true; discoveryError = false; publish();
    try {
      const result = await invoke("discover_calendar_colors", { forceRefresh });
      if (request !== generation) return;
      if (!result || !Array.isArray(result.types) || !Array.isArray(result.views)) throw Error("No calendar catalog was returned.");
      catalog = result; discovered = true; hasCatalog = true; status = "";
      if (active) active = catalog.types.find(type => key(type) === key(active)) || { ...active, properties: [], unavailable: true };
      (result.info || []).forEach(text => api.onInfo?.(text));
      if (result.info?.length) status = "Some metadata could not be read. Check Logs and reload to retry.";
    } catch (error) {
      if (request === generation) {
        discoveryError = true;
        const detail = String(error.message || error);
        api.onInfo?.(detail); status = detail + " Your saved rules are kept.";
      }
    } finally { if (request === generation) { discovering = false; publish(); } }
  }
  const controller = {
    subscribe(listener) { listeners.add(listener); return () => listeners.delete(listener); },
    getSnapshot() { return snapshot; },
    init(options) {
      api = options;
      unregisterReload = api.registerReload?.(() => discover(true));
      publish();
    },
    destroy() {
      disposed = true;
      generation++;
      loadEpoch++;
      unregisterReload?.();
    },
    read: () => copy(state),
    validateSaved(result) { if (!result || result.version !== 1 || !Array.isArray(result.types)) throw Error("Unsupported saved calendar rules."); },
    reconcile(value) {
      const previous = state; state = copy(value); loadFailed = false;
      if (modal && active) {
        const id = key(active), before = previous.types.find(type => key(type) === id), after = state.types.find(type => key(type) === id);
        if (!window.BlobUI.SavedState.equal(before, after)) {
          if (before && !after) close();
          else if (screen === "type") publish();
        }
      }
      publish();
    },
    acceptSaved(result) { revision = result.revision || null; status = ""; publish(); },
    restore(value) { state = copy(value || defaults()); close(); publish(); },
    async load(bridge) {
      const epoch = ++loadEpoch;
      invoke = bridge; generation++; discovering = false; discovered = false; hasCatalog = false; discoveryError = false;
      catalog = { types: [], views: [] }; loading = true; loadFailed = false; close(); publish();
      try {
        const result = await bridge("load_calendar_rules");
        if (epoch !== loadEpoch) return false;
        if (result && (result.version !== 1 || !Array.isArray(result.types))) throw Error("Unsupported calendar rules.");
        revision = result?.revision || null; state = copy(result || defaults()); delete state.revision; status = "";
      } catch (error) {
        if (epoch !== loadEpoch) return false;
        loadFailed = true;
        status = "Could not load calendar rules. Reload Settings to retry. " + String(error.message || error);
      }
      loading = false; publish();
      if (!loadFailed && !locked && !discovered && (shown || discoveryRequested)) {
        void discover(discoveryForceRequested);
      }
      return !loadFailed;
    },
    async save(bridge, configuration, settings) {
      const result = await bridge("save_calendar_rules", { configuration, settings, revision });
      revision = result?.revision || revision;
      status = "Calendar rules saved. Press Ctrl+R in Anytype to apply.";
      (result?.info || []).forEach(text => api.onInfo?.(text)); publish(); return result;
    },
    setLocked(value) {
      if (locked === value) return;
      locked = value;
      publish();
      if (!locked && !loading && !loadFailed && !discovered && (shown || discoveryRequested)) void discover(discoveryForceRequested);
    },
    show(value) { shown = value; if (shown && !discovered) return discover(); },
    reload() { generation++; discovering = false; discovered = false; publish(); if (shown) return discover(); },
    discover,
    refresh() { api.onAttempt?.(); return discover(true); },
    getTypes() {
      const types = catalog.types.slice();
      if (hasCatalog) for (const saved of state.types) if (!types.some(type => key(type) === key(saved))) types.push({ spaceId: saved.spaceId,
        key: saved.typeKey, name: "Unavailable type", space: catalog.types.find(type => type.spaceId === saved.spaceId)?.space || saved.spaceId, properties: [], unavailable: true });
      return types.sort((a, b) => a.name.localeCompare(b.name) || a.space.localeCompare(b.space) || a.key.localeCompare(b.key));
    },
    open, close, setScreen(value) { screen = value; publish(); },
    setRule, options,
    removeType(type) { state.types = state.types.filter(saved => key(saved) !== key(type)); changed(); },
    removeOverride(override) { const saved = entry(active); if (saved) saved.overrides = saved.overrides.filter(item => item !== override); changed(); },
    addOverride(view) { entry(active, true).overrides.push({ target: copy(view.target), rule: { mode: "inherit" } }); screen = "type"; changed(); },
    views() { const saved = entry(active); return catalog.views.filter(view => view.target.spaceId === active?.spaceId && !(saved?.overrides || []).some(item => targetKey(item.target) === targetKey(view.target)))
      .sort((a, b) => a.name.localeCompare(b.name) || a.source.localeCompare(b.source)); },
    viewFor(target) { return window.BlobUI.ViewCatalog.findViewForTarget(catalog.views, target); },
  };
  publish();
  return controller;
}
