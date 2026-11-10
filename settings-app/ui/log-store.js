(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  const severity = { info: 0, warning: 1, error: 2 };

  function normalizeLevel(message, kind) {
    const text = String(message).toLowerCase();
    if (/anytype(?:'s)? local api/.test(text)
      && /(could not reach|timed out|timeout|unavailable)/.test(text)) return "warning";
    return kind === "error" || kind === "warning" ? kind : "info";
  }

  function messageKey(message) {
    return String(message).trim().replace(/\s+/g, " ").toLocaleLowerCase();
  }

  UI.LogStore = Object.freeze({
    create() {
      const entries = [];
      let currentAttempt = new Map();

      function add(message, kind, time = new Date().toLocaleTimeString()) {
        const text = String(message);
        const key = messageKey(text);
        const level = normalizeLevel(text, kind);
        const previous = currentAttempt.get(key);
        if (previous) {
          if (severity[level] > severity[previous.level]) {
            previous.level = level;
            return true;
          }
          return false;
        }
        const entry = { message: text, level, time };
        entries.push(entry);
        currentAttempt.set(key, entry);
        return true;
      }

      function getSeverity() {
        if (entries.some(entry => entry.level === "error")) return "error";
        if (entries.some(entry => entry.level === "warning")) return "warning";
        return "info";
      }

      return Object.freeze({
        add,
        beginAttempt() { currentAttempt = new Map(); },
        clear() { entries.length = 0; currentAttempt.clear(); },
        entries() { return entries.slice(); },
        severity: getSeverity,
      });
    },
  });
})(window);
