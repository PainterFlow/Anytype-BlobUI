(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  const severity = { info: 0, warning: 1, error: 2 };

  function isConnectionFailure(message) {
    const text = String(message).toLowerCase();
    return /anytype(?:'s)? local api/.test(text)
      && /(could not reach|timed out|timeout|unavailable)/.test(text);
  }

  function normalizeLevel(message, kind) {
    if (isConnectionFailure(message)) return "warning";
    return kind === "error" || kind === "warning" ? kind : "info";
  }

  UI.LogStore = Object.freeze({
    isConnectionFailure,
    create() {
      const entries = [];
      let reportedErrors = new WeakMap();

      function add(message, kind, occurrence, time = new Date().toLocaleTimeString()) {
        const text = String(message);
        const level = normalizeLevel(text, kind);
        const hasOccurrence = occurrence !== null
          && (typeof occurrence === "object" || typeof occurrence === "function");
        const previous = hasOccurrence ? reportedErrors.get(occurrence) : null;
        if (previous) {
          if (severity[level] > severity[previous.level]) {
            previous.level = level;
            return true;
          }
          return false;
        }
        const entry = { message: text, level, time };
        entries.push(entry);
        if (hasOccurrence) reportedErrors.set(occurrence, entry);
        return true;
      }

      function getSeverity() {
        if (entries.some(entry => entry.level === "error")) return "error";
        if (entries.some(entry => entry.level === "warning")) return "warning";
        return "info";
      }

      return Object.freeze({
        add,
        clear() { entries.length = 0; reportedErrors = new WeakMap(); },
        entries() { return entries.slice(); },
        severity: getSeverity,
      });
    },
  });
})(window);
