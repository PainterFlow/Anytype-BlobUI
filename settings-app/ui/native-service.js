const METADATA_COMMANDS = new Set([
  "discover_kanban_views",
  "discover_calendar_colors",
  "discover_codex_spaces",
  "discover_anyblob_types",
  "discover_codex_sources",
]);

// Wraps the Tauri bridge without depending on Settings DOM or page state.
// Callbacks let the application keep its existing logging and connection UI.
export function createNativeService({ tauri, onMetadataError = null } = {}) {
  const pendingMetadata = new Set();
  const metadataRequests = new Map();
  let metadataEpoch = 0;
  let disposed = false;

  function invoke(name, args) {
    if (disposed) return Promise.reject(new Error("The Settings native service is closed."));
    const api = tauri && tauri.core;
    if (!api || typeof api.invoke !== "function") {
      return Promise.reject(new Error("The settings app bridge is unavailable. Launch BlobUI Settings using blobui-settings.exe."));
    }
    if (name === "configure_connection") metadataEpoch++;
    const epoch = metadataEpoch;
    const params = args || {};
    const metadata = METADATA_COMMANDS.has(name);
    const requestKey = JSON.stringify([metadataEpoch, name, params]);
    if (metadata && metadataRequests.has(requestKey)) return metadataRequests.get(requestKey);

    let nativeResult;
    try { nativeResult = api.invoke(name, params); }
    catch (error) { nativeResult = Promise.reject(error); }
    let request = Promise.resolve(nativeResult).then(result => {
      if (disposed) throw new Error("The Settings native service is closed.");
      if (metadata && epoch !== metadataEpoch) throw new Error("The saved Anytype connection changed. Refresh metadata again.");
      if (metadata && Array.isArray(result?.errors) && typeof onMetadataError === "function") {
        result.errors.forEach(error => onMetadataError(String(error.error || error)));
      }
      return result;
    });
    if (metadata) {
      request = request.catch(error => {
        if (error !== null && (typeof error === "object" || typeof error === "function")) throw error;
        throw new Error(String(error));
      });
      pendingMetadata.add(request);
      metadataRequests.set(requestKey, request);
      request.then(
        () => { pendingMetadata.delete(request); metadataRequests.delete(requestKey); },
        error => {
          pendingMetadata.delete(request);
          metadataRequests.delete(requestKey);
          if (!disposed && typeof onMetadataError === "function") onMetadataError(String(error.message || error), error);
        },
      );
    }
    return request;
  }

  function listen(name, handler) {
    if (disposed) return Promise.resolve(() => {});
    const api = tauri && tauri.event;
    if (!api || typeof api.listen !== "function") return Promise.resolve(() => {});
    return api.listen(name, event => {
      if (!disposed) return handler(event);
    });
  }

  return Object.freeze({
    invoke,
    listen,
    dispose() {
      disposed = true;
      pendingMetadata.clear();
      metadataRequests.clear();
    },
    get metadataPending() { return pendingMetadata; },
    get metadataEpoch() { return metadataEpoch; },
  });
}
