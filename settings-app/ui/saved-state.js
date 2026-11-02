(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {};
  const copy = value => value === undefined ? undefined : JSON.parse(JSON.stringify(value));
  const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const target = t => JSON.stringify([t.spaceId, t.objectId, t.blockId, t.viewId]);
  const type = t => JSON.stringify([t.spaceId, t.typeKey]);
  function merge(base, local, saved, warn, path = "Settings") {
    if (equal(local, base)) return copy(saved);
    if (equal(saved, base) || equal(local, saved)) return copy(local);
    if (base === undefined || local === undefined || saved === undefined) {
      warn(path); return copy(saved);
    }
    const key = path === "Calculations.definitions" ? item => target(item.target)
      : path === "Calendar.types" ? type : path.endsWith(".overrides") ? item => target(item.target) : null;
    if (key && Array.isArray(base) && Array.isArray(local) && Array.isArray(saved)) {
      const maps = [base, local, saved].map(items => new Map(items.map(item => [key(item), item])));
      return [...new Set([...maps[2].keys(), ...maps[1].keys(), ...maps[0].keys()])].sort().flatMap(id => {
        let before = maps[0].get(id);
        // Two new calendar entries can still contain independent rules.
        if (!before && maps[1].has(id) && maps[2].has(id) && path === "Calendar.types") {
          const item = maps[2].get(id);
          before = { spaceId: item.spaceId, typeKey: item.typeKey, default: { mode: "inherit" }, overrides: [] };
        }
        const result = merge(before, maps[1].get(id), maps[2].get(id), warn, path + "[" + id + "]");
        return result === undefined ? [] : [result];
      });
    }
    if (path.endsWith(".tokens") || path.endsWith(".default") || path.endsWith(".rule") || Array.isArray(local) || Array.isArray(saved) || typeof local !== "object" || typeof saved !== "object" || !local || !saved) {
      warn(path); return copy(saved);
    }
    const result = {};
    for (const key of new Set([...Object.keys(base || {}), ...Object.keys(local), ...Object.keys(saved)])) {
      const value = merge(base?.[key], local[key], saved[key], warn, path + "." + key);
      if (value !== undefined) result[key] = value;
    }
    return result;
  }
  UI.SavedState = { merge, equal, target, copy };
})(window);
