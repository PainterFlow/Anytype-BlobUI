(function (root) {
  "use strict";
  const UI = root.BlobUI = root.BlobUI || {}, doc = root.document;
  function node(tag, className, text) {
    const value = doc.createElement(tag); value.className = className || "";
    if (text !== undefined) value.textContent = text;
    return value;
  }
  function row({ name, metadata, warning, onSelect }) {
    const item = node("section", "formula-view");
    const button = UI.Components.Button({ label: "", attributes: { class: "formula-view-row" }, onClick: onSelect }).element;
    button.append(node("span", "formula-view-name", name), node("span", "formula-source", metadata));
    if (warning) button.append(node("span", "formula-view-warning", warning));
    item.append(button);
    return { item, row: button };
  }
  function toolbar(prefix, label) {
    // One toolbar definition supplies the Kanban page and every catalog browser.
    const element = doc.querySelector("#formulas-page .formula-toolbar").cloneNode(true);
    element.querySelectorAll("[id]").forEach(value => { value.id = value.id.replace(/^formula/, prefix); });
    element.querySelectorAll("[for]").forEach(value => value.htmlFor = value.htmlFor.replace(/^formula/, prefix));
    element.querySelectorAll("[aria-controls]").forEach(value => value.setAttribute("aria-controls", value.getAttribute("aria-controls").replace(/^formula/, prefix)));
    const search = element.querySelector('input[type="search"]');
    search.value = ""; search.placeholder = label;
    element.querySelector("label").textContent = label;
    element.querySelector(".expandable-search").dataset.expanded = "false";
    search.tabIndex = -1;
    const reload = element.querySelector(`#${prefix}-reload`);
    reload.disabled = false; reload.setAttribute("aria-label", "Reload list"); reload.title = "Reload list";
    element.querySelector(`#${prefix}-filter-options`).replaceChildren();
    return element;
  }
  function attach({ prefix, toolbar: bar, records, empty, onReload }) {
    const find = suffix => bar.querySelector(`#${prefix}-${suffix}`);
    const search = find("search"), filter = find("filter"), reload = find("reload");
    let selectedSpace = "", locked = false;
    const searchControl = UI.Components.Search({ container: find("search-control"), button: find("search-toggle"), input: search });
    const filterControl = UI.Components.Picker({ root: find("filter-control"), options: [{ value: "", label: "All spaces" }], value: "", closeOnSelect: false,
      onChange(value) { selectedSpace = value || ""; apply(); } });
    function apply() {
      const query = search.value.trim().toLocaleLowerCase();
      const rows = records();
      rows.forEach(record => {
        const data = record.view;
        record.item.hidden = !!selectedSpace && data.target.spaceId !== selectedSpace
          || ![data.name, data.space, data.source, data.kind].filter(Boolean).join(" ").toLocaleLowerCase().includes(query);
      });
      if (empty) { empty.textContent = "No matching results."; empty.hidden = !rows.length || rows.some(record => !record.item.hidden); }
      filter.classList.toggle("is-filtered", !!selectedSpace);
      filter.dataset.filtered = String(!!selectedSpace);
      filter.setAttribute("aria-pressed", String(!!selectedSpace));
      filter.title = selectedSpace ? "Filtered to " + selectedSpace : "Filter by space";
    }
    function refresh() {
      const spaces = new Map(records().map(record => [record.view.target.spaceId, record.view.space || "Unnamed space"]));
      const counts = new Map(); spaces.forEach(label => counts.set(label, (counts.get(label) || 0) + 1));
      if (!spaces.has(selectedSpace)) selectedSpace = "";
      filterControl.setOptions([{ value: "", label: "All spaces" }, ...[...spaces].sort((a,b) => a[1].localeCompare(b[1]) || a[0].localeCompare(b[0]))
        .map(([value,label]) => ({ value, label: counts.get(label) > 1 ? label + " · " + value.slice(0,6) : label }))]);
      filterControl.setValue(selectedSpace); apply();
    }
    function click() { if (!locked) onReload?.(); }
    search.addEventListener("input", apply); reload.addEventListener("click", click);
    return { refresh, filter: apply,
      reset() { selectedSpace = ""; search.value = ""; refresh(); },
      setLocked(value, busy = false) { locked = value; search.disabled = value; filter.disabled = value || busy; reload.disabled = value || busy; reload.setAttribute("aria-busy", String(busy)); },
      destroy() { searchControl.destroy(); filterControl.destroy(); search.removeEventListener("input", apply); reload.removeEventListener("click", click); }
    };
  }
  function status(element, text, success = false) {
    element.textContent = text;
    element.dataset.status = success ? "success" : "error";
  }
  function empty(container, { loading = false, loadingLabel = "Loading…", emptyLabel = "No accessible results found." }) {
    container.append(node("p", "input-hint formula-empty-message", loading ? loadingLabel : emptyLabel));
  }
  UI.CatalogBrowser = { row, toolbar, attach, node, status, empty };
})(window);
