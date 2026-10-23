(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {}, doc = root.document;
  const defaults = () => ({ version: 2, display: "balanced", definitions: [] });
  const copy = v => JSON.parse(JSON.stringify(v));
  const identity = t => JSON.stringify([t.spaceId, t.objectId, t.blockId, t.viewId]);
  const el = id => doc.getElementById(id);
  const contents = el("formula-contents"), message = el("formula-storage-message"), preference = el("formula-display");
  const fields = el("formula-fields"), search = el("formula-search"), reload = el("formula-reload"), refreshButton = el("formula-refresh-results");
  const operations = { total: "Total", average: "Average", minimum: "Minimum", maximum: "Maximum" };
  let state = defaults(), catalog = [], api, invoke, disclosure, locked = false, dirty = false, loading = true;
  let discovering = false, discovered = false, records = [], initialized = false, calculatedAt = null, loadError = "";
  const expanded = new Set();
  function make(tag, className = "", text) { const node = doc.createElement(tag); node.className = className; if (text !== undefined) node.textContent = text; return node; }
  function button(text, action, className = "formula-button") { const node = make("button", className, text); node.type = "button"; node.addEventListener("click", action); return node; }
  function definition(target) { return state.definitions.find(item => identity(item.target) === identity(target)); }
  function label(token, view) {
    if (token.kind === "number") return token.value;
    if (token.kind === "aggregate") return `${operations[token.operation]} ${view.properties.find(p => p.key === token.property)?.name || "Unavailable property"}`;
    if (token.kind === "count") return "Card count";
    if (token.kind === "operator") return ({ "*": "×", "/": "÷", "-": "−" })[token.value] || token.value;
    if (token.kind === "function") return token.value === "sqrt" ? "√" : "ⁿ√";
    return ({ open: "(", close: ")", comma: "," })[token.kind] || "?";
  }
  function updateTimestamp() {
    const date = calculatedAt && new Date(calculatedAt);
    el("formula-calculated-at").textContent = date && !Number.isNaN(date.valueOf()) ? `Last calculated ${date.toLocaleString()}. Press Ctrl+R in Anytype after saving or refreshing.` : "Save a calculation, then press Ctrl+R in Anytype to show its results.";
  }
  function syncLocks() { fields.disabled = locked || loading; preference.disabled = locked || loading; reload.disabled = locked || loading || discovering; refreshButton.disabled = locked || loading || dirty || !state.definitions.length; }
  function updateDisplay(record) {
    if (record.remove) record.remove.hidden = !record.view.unsupported || !record.tokens.length;
    record.display.replaceChildren();
    if (!record.tokens.length) record.display.append(make("span", "formula-placeholder", "Enter a calculation"));
    for (let index = 0; index <= record.tokens.length; index++) {
      if (index === record.cursor) record.display.append(make("span", "formula-caret", "│"));
      if (index < record.tokens.length) record.display.append(make("span", ["aggregate", "count"].includes(record.tokens[index].kind) ? "formula-term" : "formula-symbol", label(record.tokens[index], record.view)));
    }
    record.display.setAttribute("aria-label", `Calculation, ${record.tokens.map(t => label(t, record.view)).join(" ") || "empty"}`);
    record.display.scrollLeft = record.display.scrollWidth;
  }
  function changed(record) {
    record.generation++;
    state.definitions = state.definitions.filter(item => identity(item.target) !== identity(record.view.target));
    if (record.tokens.length) state.definitions.push({ target: copy(record.view.target), tokens: copy(record.tokens) });
    state.definitions.sort((a, b) => identity(a.target).localeCompare(identity(b.target)));
    record.status.textContent = ""; updateDisplay(record); api.onChange();
  }
  function edit(record, action) {
    if (locked || loading || record.view.unsupported) return;
    const tokens = record.tokens;
    if (/^[0-9.]$/.test(action)) {
      const previous = tokens[record.cursor - 1];
      if (previous?.kind === "number") { if ((action === "." && previous.value.includes(".")) || previous.value.length >= 32) return; previous.value += action; }
      else if (tokens.length < 256) tokens.splice(record.cursor++, 0, { kind: "number", value: action === "." ? "0." : action });
    } else if (action === "clear") { tokens.length = 0; record.cursor = 0; }
    else if (action === "backspace") {
      if (!record.cursor) return;
      const previous = tokens[record.cursor - 1];
      if (previous.kind === "number" && previous.value.length > 1) previous.value = previous.value.slice(0, -1); else tokens.splice(--record.cursor, 1);
    } else if (action === "delete") tokens.splice(record.cursor, 1);
    else {
      const inserted = ["sqrt", "root"].includes(action) ? [{ kind: "function", value: action }, { kind: "open" }] : [({ "(": { kind: "open" }, ")": { kind: "close" }, ",": { kind: "comma" } })[action] || { kind: "operator", value: action }];
      if (tokens.length + inserted.length > 256) return;
      tokens.splice(record.cursor, 0, ...inserted); record.cursor += inserted.length;
    }
    changed(record); record.display.focus({ preventScroll: true });
    if (action === "root") record.status.textContent = "Nth root uses (value, degree). Enter the value, comma, degree, then close the parenthesis.";
  }
  async function evaluate(record) {
    if (locked || loading || record.view.unsupported || record.evaluating || !record.tokens.length) return;
    const request = ++record.generation; record.evaluating = true; record.equals.disabled = true; record.status.textContent = "Calculating categories…";
    try {
      const result = await invoke("evaluate_kanban_formula", { target: record.view.target, tokens: copy(record.tokens) });
      if (request === record.generation && record.display.isConnected) record.status.textContent = `Calculation is valid for ${result.categories} ${result.categories === 1 ? "category" : "categories"}. Save changes to apply it.`;
    } catch (error) { if (request === record.generation && record.display.isConnected) record.status.textContent = String(error.message || error); }
    finally { record.evaluating = false; record.equals.disabled = false; }
  }
  function createView(view, index) {
    const item = make("section", "settings-list formula-view"), key = identity(view.target);
    const toggle = button("", () => {}, "formula-view-toggle"); toggle.dataset.disclosureToggle = "";
    const panel = make("div", "formula-view-panel"); panel.id = `formula-panel-${index}`; panel.dataset.disclosurePanel = ""; panel.hidden = !expanded.has(key);
    toggle.setAttribute("aria-controls", panel.id); toggle.setAttribute("aria-expanded", String(!panel.hidden));
    toggle.append(make("span", "formula-view-name", view.name), make("span", "formula-source", `${view.space} · ${view.source} · ${view.kind}`), make("span", "formula-chevron", "⌄"));
    const tokens = copy(definition(view.target)?.tokens || []), display = make("div", "formula-calculation"), status = make("p", "formula-calculation-status");
    display.tabIndex = view.unsupported ? -1 : 0; display.setAttribute("role", "textbox"); display.setAttribute("aria-multiline", "false"); status.id = `formula-status-${index}`; status.setAttribute("role", "status"); status.setAttribute("aria-live", "polite"); display.setAttribute("aria-describedby", status.id);
    const record = { view, item, toggle, panel, tokens, display, status, cursor: tokens.length, generation: 0, evaluating: false };
    const controls = make("fieldset", "formula-calculator"); controls.disabled = !!view.unsupported; controls.append(make("legend", "sr-only", `Calculator for ${view.name}`), display);
    const insert = button("Insert property", () => {}); insert.dataset.disclosureToggle = "";
    const picker = make("div", "formula-property-picker"); picker.id = `formula-property-picker-${index}`; picker.dataset.disclosurePanel = ""; picker.hidden = true;
    insert.setAttribute("aria-controls", picker.id); insert.setAttribute("aria-expanded", "false");
    const propertyLabel = make("label", "", "Property"), property = make("select"); propertyLabel.htmlFor = property.id = `formula-property-${index}`;
    property.append(new Option("Choose a numeric property", "")); view.properties.forEach(p => property.append(new Option(p.name, p.key)));
    const aggregateLabel = make("label", "", "Aggregation"), aggregate = make("select"); aggregateLabel.htmlFor = aggregate.id = `formula-aggregate-${index}`;
    aggregate.hidden = aggregateLabel.hidden = true; aggregate.append(new Option("Choose aggregation", "")); Object.entries(operations).forEach(([value, name]) => aggregate.append(new Option(name, value)));
    const accept = button("Insert", () => {
      if (locked || !property.value || !aggregate.value || record.tokens.length >= 256) return;
      record.tokens.splice(record.cursor++, 0, { kind: "aggregate", property: property.value, operation: aggregate.value }); changed(record); disclosure.setExpanded(insert, false); display.focus();
    }); accept.disabled = true;
    property.addEventListener("change", () => { aggregate.hidden = aggregateLabel.hidden = !property.value; aggregate.value = ""; accept.disabled = true; });
    aggregate.addEventListener("change", () => { accept.disabled = !property.value || !aggregate.value; });
    picker.append(propertyLabel, property, aggregateLabel, aggregate, accept);
    picker.addEventListener("keydown", event => { if (event.key === "Escape") { event.preventDefault(); disclosure.setExpanded(insert, false); insert.focus(); } });
    const count = button("Card count", () => { if (locked || record.tokens.length >= 256) return; record.tokens.splice(record.cursor++, 0, { kind: "count" }); changed(record); display.focus(); });
    const variables = make("div", "formula-variable-actions"); variables.append(insert, count); controls.append(variables, picker);
    const keypad = make("div", "formula-keypad");
    const keys = [["C", "clear", "Clear calculation"], ["⌫", "backspace", "Backspace"], ["(", "(", "Open parenthesis"], [")", ")", "Close parenthesis"], ["7", "7"], ["8", "8"], ["9", "9"], ["÷", "/", "Divide"], ["4", "4"], ["5", "5"], ["6", "6"], ["×", "*", "Multiply"], ["1", "1"], ["2", "2"], ["3", "3"], ["−", "-", "Subtract"], ["0", "0"], [".", ".", "Decimal point"], ["xʸ", "^", "Power"], ["+", "+", "Add"], ["√", "sqrt", "Square root"], ["ⁿ√", "root", "Nth root"], [",", ",", "Root argument separator"], ["=", "evaluate", "Calculate categories"]];
    keys.forEach(([text, action, name]) => { const node = button(text, () => action === "evaluate" ? evaluate(record) : edit(record, action), "formula-key"); if (name) { node.setAttribute("aria-label", name); node.title = name; } if (action === "evaluate") record.equals = node; keypad.append(node); });
    display.addEventListener("keydown", event => {
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) { event.preventDefault(); record.cursor = event.key === "Home" ? 0 : event.key === "End" ? record.tokens.length : Math.max(0, Math.min(record.tokens.length, record.cursor + (event.key === "ArrowLeft" ? -1 : 1))); updateDisplay(record); return; }
      if (event.key === "Enter" || event.key === "=") { event.preventDefault(); void evaluate(record); return; }
      const action = ({ Backspace: "backspace", Delete: "delete", x: "*", "×": "*", "÷": "/" })[event.key] || event.key;
      if (/^[0-9.+\-*/^(),]$/.test(action) || ["backspace", "delete"].includes(action)) { event.preventDefault(); edit(record, action); }
    });
    controls.append(keypad);
    if (view.unsupported) panel.append(make("p", "field-error", view.unsupported));
    else if (tokens.some(t => t.kind === "aggregate" && !view.properties.some(p => p.key === t.property))) status.textContent = "A saved property is unavailable. Remove its term and choose a replacement.";
    const remove = button("Remove calculation", () => { if (locked) return; record.tokens.length = 0; record.cursor = 0; changed(record); status.textContent = "Calculation removed. Save changes to remove its published results."; });
    record.remove = remove;
    panel.append(controls, status, remove); item.append(toggle, panel); updateDisplay(record); records.push(record); return item;
  }
  function filter() { const query = search.value.trim().toLocaleLowerCase(); records.forEach(r => { r.item.hidden = !`${r.view.name} ${r.view.space} ${r.view.source} ${r.view.kind}`.toLocaleLowerCase().includes(query); }); const empty = el("formula-no-matches"); if (empty) empty.hidden = !records.length || records.some(r => !r.item.hidden); }
  function render() {
    if (!initialized) return;
    records.forEach(r => { r.generation++; }); disclosure?.destroy(); contents.replaceChildren(); records = []; preference.value = state.display;
    const views = catalog.slice();
    for (const saved of state.definitions) if (!views.some(v => identity(v.target) === identity(saved.target))) views.push({ target: saved.target, name: "Unavailable view", space: "Saved calculation", source: "Reload the list or remove this calculation", kind: "Unavailable", properties: [], unsupported: "This saved view is unavailable. Its calculation is retained until you remove it and save." });
    views.sort((a, b) => a.name.localeCompare(b.name) || a.space.localeCompare(b.space) || a.source.localeCompare(b.source));
    views.forEach((view, index) => contents.append(createView(view, index)));
    if (!views.length) contents.append(make("p", "input-hint", discovering ? "Loading Kanban views…" : discovered ? "No accessible Kanban views found." : "Load your Kanban views to add a calculation."));
    const empty = make("p", "input-hint", "No matching Kanban views."); empty.id = "formula-no-matches"; empty.hidden = true; contents.append(empty);
    disclosure = UI.Disclosure.attach(contents, { onToggle({ toggle, expanded: value }) { const r = records.find(r => r.toggle === toggle); if (r) { if (value) expanded.add(identity(r.view.target)); else expanded.delete(identity(r.view.target)); } } });
    filter(); syncLocks(); updateTimestamp();
  }
  async function discover() {
    if (!invoke || discovering || locked || loading) return;
    discovering = true; message.textContent = loadError; syncLocks(); if (!records.length) render(); reload.textContent = "Loading views…";
    try { const result = await invoke("discover_kanban_views"); if (!result || !Array.isArray(result.views)) throw Error("Anytype did not return a view list."); catalog = result.views; discovered = true; message.textContent = [loadError, result.warning].filter(Boolean).join(" "); }
    catch (error) { message.textContent = String(error.message || error); }
    finally { discovering = false; reload.textContent = "Reload views"; render(); }
  }
  UI.FormulaPage = {
    init(options) {
      api = options; initialized = true;
      el("formulas-nav").addEventListener("click", () => api.navigate("formulas")); el("open-formulas").addEventListener("click", () => { api.navigate("formulas"); el("page-title").focus({ preventScroll: true }); });
      preference.addEventListener("change", () => { if (!locked && !loading) { state.display = preference.value; api.onChange(); } }); search.addEventListener("input", filter); reload.addEventListener("click", discover);
      refreshButton.addEventListener("click", async () => {
        if (locked || dirty || !state.definitions.length) return;
        api.onBusyChange(true); message.textContent = "Refreshing saved calculations…";
        try { const result = await invoke("refresh_kanban_results"); calculatedAt = result.calculatedAt; message.textContent = "Results refreshed. Press Ctrl+R in Anytype to apply."; }
        catch (error) { message.textContent = `${String(error.message || error)} Previous published results are unchanged.`; }
        finally { updateTimestamp(); api.onBusyChange(false); }
      }); render();
    },
    read: () => copy(state),
    restore(value) { state = copy(value || defaults()); render(); },
    async load(bridge) {
      invoke = bridge;
      try { const result = await invoke("load_kanban_formulas"); if (result) { if (!result.configuration || result.configuration.version !== 2 || !Array.isArray(result.configuration.definitions) || !["compact", "balanced", "spacious"].includes(result.configuration.display)) throw Error("Unsupported configuration"); state = copy(result.configuration); calculatedAt = result.calculatedAt; } message.textContent = ""; }
      catch (_) { loadError = "Could not load saved formulas. The original file is unchanged. Make changes and Save to replace it, or restart to retry."; message.textContent = loadError; }
      loading = false; render();
    },
    async save(bridge, submitted) {
      try { const result = await bridge("save_kanban_formulas", { configuration: submitted }); loadError = ""; calculatedAt = result?.calculatedAt || calculatedAt; updateTimestamp(); message.textContent = "Formulas saved. Press Ctrl+R in Anytype to apply."; }
      catch (error) { message.textContent = `${String(error.message || error)} Your pending edits and previous published results are kept.`; throw Error("Kanban formulas could not be saved. Review the message on the Kanban View Formulas page and retry."); }
    },
    setLocked(value, pending = false) { locked = value; dirty = pending; syncLocks(); },
    refresh() { if (!discovered && !discovering && !loading) void discover(); },
    reload() { discovered = false; if (!el("formulas-page").hidden) void discover(); },
  };
})(window);
