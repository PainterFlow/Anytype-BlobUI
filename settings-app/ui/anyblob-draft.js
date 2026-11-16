export function createAnyblobDraft() {
  let api, baseline = null, pending = null;
  let snapshot = { loaded: false, locked: true, enabled: false, folder: "", tunnelEnabled: false, tunnelId: "", runtimeApiKey: "" };
  const listeners = new Set();
  const values = () => ({ enabled: snapshot.enabled, folder: snapshot.folder, tunnelEnabled: snapshot.tunnelEnabled, tunnelId: snapshot.tunnelId, runtimeApiKey: snapshot.runtimeApiKey });
  const publish = patch => { snapshot = { ...snapshot, ...patch }; listeners.forEach(listener => listener()); };
  const dirty = () => baseline !== null && JSON.stringify(values()) !== JSON.stringify(baseline);
  const commit = patch => { baseline = { ...baseline, ...patch }; publish(patch); api?.onChange(); };
  const controller = {
    subscribe(listener) { listeners.add(listener); return () => listeners.delete(listener); },
    getSnapshot: () => snapshot,
    init(options) { api = options; },
    isDirty: dirty,
    setLocked(locked) { if (snapshot.locked !== locked) publish({ locked }); },
    change(patch) {
      if (!snapshot.loaded || snapshot.locked) return;
      publish({ ...patch, ...(patch.enabled === false ? { runtimeApiKey: "", tunnelEnabled: false } : {}) });
      api?.onChange();
    },
    discard() { if (baseline) { publish(baseline); api?.onChange(); } },
    async load() {
      if (pending) return pending;
      pending = (async () => {
        const [integration, tunnel] = await Promise.all([api.invoke("anyblob_status"), api.invoke("secure_mcp_tunnel_status")]);
        if (!baseline || !dirty()) {
          baseline = { enabled: integration.enabled, folder: integration.localProjectsFolder || "", tunnelEnabled: tunnel.enabled, tunnelId: tunnel.tunnelId || "", runtimeApiKey: "" };
          publish({ ...baseline, loaded: true });
        }
        return tunnel;
      })();
      try { return await pending; } finally { pending = null; }
    },
    async save({ startTunnel = false } = {}) {
      if (!baseline) return;
      const next = values();
      if (next.enabled !== baseline.enabled) {
        await api.invoke("configure_anyblob", { enabled: next.enabled });
        commit({ enabled: next.enabled, ...(!next.enabled ? { tunnelEnabled: false } : {}) });
        if (!next.enabled) next.tunnelEnabled = false;
      }
      if (next.folder !== baseline.folder) {
        const result = await api.invoke("set_anyblob_projects_folder", { folder: next.folder });
        commit({ folder: result.localProjectsFolder });
      }
      if (startTunnel || next.tunnelEnabled !== baseline.tunnelEnabled || next.tunnelId !== baseline.tunnelId || next.runtimeApiKey) {
        await api.invoke("configure_secure_mcp_tunnel", { enabled: next.tunnelEnabled, tunnelId: next.tunnelId.trim(), runtimeApiKey: next.runtimeApiKey.trim() || null, start: startTunnel });
        commit({ tunnelEnabled: next.tunnelEnabled, tunnelId: next.tunnelId.trim(), runtimeApiKey: "" });
      }
    },
    async stop() {
      await api.invoke("configure_secure_mcp_tunnel", { enabled: false, tunnelId: baseline.tunnelId, runtimeApiKey: null, start: false });
      commit({ tunnelEnabled: false });
    },
  };
  return controller;
}
