(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {}, doc = root.document;
  const defaults = () => ({ version: 2, display: "balanced", definitions: [] });
  const copy = value => JSON.parse(JSON.stringify(value));
  const identity = target => JSON.stringify([target.spaceId, target.objectId, target.blockId, target.viewId]);
  const el = id => doc.getElementById(id);
  const contents = el("formula-contents"), message = el("formula-storage-message"), preference = el("formula-display");
  const fields = el("formula-fields"), search = el("formula-search"), reload = el("formula-reload");
  const filterButton = el("formula-filter");
  const displayPickerRoot = el("formula-display-select");
  const modalBackdrop = el("formula-calculator-backdrop"), modalDialog = el("formula-calculator-dialog");
  const modalClose = el("formula-calculator-close"), modalContent = el("formula-calculator-content");
  function setMessage(text, success = false) {
    if (!message) return;
    UI.CatalogBrowser.status(message, text, success);
  }
  const operations = { total: "Total", average: "Average", minimum: "Minimum", maximum: "Maximum", median: "Median" };
  let state = defaults(), persistedState = defaults(), catalog = [], api, invoke, calculatorModal;
  let locked = false, dirty = false, loading = true, discovering = false, discovered = false;
  let records = [], initialized = false, loadError = "", activeRecord = null, modalSaving = false, calculatedAt = "";
  let displayControl, reloadController, unregisterReload, browser;
  let discoveryRequested = false;
  let discoveryGeneration = 0;
  const localDrafts = new Map();
  let draftGeneration=0;
  const math = UI.CalculatorMath;

  function make(tag, className, text) {
    const node = doc.createElement(tag);
    node.className = className || "";
    if (text !== undefined) node.textContent = text;
    return node;
  }
  function button(text, action, className) {
    return UI.Components.Button({ label: text, onClick: action,
      attributes: { class: className || "formula-button" },
    }).element;
  }
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
      record.row.dataset.active = String(active);
      record.row.setAttribute("aria-pressed", String(active));
    });
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
    if (preference) preference.disabled = pageLocked;
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
  function syncDisplayPicker() {
    if (!preference) return;
    preference.value = state.display;
    if (displayControl) displayControl.setValue(state.display);
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
  }
  function renderSequence(record, host, tokens, cursor, section, placeholder) {
    host.replaceChildren();
    if (!tokens.length) {
      if (record.active === section && cursor === 0) host.append(make("span", "formula-caret", "│"));
      host.append(make("span", "formula-placeholder", placeholder));
      return;
    }
    for (let index = 0; index <= tokens.length; index++) {
      if (record.active === section && index === cursor) host.append(make("span", "formula-caret", "│"));
      if (index < tokens.length) {
        const token = make("span", ["aggregate", "count"].includes(tokens[index].kind) ? "formula-term" : "formula-symbol", tokenLabel(tokens[index], record.view));
        token.dataset.tokenIndex = String(index);
        host.append(token);
      }
    }
  }
  function updateDisplay(record) {
    if (!record.display) return;
    syncRenderToggle(record);
    record.display.hidden = !!record.sourceMode;
    if (record.sourceInput) record.sourceInput.hidden = !record.sourceMode;
    record.linearDisplay.hidden = record.fraction;
    record.fractionDisplay.hidden = !record.fraction;
    record.display.dataset.fraction = String(record.fraction);
    const renderMath = (host, tokens, section) => { host.innerHTML = math.toMathML(tokens, record.view.properties, record.cursors[section], false); };
    renderMath(record.fraction ? record.numeratorDisplay : record.linearDisplay, record.numerator, "numerator");
    if (record.fraction) {
      record.fractionDisplay.innerHTML='<math xmlns="http://www.w3.org/1998/Math/MathML"><mfrac><mrow data-section="numerator">'+math.toMathMLBody(record.numerator,record.view.properties,record.cursors.numerator,false)+'</mrow><mrow data-section="denominator">'+math.toMathMLBody(record.denominator,record.view.properties,record.cursors.denominator,false)+'</mrow></mfrac></math>';
      record.numeratorDisplay=record.fractionDisplay.querySelector('[data-section="numerator"]');record.denominatorDisplay=record.fractionDisplay.querySelector('[data-section="denominator"]');
      for(const section of ['numerator','denominator']) record[section+'Display'].addEventListener('click',event=>displayTokenClick(record,section,event));
    }
    else { record.numeratorDisplay.replaceChildren(); record.denominatorDisplay.replaceChildren(); }
    record.display.dataset.active = record.active;
    record.numeratorDisplay.classList.toggle("is-active", record.fraction && record.active === "numerator");
    record.linearDisplay.classList.toggle("is-active", !record.fraction && record.active === "numerator");
    record.denominatorDisplay.classList.toggle("is-active", record.active === "denominator");
    const topLabel = record.numerator.map(token => tokenLabel(token, record.view)).join(" ") || "empty";
    const bottomLabel = record.denominator.map(token => tokenLabel(token, record.view)).join(" ") || "empty";
    record.display.setAttribute("aria-label", record.fraction
      ? "Calculation numerator " + topLabel + ", denominator " + bottomLabel + ". Read only. Show LaTeX to edit."
      : "Calculation " + topLabel + ". Read only. Show LaTeX to edit.");
  }
  function syncRenderToggle(record) {
    if (!record || !record.renderToggle) return;
    const label = record.sourceMode ? "Show rendered" : "Show LaTeX";
    record.renderToggle.setAttribute("aria-label", label);
    record.renderToggle.setAttribute("title", label);
    record.renderToggle.setAttribute("aria-pressed", String(!!record.sourceMode));
  }
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
      source.dispatchEvent(new root.Event("input", { bubbles: true }));
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
    source.dispatchEvent(new root.Event("input", { bubbles: true }));
    source.focus({ preventScroll: true });
  }

  function setActiveStatus(record, text) {
    if (!record || !record.status) return;
    record.status.textContent = text || "";
    record.status.title = text || "";
  }
  function handleDisplayKeydown(record, event) {
    if (event.key === "Enter" || event.key === "=") {
      event.preventDefault();
      void saveCalculator(record);
    }
  }
  function displayTokenClick() { /* The rendered formula is read only. */ }

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
      source.dispatchEvent(new root.Event("input", { bubbles: true }));
      source.focus({ preventScroll: true });
      const caret = start + before.length + snippet.length;
      source.setSelectionRange(caret, caret);
      return;
    }
  }

  function createPropertyControls(record, host) {
    const area = make('div', 'formula-variable-actions'); record.variableActions=area;
    const picker = make('div', 'kanban-select formula-property-picker');
    const trigger = button('Insert property', event => {
      if (!record.sourceMode) { event.preventDefault(); event.stopImmediatePropagation(); }
      else if (trigger.getAttribute('aria-expanded') !== 'true') record.resetPropertyPicker?.();
    }, 'formula-insert-property');
    trigger.addEventListener('keydown', event => {
      if (!record.sourceMode && ['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
    });
    trigger.tabIndex = -1;
    trigger.setAttribute('aria-haspopup','listbox'); trigger.setAttribute('aria-expanded','false');
    const menu=make('div','kanban-select-menu'); menu.setAttribute('role','listbox'); menu.setAttribute('aria-label','Insert property'); menu.hidden=true;
    picker.append(trigger,menu); area.append(picker,button('Card count',()=>insertVariable(record,{kind:'count'}),'formula-variable-button'));
    const properties=()=>(record.view.properties||[])
      .filter(property=>(property.format==="number"||property.format==="boolean"||property.format==="checkbox")
        && typeof property.key==="string" && typeof property.name==="string")
      .map(property=>({
        value:property.key,
        label:property.name,
        description:math.propertyReference(property.key,record.view.properties),
      }));
    let pendingProperty = null;
    record.resetPropertyPicker = () => {
      pendingProperty = null;
      record.propertyPicker?.setOptions(properties());
      trigger.textContent = "Insert property";
    };
    record.propertyPicker=UI.Components.Picker({root:picker,options:properties(),focusOnOpen:false,restoreFocusOnSelect:false,emptyMessage:'No number or boolean properties in this view',onChange(value){
      if (!record.sourceMode) return;
      if (!pendingProperty) {
        const source = record.sourceInput;
        if (source && sourceIsPerCard(source.value, source.selectionStart, source.selectionEnd)) {
          insertVariable(record, { kind: 'property', property: value });
          return;
        }
        pendingProperty = value;
        record.propertyPicker.setOptions(Object.entries(operations).map(([operation, label]) => ({ value: operation, label })));
        trigger.textContent = 'Choose aggregation';
        queueMicrotask(() => record.propertyPicker.open());
        return;
      }
      insertVariable(record,{kind:'aggregate',property:pendingProperty,operation:value});
      pendingProperty = null;
      record.propertyPicker.setOptions(properties());
      trigger.textContent = 'Insert property';
    }});
    host.append(area);
  }

  function saveCalculator(record) {
    if (!record || record !== activeRecord || locked || loading) return;
    calculatorModal.close();
  }
  function buildCalculator(record) {
    const view = record.view, host = modalContent;
    UI.Components.destroyTree(host);
    host.replaceChildren();
    const heading = make("div", "formula-calculator-heading");
    heading.append(make("h2", "formula-calculator-title", view.name));
    heading.append(make("p", "formula-calculator-source", view.space + " · " + view.source + " · " + view.kind));
    host.append(heading);
    if (view.unsupported) host.append(make("p", "formula-calculator-error field-error", view.unsupported));
    else {
      const fieldset = make("fieldset", "formula-calculator");
      record.fieldset = fieldset;
      fieldset.append(make("legend", "sr-only", "Calculator for " + view.name));
      const display = make("div", "formula-display");
      const displaySlot = make("div", "formula-display-slot");
      record.displaySlot = displaySlot;
      display.tabIndex = 0;
      display.setAttribute("aria-readonly", "true");
      display.setAttribute("role", "textbox");
      display.setAttribute("aria-multiline", "false");
      display.setAttribute("aria-label", "Calculation numerator and denominator");
      const linear = make("div", "formula-linear");
      const fraction = make("div", "formula-fraction");
      record.fractionDisplay = fraction;
      const numerator = make("div", "formula-numerator");
      const denominator = make("div", "formula-denominator");
      numerator.setAttribute("aria-label", "Numerator");
      denominator.setAttribute("aria-label", "Denominator");
      fraction.append(numerator, make("span", "formula-fraction-bar", ""), denominator);
      display.append(linear, fraction);
      record.display = display;
      record.linearDisplay = linear;
      record.numeratorDisplay = numerator;
      record.denominatorDisplay = denominator;
      linear.addEventListener("click", event => displayTokenClick(record, "numerator", event));
      numerator.addEventListener("click", event => displayTokenClick(record, "numerator", event));
      denominator.addEventListener("click", event => displayTokenClick(record, "denominator", event));
      display.addEventListener("keydown", event => handleDisplayKeydown(record, event));
      displaySlot.append(display);
      fieldset.append(displaySlot);
      createPropertyControls(record, fieldset);
      const utility = make("div", "formula-calculator-utility");
      [["C", "clear", "Clear calculation"], ["", "backspace", "Backspace"]].forEach(([label, action, accessible]) => {
        const key = button(label, () => edit(record, action), "formula-key");
        key.tabIndex = -1;
        key.setAttribute("aria-label", accessible); key.title = accessible;
        if (action === "backspace") {
          const icon = doc.createElementNS("http://www.w3.org/2000/svg", "svg");
          icon.setAttribute("viewBox", "0 0 20 20");
          icon.setAttribute("aria-hidden", "true");
          const outline = doc.createElementNS("http://www.w3.org/2000/svg", "path");
          outline.setAttribute("d", "M8 4.5h9.5v11H8L3.5 10 8 4.5Z");
          outline.setAttribute("fill", "none");
          outline.setAttribute("stroke", "currentColor");
          outline.setAttribute("stroke-width", "1.5");
          outline.setAttribute("stroke-linejoin", "round");
          const cross = doc.createElementNS("http://www.w3.org/2000/svg", "path");
          cross.setAttribute("d", "m11 8 4 4m0-4-4 4");
          cross.setAttribute("fill", "none");
          cross.setAttribute("stroke", "currentColor");
          cross.setAttribute("stroke-width", "1.5");
          cross.setAttribute("stroke-linecap", "round");
          icon.append(outline, cross);
          key.append(icon);
        }
        utility.append(key);
      });
      fieldset.append(utility);
      const keypad = make("div", "formula-keypad");
      const keys = [
        ["√", "sqrt", "Square root"], ["(", "(", "Open parenthesis"], [")", ")", "Close parenthesis"], ["ⁿ√", "root", "Nth root"], ["÷", "/", "Divide"],
        ["xʸ", "^", "Power"], ["7", "7"], ["8", "8"], ["9", "9"], ["×", "*", "Multiply"],
        ["x²", "square", "Square"], ["4", "4"], ["5", "5"], ["6", "6"], ["−", "-", "Subtract"],
        [",", ",", "Comma"], ["1", "1"], ["2", "2"], ["3", "3"], ["+", "+", "Add"],
        ["x/y", "fraction", "Fraction"], ["±", "sign", "Change sign"], ["0", "0"], [".", ".", "Decimal point"], ["Save", "save", "Save calculation"],
      ];
      const logicKeys = [
        ['=', '==', 'Equal'], ['≠', '!=', 'Not equal'], ['<', '<', 'Less than'], ['≤', '<=', 'Less than or equal'], ['Save', 'save', 'Save calculation'],
        ['>', '>', 'Greater than'], ['≥', '>=', 'Greater than or equal'], ['AND', '&&', 'Logical AND'], ['OR', '||', 'Logical OR'], ['NOT', '!', 'Logical NOT'],
        ['True', 'true', 'True'], ['False', 'false', 'False'], ['(', '(', 'Open parenthesis'], [')', ')', 'Close parenthesis'], ['?', '?', 'Conditional question mark'],
        [':', ':', 'Conditional colon'],
      ];
      const modes = make('div', 'formula-keypad-modes');
      modes.setAttribute('role', 'group'); modes.setAttribute('aria-label', 'Calculator keypad');
      const modeButtons = ['Math', 'Logic'].map(label => {
        const control = button(label, () => showKeys(label), 'formula-key');
        control.addEventListener('mousedown', event => event.preventDefault());
        modes.append(control); return control;
      });
      fieldset.append(modes);
      function showKeys(mode) {
        modeButtons.forEach(control => control.setAttribute('aria-pressed', String(control.textContent === mode)));
        keypad.replaceChildren();
        (mode === 'Logic' ? logicKeys : keys).forEach(([label, action, accessible]) => {
          const key = button(label, () => {
            if (action === "save") void saveCalculator(record);
            else edit(record, action);
          }, "formula-key");
          key.tabIndex = -1;
          if (accessible) { key.setAttribute("aria-label", accessible); key.title = accessible; }
          if (action === "save") { key.id = "formula-calculator-save"; record.save = key; }
          keypad.append(key);
        });
      }
      showKeys('Math');
      fieldset.append(keypad);
      host.append(fieldset);
      record.status = make("p", "formula-calculator-status");
      record.status.id = "formula-calculator-status";
      record.status.setAttribute("role", "status");
      record.status.setAttribute("aria-live", "polite");
      display.setAttribute("aria-describedby", record.status.id);
      const unavailableProperty = tokens => tokens.some(token => (["aggregate", "property"].includes(token.kind)
        && !(view.properties || []).some(item => item.key === token.property)) || (token.calculation && unavailableProperty(token.calculation)));
      if (unavailableProperty([...record.numerator, ...record.denominator])) {
        record.status.textContent = "A saved property is unavailable. Remove the calculation and choose a replacement.";
      }
      host.append(record.status);
    }
    const actions = make("div", "formula-calculator-actions");
    record.actions = actions;
    if (view.unsupported) {
      if (targetDefinition(view.target)) {
        record.remove = button("Remove calculation", () => {
          if (record !== activeRecord || locked || loading) return;
          record.numerator = [];
          record.denominator = [];
          record.fraction = false;
          record.cursors = { numerator: 0, denominator: 0 };
          record.source = "";
          record.sourceError = "";
          changed(record);
          calculatorModal.close();
        }, "formula-calculator-save");
        actions.append(record.remove);
      }
      record.save = button("Save", () => void saveCalculator(record), "formula-calculator-save");
      record.save.id = "formula-calculator-save";
      actions.append(record.save);
    }
    if (view.unsupported) host.append(actions);
    if (!view.unsupported) {
      const source=make('textarea','formula-latex-input');source.setAttribute('aria-label','LaTeX source');source.spellcheck=false;source.hidden=true;record.sourceInput=source;
      const toggle=button('',()=>{
        record.resetPropertyPicker?.();
        record.sourceMode=!record.sourceMode;
        if(record.sourceMode && record.source === undefined){try{record.source=record.fraction?math.toLatex(record,view.properties,true):record.numerator.length?math.toLatex(record.numerator,view.properties):'';}catch(_){record.source=record.numerator.map(t=>tokenLabel(t,view)).join(' ');}}
        source.value=record.source||'';
        updateDisplay(record);(record.sourceMode?source:record.display).focus();
      },'formula-render-toggle');
      toggle.tabIndex=-1;
      toggle.classList.add('neutral-icon-button');
      toggle.append(root.VisibilityIcon.create());
      record.renderToggle=toggle;
      record.variableActions.append(toggle);
      record.displaySlot.insertBefore(source,record.display);
      const scrollbars = [record.display, source].map(scrollArea => UI.Scrollbar.attach(scrollArea));
      UI.Components.component(record.fieldset, {}, undefined, () => scrollbars.forEach(controller => controller.destroy()));
      source.addEventListener('input',()=>{
        if (!record.sourceMode || locked || loading || modalSaving) { source.value=record.source||''; return; }
        record.source=source.value;record.sourceError='';
        try{const tokens=source.value.trim()?math.parseLatex(source.value,view.properties):[];const fraction=parseFraction(tokens);record.numerator=fraction?fraction.numerator:tokens;record.denominator=fraction?fraction.denominator:[];record.fraction=!!fraction;record.cursors={numerator:record.numerator.length,denominator:record.denominator.length};}
        catch(error){record.sourceError=error.message;}
        changed(record);
      });
      const utility=record.fieldset.querySelector('.formula-calculator-utility');utility.prepend(record.status);
      source.value=record.source||'';
      updateDisplay(record);
      record.fieldset.addEventListener('mousedown', event => {
        if (event.target.closest('button,[role="option"]')) event.preventDefault();
      });
    }
    updateSaveActions(record);
  }
  function filter() { browser?.filter(); }
  function updateFilterOptions() { browser?.refresh(); }
  function attachSharedControls() {
    let empty = el("formula-no-matches");
    if (!empty) { empty = make("p", "input-hint"); empty.id = "formula-no-matches"; contents.after(empty); }
    browser = UI.CatalogBrowser.attach({ prefix: "formula", toolbar: el("formulas-page").querySelector(".formula-toolbar"), records: () => records, empty });
    if (UI.PickControl && displayPickerRoot) displayControl = UI.Components.Picker({ root: displayPickerRoot });
  }
  function render() {
    if (!initialized || !contents) return;
    if (calculatorModal && calculatorModal.isOpen()) {
      updateTimestamp();
      return;
    }
    UI.Components.destroyTree(contents);
    contents.replaceChildren();
    records = [];
    syncDisplayPicker();
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
    views.forEach(view => {
      let record;
      const { item, row } = UI.CatalogBrowser.row({ name: view.name,
        metadata: view.space + " · " + view.source + " · " + view.kind,
        warning: view.unsupported, onSelect: () => openCalculator(record) });
      row.setAttribute("aria-pressed", "false");
      record = { view, item, row, key: identity(view.target) };
      records.push(record);
      contents.append(item);
    });
    if (!views.length) {
      UI.CatalogBrowser.empty(contents, { loading: discovering || !discovered, loadingLabel: "Loading Kanban views…", emptyLabel: "No accessible Kanban views found." });
    }
    let empty = el("formula-no-matches");
    if (!empty) {
      empty = make("p", "input-hint");
      empty.id = "formula-no-matches";
      contents.append(empty);
    }
    empty.textContent = "No matching Kanban views.";
    empty.hidden = true;
    updateFilterOptions();
    filter();
    syncLocks();
    updateTimestamp();
  }
  async function discover() {
    if (!invoke) return;
    if (loading || locked) { discoveryRequested = true; return; }
    if (discovering) return;
    const generation = ++discoveryGeneration;
    discoveryRequested = false;
    discovering = true;
    setMessage(loadError);
    syncLocks();
    try {
      const result = await invoke("discover_kanban_views");
      if (!result || !Array.isArray(result.views)) throw Error("Anytype did not return a view list.");
      if (generation !== discoveryGeneration) return;
      catalog = result.views;
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
      if (generation === discoveryGeneration) setMessage(String(error.message || error));
    } finally {
      if (generation === discoveryGeneration) {
        discovering = false;
        render();
      }
    }
  }
  function configureModal() {
    if (!modalBackdrop || !modalDialog || !modalContent || !UI.createModalController) return;
    calculatorModal = UI.Components.Modal({
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
        UI.Components.destroyTree(modalContent);
        modalContent.replaceChildren();
        updateRows();
        render();
      },
    });
    modalDialog.addEventListener("keydown", event => {
      if (event.key !== "Tab" || modalBackdrop.hidden) return;
      const focusable = [activeRecord && activeRecord.display, activeRecord && activeRecord.sourceMode && activeRecord.sourceInput]
        .filter(element => element && !element.hidden && !element.disabled);
      if (!focusable.length) return;
      event.preventDefault();
      const index = focusable.indexOf(doc.activeElement);
      const next = event.shiftKey
        ? index <= 0 ? focusable.length - 1 : index - 1
        : index < 0 || index === focusable.length - 1 ? 0 : index + 1;
      focusable[next].focus({ preventScroll: true });
    }, true);
    if (modalClose) modalClose.addEventListener("click", () => calculatorModal.close());
  }
  UI.FormulaPage = {
    init(options) {
      api = options;
      initialized = true;
      const nav = el("formulas-nav"), open = el("open-formulas"), title = el("page-title");
      if (nav) nav.addEventListener("click", () => api.navigate("formulas"));
      if (open) open.addEventListener("click", () => { api.navigate("formulas"); if (title) title.focus({ preventScroll: true }); });
      if (preference) preference.addEventListener("change", () => {
        if (locked || loading) return;
        state.display = ["balanced", "spacious"].includes(preference.value) ? preference.value : "balanced";
        syncDisplayPicker();
        api.onChange();
      });
      if (UI.AppReload) {
        reloadController = UI.AppReload;
        reloadController.bindButton(reload);
        unregisterReload = reloadController.register(discover);
      }
      configureModal();
      attachSharedControls();
      render();
    },
    read: () => copy(state),
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
      browser?.reset();
      draftGeneration++;
      if (calculatorModal?.isOpen()) calculatorModal.close({ restoreFocus: false });
      localDrafts.clear();
      try {
        const result = await invoke("load_kanban_formulas");
        if (result) {
          if (!result.configuration || result.configuration.version !== 2 || !Array.isArray(result.configuration.definitions)
            || !["balanced", "spacious"].includes(result.configuration.display)) throw Error("Unsupported configuration");
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
        if (reloadController) void reloadController.run();
        else void discover();
      }
      return !loadError;
    },
    async save(bridge, submitted) {
      try {
        const result = await bridge("save_kanban_formulas", { configuration: submitted });
        (result?.info || []).forEach(message => api?.onInfo?.("Anytype API. " + message));
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
      if (wasLocked && !locked && !loading && discoveryRequested) {
        void discover();
      }
    },
    refresh() { if (!discovered && !discovering && !loading) return reloadController ? reloadController.run() : discover(); },
    reload() {
      discovered = false;
      discoveryRequested = true;
      return reloadController ? reloadController.run() : !discovering && !loading && !locked ? discover() : undefined;
    },
    destroy() {
      browser?.destroy();
      if (displayControl) displayControl.destroy();
      unregisterReload?.();
      if (reloadController) reloadController.bindButton(null);
      if (calculatorModal) calculatorModal.close({ restoreFocus: false });
    },
  };
})(window);
