(function (root) {
  'use strict';
  const UI = root.BlobUI = root.BlobUI || {};
  const defaults = { version: 1, sidebarWidth: 280, sidebarCollapsed: false, fullscreen: false, uiScale: 1, windowWidth: 960, windowHeight: 740 };
  let state = { ...defaults }, invoke = null, writing = false, pendingPatch = {};
  function notify() { listeners.forEach(listener => listener()); }
  const listeners = new Set();
  function flush() {
    if (!invoke || writing || !Object.keys(pendingPatch).length) return;
    const patch = pendingPatch; pendingPatch = {};
    writing = true;
    invoke('update_low_level_settings', { patch }).then(saved => {
      state = { ...state, ...saved };
      notify();
    }).catch(() => {}).finally(() => {
      writing = false;
      flush();
    });
  }
  function update(patch) {
    state = { ...state, ...patch };
    pendingPatch = { ...pendingPatch, ...patch };
    notify();
    flush();
  }
  async function init(nativeInvoke) {
    invoke = nativeInvoke;
    try { state = { ...defaults, ...await invoke('get_low_level_settings') }; } catch (_) {}
    notify();
    return state;
  }
  UI.LowLevelSettings = {
    init, update, flush,
    snapshot: () => ({ ...state }),
    subscribe(listener) { listeners.add(listener); return () => listeners.delete(listener); },
  };
})(window);
