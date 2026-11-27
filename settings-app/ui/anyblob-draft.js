export function createAnyblobDraft() {
  let api, baseline = null, pending = null;
  let snapshot = { loaded: false, locked: true, enabled: false, agentsGuide: false };
  const listeners = new Set();
  const values = () => ({ enabled: snapshot.enabled, agentsGuide: snapshot.agentsGuide });
  const publish = patch => { snapshot = { ...snapshot, ...patch }; listeners.forEach(listener => listener()); };
  const dirty = () => baseline !== null && JSON.stringify(values()) !== JSON.stringify(baseline);
  const commit = patch => { baseline = { ...baseline, ...patch }; publish(patch); api?.onChange(); };
  const controller = {
    subscribe(listener) { listeners.add(listener); return () => listeners.delete(listener); },
    getSnapshot: () => snapshot,
    init(options) { api = options; },
    isDirty: dirty,
    commit(patch) { if (baseline) commit(patch); },
    setLocked(locked) { if (snapshot.locked !== locked) publish({ locked }); },
    change(patch) {
      if (!snapshot.loaded || snapshot.locked) return;
      publish(patch);
      api?.onChange();
    },
    discard() { if (baseline) { publish(baseline); api?.onChange(); } },
    async load() {
      if (pending) return pending;
      pending = (async () => {
        const integration = await api.invoke("anyblob_status");
        if (!baseline || !dirty()) {
          baseline = { enabled: integration.enabled, agentsGuide: integration.agentsGuide === true };
          publish({ ...baseline, loaded: true });
        }
        return integration;
      })();
      try { return await pending; } finally { pending = null; }
    },
    async save() {
      if (!baseline) return;
      const next = values();
      if (next.enabled !== baseline.enabled) {
        await api.invoke("configure_anyblob", { enabled: next.enabled });
        commit({ enabled: next.enabled });
      }
      if (next.agentsGuide !== baseline.agentsGuide) {
        await api.invoke("configure_anyblob_agents_guide", { agentsGuide: next.agentsGuide });
        commit({ agentsGuide: next.agentsGuide });
      }
    },
  };
  return controller;
}
