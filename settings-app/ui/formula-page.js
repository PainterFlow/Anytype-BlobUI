(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {}, doc = root.document;
  const defaults = () => ({ version: 2, display: "balanced", definitions: [] });
  const copy = value => JSON.parse(JSON.stringify(value));
  const identity = target => JSON.stringify([target.spaceId, target.objectId, target.blockId, target.viewId]);
  const el = id => doc.getElementById(id);
  const contents = el("formula-contents"), message = el("formula-storage-message"), preference = el("formula-display");
  const fields = el("formula-fields"), search = el("formula-search"), reload = el("formula-reload");
  const filterButton = el("formula-filter"), filterRoot = el("formula-filter-control"), filterMenu = el("formula-filter-options");
  const searchRoot = el("formula-search-control"), searchToggle = el("formula-search-toggle");
  const displayPickerRoot = el("formula-display-select");
  const modalBackdrop = el("formula-calculator-backdrop"), modalDialog = el("formula-calculator-dialog");
  const modalClose = el("formula-calculator-close"), modalContent = el("formula-calculator-content");
  const operations = { total: "Total", average: "Average", minimum: "Minimum", maximum: "Maximum" };
  let state = defaults(), persistedState = defaults(), catalog = [], api, invoke, calculatorModal;
  let locked = false, dirty = false, loading = true, discovering = false, discovered = false;
  let records = [], initialized = false, loadError = "", activeRecord = null, modalSaving = false, calculatedAt = "";
  let selectedSpace = "", searchControl, filterControl, displayControl;

  function make(tag, className, text) {
    const node = doc.createElement(tag);
    node.className = className || "";
    if (text !== undefined) node.textContent = text;
    return node;
  }
  function button(text, action, className) {
    const node = make("button", className || "formula-button", text);
    node.type = "button";
    node.addEventListener("click", action);
    return node;
  }
  function targetDefinition(target, source) {
    return (source || state).definitions.find(item => identity(item.target) === identity(target));
  }
  function tokenLabel(token, view) {
    if (token.kind === "number") return token.value;
    if (token.kind === "aggregate") {
      const property = (view.properties || []).find(item => item.key === token.property);
      return (operations[token.operation] || "Aggregate") + " " + (property ? property.name : "Unavailable property");
    }
    if (token.kind === "count") return "Card count";
    if (token.kind === "operator") return ({ "*": "×", "/": "÷", "-": "−" })[token.value] || token.value;
    if (token.kind === "function") return token.value === "sqrt" ? "√" : "ⁿ√";
    return ({ open: "(", close: ")", comma: "," })[token.kind] || "?";
  }
  function updateTimestamp() {
    const target = el("formula-calculated-at");
    if (!target) return;
    const timestamp = calculatedAt ? new Date(calculatedAt) : null;
    const formatted = timestamp && !Number.isNaN(timestamp.valueOf()) ? timestamp.toLocaleString() : "Not calculated yet";
    target.textContent = "Last calculated: " + formatted + ". Saved calculations are applied after you press Ctrl+R in Anytype.";
  }
  function definitionIsSaved(target) { return !!targetDefinition(target, persistedState); }
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
  function hasSaveableDefinition(record) {
    return serializedTokens(record).length > 0 || definitionIsSaved(record.view.target);
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
    if (record.remove) record.remove.hidden = !hasSaveableDefinition(record);
    if (record.save) record.save.disabled = locked || loading || modalSaving || !hasSaveableDefinition(record)
      || (!record.view.unsupported && record.fraction && (!record.numerator.length || !record.denominator.length));
    if (record.actions) record.actions.hidden = !record.view.unsupported && (!record.remove || record.remove.hidden);
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
    const saved = targetDefinition(record.view.target);
    const savedTokens = copy(saved ? saved.tokens : []), fraction = parseFraction(savedTokens);
    activeRecord = {
      view: record.view,
      viewKey: record.key,
      row: record.row,
      numerator: fraction ? fraction.numerator : savedTokens,
      denominator: fraction ? fraction.denominator : [],
      fraction: !!fraction,
      cursors: { numerator: 0, denominator: 0 },
      active: "numerator",
      fieldset: null, display: null, status: null, save: null, remove: null,
      generation: 0,
    };
    activeRecord.cursors.numerator = activeRecord.numerator.length;
    activeRecord.cursors.denominator = activeRecord.denominator.length;
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
    record.linearDisplay.hidden = record.fraction;
    record.fractionDisplay.hidden = !record.fraction;
    record.display.dataset.fraction = String(record.fraction);
    renderSequence(record, record.fraction ? record.numeratorDisplay : record.linearDisplay, record.numerator, record.cursors.numerator, "numerator", "x");
    if (record.fraction) renderSequence(record, record.denominatorDisplay, record.denominator, record.cursors.denominator, "denominator", "y");
    else { record.numeratorDisplay.replaceChildren(); record.denominatorDisplay.replaceChildren(); }
    record.display.dataset.active = record.active;
    record.numeratorDisplay.classList.toggle("is-active", record.fraction && record.active === "numerator");
    record.linearDisplay.classList.toggle("is-active", !record.fraction && record.active === "numerator");
    record.denominatorDisplay.classList.toggle("is-active", record.active === "denominator");
    const topLabel = record.numerator.map(token => tokenLabel(token, record.view)).join(" ") || "empty";
    const bottomLabel = record.denominator.map(token => tokenLabel(token, record.view)).join(" ") || "empty";
    record.display.setAttribute("aria-label", record.fraction
      ? "Calculation numerator " + topLabel + ", denominator " + bottomLabel + ". Use arrow up or down to change fields."
      : "Calculation " + topLabel + ". Press x/y to enter a fraction.");
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
    const before = recordSnapshot(record);
    if (record.active === "denominator") activateFraction(record);
    const tokens = sectionTokens(record), cursor = record.cursors[record.active];
    if (action === "fraction") activateFraction(record);
    else if (/^[0-9.]$/.test(action)) insertNumeric(record, action);
    else if (action === "clear") {
      tokens.length = 0;
      record.cursors[record.active] = 0;
    } else if (action === "backspace") {
      if (!cursor) { restoreRecordSnapshot(record, before); return; }
      const previous = tokens[cursor - 1];
      if (previous.kind === "number" && previous.value.length > 1) previous.value = previous.value.slice(0, -1);
      else { tokens.splice(cursor - 1, 1); record.cursors[record.active]--; }
    } else if (action === "delete") {
      if (cursor >= tokens.length) { restoreRecordSnapshot(record, before); return; }
      tokens.splice(cursor, 1);
    } else if (action === "sign") toggleSign(record);
    else if (action === "square") {
      if (!isValueBefore(tokens, record.cursors[record.active])) {
        restoreRecordSnapshot(record, before);
        setActiveStatus(record, "Enter a value before squaring it.");
        return;
      }
      tokens.splice(record.cursors[record.active], 0, { kind: "operator", value: "^" }, { kind: "number", value: "2" });
      record.cursors[record.active] += 2;
    } else {
      const inserted = action === "sqrt" || action === "root"
        ? [{ kind: "function", value: action }, { kind: "open" }]
        : action === "(" ? [{ kind: "open" }]
          : action === ")" ? [{ kind: "close" }]
            : action === "," ? [{ kind: "comma" }]
              : [{ kind: "operator", value: action }];
      tokens.splice(record.cursors[record.active], 0, ...inserted);
      record.cursors[record.active] += inserted.length;
    }
    if (serializedTokens(record).length > 256 || sectionTokens(record).some(token => token.kind === "number" && token.value.length > 32)) {
      restoreRecordSnapshot(record, before);
      setActiveStatus(record, "Use at most 256 calculator terms and 32 characters per number.");
      updateDisplay(record);
      return;
    }
    setActiveStatus(record, "");
    if (!changed(record)) {
      restoreRecordSnapshot(record, before);
      setActiveStatus(record, "Use at most 256 calculator terms.");
      updateDisplay(record);
      return;
    }
    record.display.focus({ preventScroll: true });
  }
  function setActiveStatus(record, text) { if (record && record.status) record.status.textContent = text || ""; }
  function handleDisplayKeydown(record, event) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const cursor = record.cursors[record.active], length = sectionTokens(record).length;
      record.cursors[record.active] = event.key === "Home" ? 0 : event.key === "End" ? length
        : Math.max(0, Math.min(length, cursor + (event.key === "ArrowLeft" ? -1 : 1)));
      updateDisplay(record);
      return;
    }
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      if (event.key === "ArrowDown") {
        activateFraction(record);
        record.active = "denominator";
        updateSaveActions(record);
      }
      else if (record.fraction) record.active = "numerator";
      updateDisplay(record);
      return;
    }
    if (event.key === "Enter" || event.key === "=") {
      event.preventDefault();
      void saveCalculator(record);
      return;
    }
    const action = ({ Backspace: "backspace", Delete: "delete", x: "*", "×": "*", "÷": "/" })[event.key] || event.key;
    if (/^[0-9.+\-*/^(),]$/.test(action) || ["backspace", "delete"].includes(action)) {
      event.preventDefault();
      edit(record, action);
    }
  }
  function displayTokenClick(record, section, event) {
    const target = event.target.closest("[data-token-index]");
    record.active = section;
    if (target) {
      const index = Number(target.dataset.tokenIndex), bounds = target.getBoundingClientRect();
      record.cursors[section] = index + (event.clientX >= bounds.left + bounds.width / 2 ? 1 : 0);
    } else record.cursors[section] = section === "numerator" ? record.numerator.length : record.denominator.length;
    updateDisplay(record);
    record.display.focus({ preventScroll: true });
  }
  function insertVariable(record, token) {
    if (locked || loading || modalSaving || record.view.unsupported) return;
    const before = recordSnapshot(record);
    if (record.active === "denominator") activateFraction(record);
    const tokens = sectionTokens(record), cursor = record.cursors[record.active];
    tokens.splice(cursor, 0, copy(token));
    record.cursors[record.active]++;
    if (serializedTokens(record).length > 256 || !changed(record)) {
      restoreRecordSnapshot(record, before);
      setActiveStatus(record, "Use at most 256 calculator terms.");
      updateDisplay(record);
      return;
    }
    setActiveStatus(record, "");
    record.display.focus({ preventScroll: true });
  }
  function openPropertyPicker(record) {
    if (!record.propertyPanel || locked || record.view.unsupported) return;
    record.propertyPanel.hidden = false;
    record.propertyButton.setAttribute("aria-expanded", "true");
    record.propertySelect.focus({ preventScroll: true });
  }
  function closePropertyPicker(record) {
    if (!record.propertyPanel) return;
    record.propertyPanel.hidden = true;
    record.propertyButton.setAttribute("aria-expanded", "false");
  }
  function createPropertyControls(record, host) {
    const area = make("div", "formula-variable-actions");
    record.propertyButton = button("Insert property", () => openPropertyPicker(record), "formula-insert-property");
    record.propertyButton.setAttribute("aria-expanded", "false");
    record.propertyButton.setAttribute("aria-controls", "formula-property-panel");
    const count = button("Card count", () => insertVariable(record, { kind: "count" }), "formula-variable-button");
    area.append(record.propertyButton, count);
    const panel = make("div", "formula-property-picker");
    panel.id = "formula-property-panel";
    panel.hidden = true;
    panel.setAttribute("aria-label", "Insert numeric property");
    const propertyLabel = make("label", "", "Property"), property = make("select");
    property.id = "formula-property-select";
    propertyLabel.htmlFor = property.id;
    property.append(new Option("Choose a numeric property", ""));
    (record.view.properties || []).forEach(item => property.append(new Option(item.name, item.key)));
    const aggregateLabel = make("label", "", "Aggregation"), aggregate = make("select");
    aggregate.id = "formula-aggregate-select";
    aggregateLabel.htmlFor = aggregate.id;
    aggregate.append(new Option("Choose aggregation", ""));
    Object.entries(operations).forEach(([value, name]) => aggregate.append(new Option(name, value)));
    aggregate.hidden = aggregateLabel.hidden = true;
    const insert = button("Insert", () => {
      if (!property.value || !aggregate.value) return;
      insertVariable(record, { kind: "aggregate", property: property.value, operation: aggregate.value });
      property.value = ""; aggregate.value = "";
      aggregate.hidden = aggregateLabel.hidden = true;
      closePropertyPicker(record);
    }, "formula-property-insert");
    insert.disabled = true;
    property.addEventListener("change", () => {
      aggregate.value = "";
      aggregate.hidden = aggregateLabel.hidden = !property.value;
      insert.disabled = !property.value || !aggregate.value;
    });
    aggregate.addEventListener("change", () => { insert.disabled = !property.value || !aggregate.value; });
    panel.addEventListener("keydown", event => {
      if (event.key === "Escape") { event.preventDefault(); closePropertyPicker(record); record.propertyButton.focus(); }
    });
    panel.append(propertyLabel, property, aggregateLabel, aggregate, insert);
    record.propertyPanel = panel;
    record.propertySelect = property;
    host.append(area, panel);
  }
  async function saveCalculator(record) {
    if (!record || record !== activeRecord || locked || loading || modalSaving) return;
    if (!record.view.unsupported && record.fraction && (!record.numerator.length || !record.denominator.length)) {
      setActiveStatus(record, "Enter both a numerator and denominator before saving the fraction.");
      return;
    }
    if (!hasSaveableDefinition(record)) return;
    if (typeof api.onSave !== "function") {
      setActiveStatus(record, "Use Save changes in Settings to publish this calculation.");
      api.onChange();
      return;
    }
    modalSaving = true;
    if (record.save) record.save.textContent = "Saving…";
    syncLocks();
    setActiveStatus(record, "Saving calculation…");
    try {
      const committed = await api.onSave();
      if (committed === true) calculatorModal.close();
      else setActiveStatus(record, "The calculation is still pending. Review the Save status and try again.");
    } catch (error) {
      setActiveStatus(record, String(error && error.message || error));
    } finally {
      modalSaving = false;
      if (record.save && record.save.isConnected) record.save.textContent = "Save";
      syncLocks();
    }
  }
  function removeCalculation(record) {
    if (!record || locked || loading || modalSaving) return;
    record.numerator = [];
    record.denominator = [];
    record.fraction = false;
    record.cursors = { numerator: 0, denominator: 0 };
    record.active = "numerator";
    changed(record);
    setActiveStatus(record, "Calculation removed from the draft. Save to publish its removal.");
    updateDisplay(record);
  }
  function buildCalculator(record) {
    const view = record.view, host = modalContent;
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
      display.tabIndex = 0;
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
      fieldset.append(display);
      createPropertyControls(record, fieldset);
      const utility = make("div", "formula-calculator-utility");
      [["C", "clear", "Clear calculation"], ["⌫", "backspace", "Backspace"]].forEach(([label, action, accessible]) => {
        const key = button(label, () => edit(record, action), "formula-key");
        key.setAttribute("aria-label", accessible); key.title = accessible;
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
      keys.forEach(([label, action, accessible]) => {
        const key = button(label, () => {
          if (action === "save") void saveCalculator(record);
          else edit(record, action);
        }, "formula-key");
        if (accessible) { key.setAttribute("aria-label", accessible); key.title = accessible; }
        if (action === "save") { key.id = "formula-calculator-save"; record.save = key; }
        keypad.append(key);
      });
      fieldset.append(keypad);
      host.append(fieldset);
      record.status = make("p", "formula-calculator-status");
      record.status.id = "formula-calculator-status";
      record.status.setAttribute("role", "status");
      record.status.setAttribute("aria-live", "polite");
      display.setAttribute("aria-describedby", record.status.id);
      if ([...record.numerator, ...record.denominator].some(token => token.kind === "aggregate" && !(view.properties || []).some(item => item.key === token.property))) {
        record.status.textContent = "A saved property is unavailable. Remove the calculation and choose a replacement.";
      }
      host.append(record.status);
    }
    const actions = make("div", "formula-calculator-actions");
    record.actions = actions;
    record.remove = button("Remove calculation", () => removeCalculation(record), "formula-remove-calculation");
    record.remove.hidden = !hasSaveableDefinition(record);
    actions.append(record.remove);
    if (view.unsupported) {
      record.save = button("Save", () => void saveCalculator(record), "formula-calculator-save");
      record.save.id = "formula-calculator-save";
      actions.append(record.save);
    }
    if (record.remove.hidden && !view.unsupported) actions.hidden = true;
    host.append(actions);
    if (!view.unsupported) updateDisplay(record);
    updateSaveActions(record);
  }
  function filter() {
    const query = (search ? search.value : "").trim().toLocaleLowerCase();
    records.forEach(record => {
      const matchesSpace = !selectedSpace || record.view.target.spaceId === selectedSpace;
      const searchable = (record.view.name + " " + record.view.space + " " + record.view.source + " " + record.view.kind).toLocaleLowerCase();
      record.item.hidden = !matchesSpace || !searchable.includes(query);
    });
    const empty = el("formula-no-matches");
    if (empty) empty.hidden = !records.length || records.some(record => !record.item.hidden);
    if (filterButton) {
      const filtered = !!selectedSpace;
      filterButton.classList.toggle("is-filtered", filtered);
      filterButton.dataset.filtered = String(filtered);
      filterButton.setAttribute("aria-pressed", String(filtered));
      const chosen = records.find(record => record.view.target.spaceId === selectedSpace);
      filterButton.title = filtered ? "Filtered to " + (chosen ? chosen.view.space : selectedSpace) : "Filter by space";
    }
  }
  function updateFilterOptions() {
    if (!filterControl) return;
    const spaces = new Map();
    records.forEach(record => {
      const id = record.view.target.spaceId;
      if (id) spaces.set(id, record.view.space || "Unnamed space");
    });
    const nameCounts = new Map();
    spaces.forEach(name => nameCounts.set(name, (nameCounts.get(name) || 0) + 1));
    const options = [{ value: "", label: "All spaces" }, ...[...spaces.entries()]
      .sort((left, right) => left[1].localeCompare(right[1]) || left[0].localeCompare(right[0]))
      .map(([id, name]) => ({ value: id, label: nameCounts.get(name) > 1 ? name + " · " + id.slice(0, 6) : name }))];
    if (selectedSpace && !spaces.has(selectedSpace)) selectedSpace = "";
    filterControl.setOptions(options);
    filterControl.setValue(selectedSpace);
  }
  function attachSharedControls() {
    if (UI.ExpandableSearch && searchRoot && searchToggle && search) {
      searchControl = UI.ExpandableSearch.attach({ container: searchRoot, button: searchToggle, input: search });
    }
    if (UI.PickControl && filterRoot && filterButton && filterMenu) {
      filterControl = UI.PickControl.attach({
        root: filterRoot,
        options: [{ value: "", label: "All spaces" }],
        value: selectedSpace,
        onChange(value) { selectedSpace = value || ""; filter(); },
      });
    }
    if (UI.PickControl && displayPickerRoot) displayControl = UI.PickControl.attach({ root: displayPickerRoot });
  }
  function render() {
    if (!initialized || !contents) return;
    if (calculatorModal && calculatorModal.isOpen()) calculatorModal.close({ restoreFocus: false });
    contents.replaceChildren();
    records = [];
    syncDisplayPicker();
    const views = catalog.slice();
    for (const saved of state.definitions) {
      if (!views.some(view => identity(view.target) === identity(saved.target))) {
        views.push({
          target: saved.target, name: "Unavailable view", space: "Saved calculation",
          source: "Reload the list or remove this calculation", kind: "Unavailable", properties: [],
          unsupported: "This saved view is unavailable. Its calculation is kept until you remove it and save.",
        });
      }
    }
    views.sort((left, right) => left.name.localeCompare(right.name) || left.space.localeCompare(right.space) || left.source.localeCompare(right.source));
    views.forEach(view => {
      const item = make("section", "settings-list formula-view");
      let record;
      const row = button("", () => openCalculator(record), "formula-view-row");
      row.setAttribute("aria-pressed", "false");
      row.append(make("span", "formula-view-name", view.name));
      row.append(make("span", "formula-source", view.space + " · " + view.source + " · " + view.kind));
      if (targetDefinition(view.target)) row.append(make("span", "formula-view-saved", "Calculation saved"));
      if (view.unsupported) row.append(make("span", "formula-view-warning", view.unsupported));
      item.append(row);
      record = { view, item, row, key: identity(view.target) };
      records.push(record);
      contents.append(item);
    });
    if (!views.length) {
      const text = discovering ? "Loading Kanban views…" : discovered ? "No accessible Kanban views found." : "Open this page to discover Kanban views.";
      contents.append(make("p", "input-hint formula-empty-message", text));
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
    if (!invoke || discovering || locked || loading) return;
    discovering = true;
    if (message) message.textContent = loadError;
    syncLocks();
    try {
      const result = await invoke("discover_kanban_views");
      if (!result || !Array.isArray(result.views)) throw Error("Anytype did not return a view list.");
      catalog = result.views;
      discovered = true;
      if (message) message.textContent = [loadError, result.warning].filter(Boolean).join(" ");
    } catch (error) {
      if (message) message.textContent = String(error.message || error);
    } finally {
      discovering = false;
      render();
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
        if (activeRecord && activeRecord.row) {
          activeRecord.row.dataset.active = "false";
          activeRecord.row.setAttribute("aria-pressed", "false");
        }
        activeRecord = null;
        modalSaving = false;
        modalContent.replaceChildren();
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
      const nav = el("formulas-nav"), open = el("open-formulas"), title = el("page-title");
      if (nav) nav.addEventListener("click", () => api.navigate("formulas"));
      if (open) open.addEventListener("click", () => { api.navigate("formulas"); if (title) title.focus({ preventScroll: true }); });
      if (preference) preference.addEventListener("change", () => {
        if (locked || loading) return;
        state.display = ["compact", "balanced", "spacious"].includes(preference.value) ? preference.value : "balanced";
        syncDisplayPicker();
        api.onChange();
      });
      if (search) search.addEventListener("input", filter);
      if (reload) reload.addEventListener("click", () => void discover());
      configureModal();
      attachSharedControls();
      render();
    },
    read: () => copy(state),
    isPersisted: () => JSON.stringify(state) === JSON.stringify(persistedState),
    restore(value) {
      state = copy(value || defaults());
      persistedState = copy(state);
      if (calculatorModal && calculatorModal.isOpen()) calculatorModal.close({ restoreFocus: false });
      render();
    },
    async load(bridge) {
      invoke = bridge;
      try {
        const result = await invoke("load_kanban_formulas");
        if (result) {
          if (!result.configuration || result.configuration.version !== 2 || !Array.isArray(result.configuration.definitions)
            || !["compact", "balanced", "spacious"].includes(result.configuration.display)) throw Error("Unsupported configuration");
          state = copy(result.configuration);
          persistedState = copy(state);
          calculatedAt = result.calculatedAt || "";
        }
        if (message) message.textContent = "";
      } catch (_) {
        state = defaults();
        persistedState = copy(state);
        loadError = "Could not load saved formulas. The original file is unchanged. Make changes and Save to replace it, or restart to retry.";
        if (message) message.textContent = loadError;
      }
      loading = false;
      render();
    },
    async save(bridge, submitted) {
      try {
        const result = await bridge("save_kanban_formulas", { configuration: submitted });
        persistedState = copy(submitted);
        calculatedAt = result && result.calculatedAt || calculatedAt;
        loadError = "";
        updateTimestamp();
        if (message) message.textContent = "Formulas saved. Press Ctrl+R in Anytype to apply.";
        return result;
      } catch (error) {
        if (message) message.textContent = String(error.message || error) + " Your pending edits and previous published results are kept.";
        throw Error("Kanban formulas could not be saved. Review the message on the Kanban View Formulas page and retry.");
      }
    },
    setLocked(value, pending) { locked = value; dirty = !!pending; syncLocks(); },
    refresh() { if (!discovered && !discovering && !loading) void discover(); },
    reload() { discovered = false; if (!el("formulas-page") || !el("formulas-page").hidden) void discover(); },
    destroy() {
      if (searchControl) searchControl.destroy();
      if (filterControl) filterControl.destroy();
      if (displayControl) displayControl.destroy();
      if (calculatorModal) calculatorModal.close({ restoreFocus: false });
    },
  };
})(window);
