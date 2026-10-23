(function (root) {
  "use strict";

  const BlobUI = root.BlobUI = root.BlobUI || {};

  function targetIdentity(target) {
    if (!target || typeof target !== "object") return null;
    const parts = [target.spaceId, target.objectId, target.blockId, target.viewId];
    if (parts.some(value => typeof value !== "string" || !value)) return null;
    return JSON.stringify(parts);
  }

  function sourceViewIdentity(view) {
    const target = view && view.target;
    if (!targetIdentity(target)) return null;
    // The discovery API exposes the owning query/collection as target.objectId.
    // blockId can differ between an inline embed and the canonical list view.
    return JSON.stringify([target.spaceId, target.objectId, target.viewId]);
  }

  function canonicalRank(view) {
    if (!view || typeof view !== "object") return 3;
    if (view.kind === "Query" || view.kind === "Collection") return 0;
    if (typeof view.kind === "string" && view.kind.toLocaleLowerCase().includes("inline")) return 2;
    return 1;
  }

  function chooseRepresentative(group) {
    return group.reduce((best, candidate) => {
      const candidateRank = canonicalRank(candidate), bestRank = canonicalRank(best);
      if (candidateRank !== bestRank) return candidateRank < bestRank ? candidate : best;
      const candidateTarget = targetIdentity(candidate && candidate.target) || "";
      const bestTarget = targetIdentity(best && best.target) || "";
      return candidateTarget < bestTarget ? candidate : best;
    });
  }

  function cloneTarget(target) {
    return target && typeof target === "object" ? { ...target } : target;
  }

  function cloneView(view, aliasTargets) {
    const copy = { ...view, target: cloneTarget(view.target) };
    Object.defineProperty(copy, "aliasTargets", {
      value: Object.freeze(aliasTargets.map(cloneTarget)),
      enumerable: false,
      configurable: false,
      writable: false,
    });
    return copy;
  }

  function uniqueTargets(group, representative) {
    const ordered = [representative, ...group.filter(view => view !== representative)
      .sort((left, right) => (targetIdentity(left && left.target) || "").localeCompare(targetIdentity(right && right.target) || ""))];
    const seen = new Set();
    const targets = [];
    for (const view of ordered) {
      const key = targetIdentity(view.target);
      if (!key || seen.has(key)) continue;
      seen.add(key);
      targets.push(view.target);
    }
    return targets;
  }

  function dedupeViews(input) {
    const groups = [];
    const groupByIdentity = new Map();
    for (const view of Array.isArray(input) ? input : []) {
      const key = view && typeof view === "object" ? sourceViewIdentity(view) : null;
      if (!key) {
        groups.push([view]);
        continue;
      }
      let group = groupByIdentity.get(key);
      if (!group) {
        group = [];
        groupByIdentity.set(key, group);
        groups.push(group);
      }
      group.push(view);
    }

    const views = groups.map(group => {
      const representative = chooseRepresentative(group);
      return cloneView(representative, uniqueTargets(group, representative));
    });
    const spacesById = new Map();
    for (const view of Array.isArray(input) ? input : []) {
      const id = view && view.target && view.target.spaceId;
      if (typeof id !== "string" || !id) continue;
      const label = typeof view.space === "string" && view.space.trim() ? view.space : "Unnamed space";
      if (!spacesById.has(id) || (spacesById.get(id) === "Unnamed space" && label !== "Unnamed space")) {
        spacesById.set(id, label);
      }
    }
    return { views, spacesById };
  }

  function targetAliases(view) {
    return view && Array.isArray(view.aliasTargets) ? view.aliasTargets : view && view.target ? [view.target] : [];
  }

  function definitionsForView(view, definitions) {
    const identities = new Set(targetAliases(view).map(targetIdentity).filter(Boolean));
    return (Array.isArray(definitions) ? definitions : []).filter(definition =>
      definition && identities.has(targetIdentity(definition.target)));
  }

  function hasDefinition(view, definitions) {
    return definitionsForView(view, definitions).length > 0;
  }

  function findViewForTarget(views, target) {
    const key = targetIdentity(target);
    if (!key) return null;
    return (Array.isArray(views) ? views : []).find(view =>
      targetAliases(view).some(alias => targetIdentity(alias) === key)) || null;
  }

  BlobUI.ViewCatalog = Object.freeze({
    dedupeViews,
    targetIdentity,
    targetAliases,
    definitionsForView,
    hasDefinition,
    findViewForTarget,
  });
})(typeof window !== "undefined" ? window : globalThis);
