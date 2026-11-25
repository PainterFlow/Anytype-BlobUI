(function (root) {
  'use strict';
  const UI = root.BlobUI = root.BlobUI || {};
  let loaded = false, spaces = [], pending = null, generation = 0;
  function invalidate() {
    loaded = false;
    spaces = [];
    pending = null;
    generation++;
  }
  async function load(invoke, forceRefresh = false) {
    if (pending) return pending;
    if (loaded && !forceRefresh) return spaces.slice();
    if (forceRefresh) { loaded = false; spaces = []; }
    const ticket = generation;
    pending = Promise.resolve()
      .then(() => invoke('discover_codex_spaces', { forceRefresh }))
      .then(result => {
        if (!Array.isArray(result)) throw new Error('Space discovery returned an invalid response.');
        if (ticket === generation) {
          spaces = result;
          loaded = true;
        }
        return spaces.slice();
      })
      .finally(() => { pending = null; });
    return pending;
  }
  UI.AppReload?.register(() => invalidate());
  UI.SpaceCatalog = { load, invalidate, snapshot: () => spaces.slice() };
})(window);
