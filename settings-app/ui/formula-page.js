(function (root) {
  "use strict";
  let configurationRevision = null;
  const UI = root.BlobUI = root.BlobUI || {}, doc = root.document;
  const defaults = () => ({ version: 2, display: "balanced", definitions: [] });
  const copy = value => JSON.parse(JSON.stringify(value));
  const identity = target => JSON.stringify([target.spaceId, target.objectId, target.blockId, target.viewId]);
  const el = id => doc.getElementById(id);
  let contents, fields, search, reload, filterButton;
  let modalBackdrop, modalDialog, modalClose, modalContent;
  function syncDomRefs() {
    contents = el("formula-contents");
    fields = el("formula-fields"); search = el("formula-search"); reload = el("formula-reload");
    filterButton = el("formula-filter");
    modalBackdrop = el("formula-calculator-backdrop"); modalDialog = el("formula-calculator-dialog");
    modalClose = el("formula-calculator-close"); modalContent = el("formula-calculator-content");
  }
  function setMessage(text, success = false) {
    pageMessage = text;
    messageSuccess = success;
    if (text) loadError = success ? "" : text;
    publish();
  }
  const operations = { total: "Total", average: "Average", minimum: "Minimum", maximum: "Maximum", median: "Median" };
  let state = defaults(), persistedState = defaults(), catalog = [], api, invoke, calculatorModal;
  let locked = false, dirty = false, loading = true, discovering = false, discovered = false;
  let records = [], initialized = false, loadError = "", activeRecord = null, modalSaving = false, calculatedAt = "";
  let reloadController, unregisterReload;
  let pageMessage = "", messageSuccess = false;
  let discoveryRequested = false;
  let discoveryGeneration = 0;
  const viewSubscribers = new Set();
  let searchText = "", selectedSpace = "";
  function snapshot() {
    return {
      views: records.map(record => ({ ...record.view, key: record.key, active: !!activeRecord && activeRecord.viewKey === record.key })),
      loading, locked: locked || loading, discovering, message: pageMessage,
      success: messageSuccess, display: state.display, calculatedAt,
      active: activeRecord ? calculatorSnapshot(activeRecord) : null,
      searchText, selectedSpace,
    };
  }
  function publish() { const value = snapshot(); viewSubscribers.forEach(listener => listener(value)); }
  function calculatorSnapshot(record) {
    const tokens = serializedTokens(record);
    const unavailable = values => values.some(token => (["aggregate", "property"].includes(token.kind)
      && !(record.view.properties || []).some(property => property.key === token.property))
      || (token.calculation && unavailable(token.calculation)));
    const status = record.statusText || record.status?.textContent || (unavailable([...record.numerator, ...record.denominator])
      ? "A saved property is unavailable. Remove the calculation and choose a replacement." : "");
    return {
      view: record.view, sourceMode: !!record.sourceMode, source: record.source || "", sourceError: record.sourceError || "",
      numerator: copy(record.numerator || []), denominator: copy(record.denominator || []), fraction: !!record.fraction,
      status, markup: math.toMathML(tokens, record.view.properties),
      unsupported: record.view.unsupported || "", hasSaved: !!targetDefinition(record.view.target),
      pendingProperty: record.pendingProperty || "",
      properties: (record.view.properties || []).filter(property => ["number", "boolean", "checkbox"].includes(property.format)
        && typeof property.key === "string" && typeof property.name === "string").map(property => ({
          key: property.key, name: property.name, description: math.propertyReference(property.key, record.view.properties),
        })),
      locked: locked || loading,
    };
  }
  const localDrafts = new Map();
  let draftGeneration=0;
  const math = UI.CalculatorMath;

  function targetDefinition(target, source) {
    return (source || state).definitions.find(item => identity(item.target) === identity(target));
  }
  function tokenLabel(token, view) {
    if (token.kind === "boolean") return token.value ? "True" : "False";
    if (token.kind === "number") return token.value;
    if (token.kind === "aggregate" || token.kind === "property") {
      const property = (view.properties || []).find(item => item.key === token.property);
      const name = property ? property.name : "Unavailable property";
      return token.kind === "property" ? name : (operations[token.operation] || "Aggregate") + " " + name;
    }
    if (token.kind === "count") return "Card count";
    if (token.kind === "operator") return ({ "*": "×", "/": "÷", "-": "−" })[token.value] || token.value;
    if (token.kind === "function") return token.value === "sqrt" ? "√" : "ⁿ√";
    return ({ open: "(", close: ")", comma: "," })[token.kind] || "?";
  }
  function updateTimestamp() {
    if (!el("formula-calculated-at")) return;
    const target = el("formula-calculated-at");
    if (!target) return;
    const timestamp = calculatedAt ? new Date(calculatedAt) : null;
    const formatted = timestamp && !Number.isNaN(timestamp.valueOf()) ? timestamp.toLocaleString() : "Not calculated yet";
    target.textContent = "Last calculated: " + formatted + ". Saved calculations are applied after you press Ctrl+R in Anytype.";
  }
  function parseFraction(tokens) {
    if (!Array.isArray(tokens) || tokens.length < 7 || tokens[0].kind !== "open") return null;
    let depth = 0, numeratorEnd = -1;
    for (let index = 0; index < tokens.length; index++) {
      if (tokens[index].kind === "open") depth++;
      else if (tokens[index].kind === "close" && --depth === 0) { numeratorEnd = index; break; }
    }
    if (numeratorEnd < 2 || tokens[numeratorEnd + 1]?.kind !== "operator" || tokens[numeratorEnd + 1].value !== "/"
      || tokens[numeratorEnd + 2]?.kind !== "open" || tokens[tokens.length - 1].kind !== "close") return null;
    depth = 0;
    let denominatorEnd = -1;
    for (let index = numeratorEnd + 2; index < tokens.length; index++) {
      if (tokens[index].kind === "open") depth++;
      else if (tokens[index].kind === "close" && --depth === 0) { denominatorEnd = index; break; }
    }
    if (denominatorEnd !== tokens.length - 1) return null;
    return { numerator: copy(tokens.slice(1, numeratorEnd)), denominator: copy(tokens.slice(numeratorEnd + 3, denominatorEnd)) };
  }
  function serializedTokens(record) {
    const numerator = copy(record.numerator);
    if (!record.fraction) return numerator;
    const denominator = copy(record.denominator);
    if (!numerator.length || !denominator.length) return [];
    return [
      { kind: "open" }, ...numerator, { kind: "close" },
      { kind: "operator", value: "/" },
      { kind: "open" }, ...denominator, { kind: "close" },
    ];
  }
  function hasCalculatorInput(record) {
    if (!record) return false;
    if (record.sourceMode && record.sourceInput && record.sourceInput.value.trim()) return true;
    if (record.sourceError && (record.source || "").trim()) return true;
    return !!(record.numerator.length || record.denominator.length);
  }
  function updateRows() {
    records.forEach(record => {
      const active = !!activeRecord && activeRecord.viewKey === record.key;
      if (record.row) {
        record.row.dataset.active = String(active);
        record.row.setAttribute("aria-pressed", String(active));
      }
    });
    publish();
  }
  function updateSaveActions(record) {
    if (!record) return;
    if (record.save) record.save.disabled = locked || loading;
    if (record.remove) record.remove.disabled = locked || loading;
    if (record.actions) record.actions.hidden = false;
  }
  function syncLocks() {
    const pageLocked = locked || loading;
    if (fields) fields.disabled = pageLocked;
    if (reload) {
      reload.disabled = pageLocked || discovering;
      reload.setAttribute("aria-busy", String(discovering));
      reload.classList.toggle("is-loading", discovering);
    }
    if (filterButton) filterButton.disabled = pageLocked || discovering;
    if (search) search.disabled = pageLocked;
    if (activeRecord) {
      if (activeRecord.fieldset) activeRecord.fieldset.disabled = pageLocked || !!activeRecord.view.unsupported;
      updateSaveActions(activeRecord);
    }
  }
  function openCalculator(record) {
    if (!record || locked || loading || !calculatorModal || !modalContent) return;
    const matches = UI.ViewCatalog?.definitionsForView(record.view, state.definitions) || [];
    const saved = targetDefinition(record.view.target) || matches[0];
    const editingView = saved ? { ...record.view, target: saved.target } : record.view;
    const savedTokens = copy(saved ? saved.tokens : []), fraction = parseFraction(savedTokens);
    activeRecord = {
      view: editingView,
      viewKey: record.key,
      row: record.row,
      numerator: fraction ? fraction.numerator : savedTokens,
      denominator: fraction ? fraction.denominator : [],
      fraction: !!fraction,
      cursors: { numerator: 0, denominator: 0 },
      active: "numerator",
      fieldset: null, display: null, status: null, save: null, remove: null,
      generation: 0, draftGeneration,
    };
    activeRecord.cursors.numerator = activeRecord.numerator.length;
    activeRecord.cursors.denominator = activeRecord.denominator.length;
    const pending = localDrafts.get(record.key);
    if (pending) { restoreRecordSnapshot(activeRecord, copy(pending)); activeRecord.source = pending.source; activeRecord.sourceMode = pending.sourceMode; activeRecord.sourceError=pending.sourceError; }
    buildCalculator(activeRecord);
    updateRows();
    calculatorModal.open();
    if (activeRecord.display) activeRecord.display.focus({ preventScroll: true });
    syncLocks();
    publish();
  }
  function updateDisplay() { publish(); }
  function recordSnapshot(record) {
    return {
      numerator: copy(record.numerator), denominator: copy(record.denominator),
      fraction: record.fraction, cursors: { ...record.cursors }, active: record.active,
    };
  }
  function restoreRecordSnapshot(record, snapshot) {
    record.numerator = snapshot.numerator;
    record.denominator = snapshot.denominator;
    record.fraction = snapshot.fraction;
    record.cursors = snapshot.cursors;
    record.active = snapshot.active;
  }
  function changed(record) {
    const tokens = serializedTokens(record);
    if (tokens.length > 256) return false;
    localDrafts.set(record.viewKey, { ...recordSnapshot(record), source: record.source, sourceMode: record.sourceMode, sourceError: record.sourceError });
    try {
      if (record.sourceError) throw Error(record.sourceError);
      if (record.fraction && (!record.numerator.length || !record.denominator.length)) throw Error("Complete both sides of the fraction.");
      if (tokens.length) math.parseTokens(tokens);
      setActiveStatus(record, "");
    } catch (error) { setActiveStatus(record, error.message); updateDisplay(record); updateSaveActions(record); return true; }
    if (record.fraction && (!record.numerator.length || !record.denominator.length)) {
      record.generation++;
      updateDisplay(record);
      updateSaveActions(record);
      return true;
    }
    const key = identity(record.view.target);
    state.definitions = state.definitions.filter(item => identity(item.target) !== key);
    if (tokens.length) state.definitions.push({ target: copy(record.view.target), tokens: tokens });
    state.definitions.sort((left, right) => identity(left.target).localeCompare(identity(right.target)));
    record.generation++;
    updateDisplay(record);
    updateSaveActions(record);
    api.onChange();
    return true;
  }
  function sectionTokens(record) { return record.active === "numerator" ? record.numerator : record.denominator; }
  function activateFraction(record) {
    if (record.fraction) return;
    record.fraction = true;
    record.active = "denominator";
    record.cursors.denominator = record.denominator.length;
  }
  function insertNumeric(record, action) {
    const tokens = sectionTokens(record), cursor = record.cursors[record.active];
    const previous = tokens[cursor - 1], next = tokens[cursor];
    if (previous && previous.kind === "number") {
      if (action === "." && previous.value.includes(".")) return;
      if (previous.value.length >= 32) return;
      previous.value += action;
    } else if (next && next.kind === "number" && cursor === 0) {
      if (action === "." && next.value.includes(".")) return;
      if (next.value.length >= 32) return;
      next.value = action + next.value;
      record.cursors[record.active]++;
    } else {
      tokens.splice(cursor, 0, { kind: "number", value: action === "." ? "0." : action });
      record.cursors[record.active]++;
    }
  }
  function unaryPosition(tokens, index) {
    const before = tokens[index - 1];
    return !before || before.kind === "open" || before.kind === "comma"
      || (before.kind === "operator" && ["+", "-", "*", "/", "^"].includes(before.value));
  }
  function toggleSign(record) {
    const tokens = sectionTokens(record), cursor = record.cursors[record.active];
    const previousIndex = cursor - 1, previous = tokens[previousIndex];
    if (previous && previous.kind === "number") {
      const preceding = tokens[previousIndex - 1];
      if (preceding && preceding.kind === "operator" && preceding.value === "-" && unaryPosition(tokens, previousIndex - 1)) {
        tokens.splice(previousIndex - 1, 1);
        record.cursors[record.active]--;
      } else {
        tokens.splice(previousIndex, 0, { kind: "operator", value: "-" });
        record.cursors[record.active]++;
      }
      return;
    }
    if (previous && ["aggregate", "count", "close"].includes(previous.kind)) {
      let start = previousIndex;
      if (previous.kind === "close") {
        let depth = 1;
        for (let index = previousIndex - 1; index >= 0; index--) {
          if (tokens[index].kind === "close") depth++;
          else if (tokens[index].kind === "open" && --depth === 0) { start = index; break; }
        }
      }
      const preceding = tokens[start - 1];
      if (preceding && preceding.kind === "operator" && preceding.value === "-" && unaryPosition(tokens, start - 1)) {
        tokens.splice(start - 1, 1);
        record.cursors[record.active]--;
      } else {
        tokens.splice(start, 0, { kind: "operator", value: "-" });
        record.cursors[record.active]++;
      }
      return;
    }
    tokens.splice(cursor, 0, { kind: "operator", value: "-" });
    record.cursors[record.active]++;
  }
  function isValueBefore(tokens, cursor) {
    const previous = tokens[cursor - 1];
    return previous && ["number", "aggregate", "count", "close"].includes(previous.kind);
  }
  function edit(record, action) {
    if (!record || locked || loading || modalSaving || record.view.unsupported) return;
    if (!record.sourceMode || !record.sourceInput) return;
    const source = record.sourceInput;
    if (action === "clear") {
      source.value = "";
      source.setSelectionRange(0, 0);
      handleSourceInput(record, source);
      source.focus({ preventScroll: true });
      return;
    }
    const start = source.selectionStart, end = source.selectionEnd;
    if (action === "backspace" || action === "delete") {
      const left = start === end && action === "backspace" ? Math.max(0, start - 1) : start;
      const right = start === end && action === "delete" ? Math.min(source.value.length, end + 1) : end;
      insertSource(record, "", left, right);
      return;
    }
    const snippets = { fraction: "\\frac{}{}", sqrt: "\\sqrt{}", root: "\\sqrt[]{}", square: "^{2}", sign: "-", "*": "\\times ", "/": "\\div " };
    const insertionStart = source.selectionStart;
    const snippet = snippets[action] ?? action;
    insertSource(record, snippet);
    const placeholder = action === "root" ? snippet.indexOf("[") : ["fraction", "sqrt"].includes(action) ? snippet.indexOf("{") : -1;
    if (placeholder >= 0) source.setSelectionRange(insertionStart + placeholder + 1, insertionStart + placeholder + 1);
  }
  function insertSource(record, snippet, start, end) {
    if (!record.sourceMode || !record.sourceInput || locked || loading || modalSaving || record.view.unsupported) return;
    const source = record.sourceInput;
    start ??= source.selectionStart;
    end ??= source.selectionEnd;
    source.setRangeText(snippet, start, end, "end");
    handleSourceInput(record, source);
    source.focus({ preventScroll: true });
  }

  function setActiveStatus(record, text) {
    if (!record) return;
    record.statusText = text || "";
    if (!record.status) { publish(); return; }
    record.status.textContent = text || "";
    record.status.title = text || "";
  }
  function endsValue(tokens, cursor) {
    const previous = tokens[cursor - 1];
    return !!previous && ["number", "aggregate", "count", "close"].includes(previous.kind);
  }
  function startsValue(tokens, cursor) {
    const next = tokens[cursor];
    return !!next && ["number", "aggregate", "count", "open", "function"].includes(next.kind);
  }
  // Scan source without parsing it so incomplete drafts retain their insertion scope.
  function sourcePropertyRanges(source) {
    const ranges = [];
    const skipSpace = index => { while (/\s/.test(source[index] || "")) index++; return index; };
    function group(start) {
      if (source[start] !== "{") return null;
      let depth = 1, quoted = false, escaped = false;
      for (let index = start + 1; index < source.length; index++) {
        const character = source[index];
        if (quoted) {
          if (escaped) escaped = false;
          else if (character === "\\") escaped = true;
          else if (character === '"') quoted = false;
        } else if (character === '"') quoted = true;
        else if (character === "{") depth++;
        else if (character === "}" && --depth === 0) return { start, end: index + 1, closed: true };
      }
      return { start, end: source.length, closed: false };
    }
    let quoted = false, escaped = false;
    for (let index = 0; index < source.length; index++) {
      const character = source[index];
      if (quoted) {
        if (escaped) escaped = false;
        else if (character === "\\") escaped = true;
        else if (character === '"') quoted = false;
        continue;
      }
      if (character === '"') { quoted = true; continue; }
      if (!/^\\property\b/.test(source.slice(index))) continue;
      const first = group(skipSpace(index + 9));
      if (!first) continue;
      let end = first.end;
      const selector = /^#\d+/.exec(source.slice(end));
      if (selector) end += selector[0].length;
      const second = first.closed ? group(skipSpace(end)) : null;
      if (second) end = second.end;
      const third = second?.closed ? group(skipSpace(end)) : null;
      ranges.push({ start: index, metadataEnd: end, third });
    }
    return ranges;
  }
  function sourceIsPerCard(source, start, end = start) {
    return sourcePropertyRanges(source).some(({ third }) => third && start > third.start
      && end <= third.end - (third.closed ? 1 : 0));
  }
  function sourceBoundaryIsInsideProperty(source, position) {
    return sourcePropertyRanges(source).some(range => position > range.start && position < range.metadataEnd);
  }
  function sourceEndsValue(source, position) {
    if (!position || sourceBoundaryIsInsideProperty(source, position)) return false;
    const character = source[position - 1];
    return /[\d.)}]/.test(character) || character === "#";
  }
  function sourceStartsValue(source, position) {
    if (position >= source.length || sourceBoundaryIsInsideProperty(source, position)) return false;
    const rest = source.slice(position);
    return /^(?:\d|\.|\(|\{|\\(?:property\b|frac\b|sqrt\b|count\b|left\())/.test(rest);
  }
  function insertVariable(record, token) {
    if (!record.sourceMode || locked || loading || modalSaving || record.view.unsupported) return;
    if (record.sourceMode && record.sourceInput) {
      const source = record.sourceInput;
      let snippet;
      try { snippet = token.kind === "property" ? math.propertyReference(token.property, record.view.properties) : math.toLatex([token], record.view.properties); }
      catch (error) { setActiveStatus(record, error.message); return; }
      const start = Number.isInteger(source.selectionStart) ? source.selectionStart : source.value.length;
      const end = Number.isInteger(source.selectionEnd) ? source.selectionEnd : start;
      let sourceIsValid = false;
      try { math.parseLatex(source.value, record.view.properties); sourceIsValid = true; }
      catch (_) { /* Preserve invalid source verbatim around the inserted reference. */ }
      const before = sourceIsValid && sourceEndsValue(source.value, start) ? "*" : "";
      const after = sourceIsValid && sourceStartsValue(source.value, end) ? "*" : "";
      const insertion = before + snippet + after;
      source.focus({ preventScroll: true });
      source.setRangeText(insertion, start, end, "end");
      handleSourceInput(record, source);
      source.focus({ preventScroll: true });
      const caret = start + before.length + snippet.length;
      source.setSelectionRange(caret, caret);
      return;
    }
  }

  function saveCalculator(record) {
    if (!record || record !== activeRecord || locked || loading) return;
    calculatorModal.close();
  }
  function toggleSourceMode(record = activeRecord) {
    if (!record || record !== activeRecord || locked || loading || record.view.unsupported) return;
    record.resetPropertyPicker?.();
    record.sourceMode = !record.sourceMode;
    if (record.sourceMode && record.source === undefined) {
      try { record.source = record.fraction ? math.toLatex(record, record.view.properties, true)
        : record.numerator.length ? math.toLatex(record.numerator, record.view.properties) : ""; }
      catch (_) { record.source = record.numerator.map(token => tokenLabel(token, record.view)).join(" "); }
    }
    publish();
  }
  function handleSourceInput(record, source) {
    if (!record || record !== activeRecord || !source || !record.sourceMode || locked || loading || modalSaving) return;
    record.sourceInput = source;
    record.source = source.value;
    record.sourceError = "";
    try {
      const tokens = source.value.trim() ? math.parseLatex(source.value, record.view.properties) : [];
      const fraction = parseFraction(tokens);
      record.numerator = fraction ? fraction.numerator : tokens;
      record.denominator = fraction ? fraction.denominator : [];
      record.fraction = !!fraction;
      record.cursors = { numerator: record.numerator.length, denominator: record.denominator.length };
    } catch (error) { record.sourceError = error.message; }
    changed(record);
  }
  function chooseProperty(record, key) {
    if (!record || record !== activeRecord || !record.sourceMode || locked || loading) return;
    const source = record.sourceInput;
    if (source && sourceIsPerCard(source.value, source.selectionStart, source.selectionEnd)) {
      insertVariable(record, { kind: "property", property: key });
      record.pendingProperty = null;
      publish();
      return;
    }
    record.pendingProperty = key;
    publish();
  }
  function chooseAggregation(record, operation) {
    if (!record || record !== activeRecord || !record.pendingProperty || !record.sourceMode || locked || loading) return;
    insertVariable(record, { kind: "aggregate", property: record.pendingProperty, operation });
    record.pendingProperty = null;
    publish();
  }
  function removeActiveCalculation(record = activeRecord) {
    if (!record || record !== activeRecord || locked || loading || !targetDefinition(record.view.target)) return;
    record.numerator = []; record.denominator = []; record.fraction = false;
    record.cursors = { numerator: 0, denominator: 0 }; record.source = ""; record.sourceError = "";
    changed(record);
    calculatorModal.close();
  }
  function buildCalculator(record) { publish(); }
  function filter() { publish(); }
  function updateFilterOptions() { publish(); }
  function render() {
    if (!initialized) return;
    if (calculatorModal && calculatorModal.isOpen()) {
      updateTimestamp();
      publish();
      return;
    }
    records = [];
    const supported = catalog.filter(view => !view.unsupported);
    const views = UI.ViewCatalog ? UI.ViewCatalog.dedupeViews(supported).views : supported.slice();
    for (const saved of state.definitions) {
      // Keep hidden unsupported definitions in the draft without recreating
      // them as unavailable rows. Missing views still offer manual removal.
      if (catalog.some(view => view.unsupported && identity(view.target) === identity(saved.target))) continue;
      if (!views.some(view => identity(view.target) === identity(saved.target)) && !UI.ViewCatalog?.findViewForTarget(views,saved.target)) {
        views.push({
          target: saved.target, name: "Unavailable view", space: "Saved calculation",
          source: "Reload the list or remove this calculation", kind: "Unavailable", properties: [],
          unsupported: "This saved view is unavailable. Its calculation is kept until you remove it and save.",
        });
      }
    }
    views.sort((left, right) => left.name.localeCompare(right.name) || left.space.localeCompare(right.space) || left.source.localeCompare(right.source));
    views.forEach(view => records.push({ view, item: null, row: null, key: identity(view.target) }));
    updateFilterOptions();
    filter();
    syncLocks();
    updateTimestamp();
    publish();
  }
  async function discover(forceRefresh = false) {
    if (!invoke) return;
    if (loading || locked) { discoveryRequested = true; return; }
    if (discovering) return;
    const generation = ++discoveryGeneration;
    discoveryRequested = false;
    discovering = true;
    setMessage(loadError);
    syncLocks();
    try {
      const result = await invoke("discover_kanban_views", { forceRefresh });
      if (!result || !Array.isArray(result.views)) throw Error("Anytype did not return a view list.");
      if (generation !== discoveryGeneration) return;
      catalog = result.views;
      if (activeRecord) {
        const next = catalog.find(view => identity(view.target) === activeRecord.viewKey);
        Object.assign(activeRecord.view, next ? { ...next, target: activeRecord.view.target } : { properties: [], unsupported: "This view is unavailable. Your draft is retained." });
        activeRecord.refreshPropertyOptions?.();
        syncLocks();
      }
      (result.info || []).forEach(message => api?.onInfo?.("Anytype API. " + message));
      const logged = new Set();
      catalog.forEach(view => {
        if (!view.unsupported) return;
        const key = identity(view.target);
        if (logged.has(key)) return;
        logged.add(key);
        api?.onInfo?.("Skipped Kanban view \"" + view.name + "\" in " + view.space + " · " + view.source + " · " + view.kind + ". " + view.unsupported);
      });
      discovered = true;
      setMessage([loadError, result.warning].filter(Boolean).join(" "));
    } catch (error) {
      if (generation === discoveryGeneration) { setMessage(String(error.message || error)); api?.onInfo?.(String(error.message || error)); }
    } finally {
      if (generation === discoveryGeneration) {
        discovering = false;
        render();
      }
    }
  }
  function configureModal() {
    if (!modalBackdrop || !modalDialog || !modalContent || !UI.createModalController) return;
    calculatorModal = UI.createModalController({
      backdrop: modalBackdrop,
      dialog: modalDialog,
      initialFocus: () => activeRecord && activeRecord.display ? activeRecord.display : modalClose,
      fallbackFocus: () => activeRecord && activeRecord.row ? activeRecord.row : reload,
      inertTargets: [el("settings-shell"), el("title-drag-region")].filter(Boolean),
      onClose() {
        if(activeRecord && activeRecord.draftGeneration===draftGeneration) { localDrafts.set(activeRecord.viewKey,{...recordSnapshot(activeRecord),source:activeRecord.source,sourceMode:activeRecord.sourceMode,sourceError:activeRecord.sourceError});activeRecord.propertyPicker?.destroy(); }
        if (activeRecord && activeRecord.row) {
          activeRecord.row.dataset.active = "false";
          activeRecord.row.setAttribute("aria-pressed", "false");
        }
        activeRecord = null;
        modalSaving = false;
        updateRows();
        render();
      },
    });
    if (modalClose) modalClose.addEventListener("click", () => calculatorModal.close());
  }
  UI.FormulaPage = {
    init(options) {
      api = options;
      initialized = true;
      syncDomRefs();
      if (UI.AppReload) {
        reloadController = UI.AppReload;
        unregisterReload = reloadController.register(() => discover(true));
      }
      configureModal();
      render();
    },
    getSnapshot: snapshot,
    subscribe(listener) { viewSubscribers.add(listener); listener(snapshot()); return () => viewSubscribers.delete(listener); },
    setDisplay(value) {
      if (locked || loading) return;
      state.display = ["balanced", "spacious"].includes(value) ? value : "balanced";
      publish(); api?.onChange();
    },
    selectView(target, row) {
      const record = records.find(item => identity(item.view.target) === identity(target));
      if (!record) return;
      record.row = row || null;
      openCalculator(record);
    },
    reloadViews() { return discover(true); },
    setCalculatorSourceInput(input) { if (activeRecord) activeRecord.sourceInput = input || null; },
    setCalculatorDisplay(display) { if (activeRecord) activeRecord.display = display || null; },
    calculatorSourceChanged(input) { handleSourceInput(activeRecord, input); },
    toggleCalculatorSource() { toggleSourceMode(); },
    calculatorEdit(action) { if (action === "save") saveCalculator(activeRecord); else edit(activeRecord, action); },
    calculatorChooseProperty(key) { chooseProperty(activeRecord, key); },
    calculatorChooseAggregation(operation) { chooseAggregation(activeRecord, operation); },
    calculatorInsertCount() { insertVariable(activeRecord, { kind: "count" }); },
    removeCalculatorDefinition() { removeActiveCalculation(); },
    read: () => copy(state),
    validateSaved(result) {
      if (!result?.configuration || result.configuration.version !== 2 || !Array.isArray(result.configuration.definitions)
        || !["balanced", "spacious"].includes(result.configuration.display)) throw Error("Unsupported saved calculations.");
    },
    reconcile(saved, baseline, warn) {
      const equal = UI.SavedState.equal;
      const previous = persistedState;
      // Invalid/incomplete source drafts are outside the serializable configuration.
      for (const [key, pending] of localDrafts) {
        const before = previous.definitions.find(d => identity(d.target) === key);
        const after = baseline.definitions.find(d => identity(d.target) === key);
        if (!equal(before, after)) {
          const current = state.definitions.find(d => identity(d.target) === key);
          const incomplete = pending.sourceError || (pending.fraction && (!pending.numerator.length || !pending.denominator.length));
          // A changed serializable draft already produces a merge warning.
          if (incomplete && (equal(current, before) || equal(current, after))) warn("Calculations.definitions[" + key + "].tokens");
          localDrafts.delete(key);
        }
      }
      state = copy(saved);
      persistedState = copy(baseline);
      if (activeRecord) {
        const record = activeRecord, key = identity(record.view.target);
        const before = previous.definitions.find(d => identity(d.target) === key);
        const after = baseline.definitions.find(d => identity(d.target) === key);
        if (!equal(before, after)) {
          record.generation++;
          record.propertyPicker?.destroy();
          const definition = state.definitions.find(d => identity(d.target) === key);
          if (!definition && before) {
            draftGeneration++;
            calculatorModal.close({ restoreFocus: false });
          } else {
            const tokens = copy(definition?.tokens || []), fraction = parseFraction(tokens);
            record.numerator = fraction ? fraction.numerator : tokens;
            record.denominator = fraction ? fraction.denominator : [];
            record.fraction = !!fraction;
            record.cursors = { numerator: record.numerator.length, denominator: record.denominator.length };
            record.source = undefined; record.sourceError = "";
            if (record.sourceMode) record.source = math.toLatex(tokens, record.view.properties);
            buildCalculator(record);
          }
        }
      }
      render(); updateRows();
    },
    acceptSaved(result) {
      configurationRevision = result.revision || null;
      calculatedAt = result.calculatedAt || "";
      loadError = ""; setMessage(""); updateTimestamp();
    },
    isPersisted: () => JSON.stringify(state) === JSON.stringify(persistedState),
    restore(value) {
      draftGeneration++;
      if(calculatorModal?.isOpen()) calculatorModal.close({restoreFocus:false});
      localDrafts.clear();
      state = copy(value || defaults());
      persistedState = copy(state);
      if (calculatorModal && calculatorModal.isOpen()) calculatorModal.close({ restoreFocus: false });
      render();
    },
    async load(bridge, options = {}) {
      invoke = bridge;
      loadError = "";
      discoveryRequested = options.discover !== false;
      discovered = false;
      discoveryGeneration++;
      discovering = false;
      catalog = [];
      searchText = ""; selectedSpace = "";
      draftGeneration++;
      if (calculatorModal?.isOpen()) calculatorModal.close({ restoreFocus: false });
      localDrafts.clear();
      try {
        const result = await invoke("load_kanban_formulas");
        if (result) {
          if (!result.configuration || result.configuration.version !== 2 || !Array.isArray(result.configuration.definitions)
            || !["balanced", "spacious"].includes(result.configuration.display)) throw Error("Unsupported configuration");
          configurationRevision = result.revision || null;
          state = copy(result.configuration);
          persistedState = copy(state);
          calculatedAt = result.calculatedAt || "";
        }
        setMessage("");
      } catch (_) {
        state = defaults();
        persistedState = copy(state);
        loadError = "Could not load saved formulas. The original file is unchanged. Make changes and Save to replace it, or restart to retry.";
        setMessage(loadError);
      }
      loading = false;
      render();
      if (options.discover !== false) {
        void discover();
      }
      return !loadError;
    },
    async save(bridge, submitted) {
      try {
        const result = await bridge("save_kanban_formulas", { configuration: submitted, revision: configurationRevision });
        (result?.info || []).forEach(message => api?.onInfo?.("Anytype API. " + message));
        configurationRevision = result?.revision || configurationRevision;
        persistedState = copy(submitted);
        calculatedAt = result && result.calculatedAt || calculatedAt;
        loadError = "";
        updateTimestamp();
        setMessage("Formulas saved. Press Ctrl+R in Anytype to apply.", true);
        return result;
      } catch (error) {
        setMessage(String(error.message || error) + " Your pending edits and previous published results are kept.");
        throw Error("Kanban formulas could not be saved. Review the message on the Kanban View Formulas page and retry.");
      }
    },
    setLocked(value, pending) {
      const wasLocked = locked;
      locked = value;
      dirty = !!pending;
      syncLocks();
      publish();
      if (wasLocked && !locked && !loading && discoveryRequested) {
        void discover();
      }
    },
    refresh() { if (!discovered && !discovering && !loading) return discover(); },
    reload() {
      discovered = false;
      discoveryRequested = true;
      return !discovering && !loading && !locked ? discover() : undefined;
    },
    destroy() {
      viewSubscribers.clear();
      unregisterReload?.();
      if (reloadController) reloadController.bindButton(null);
      if (calculatorModal) calculatorModal.close({ restoreFocus: false });
    },
  };
})(window);
