import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { SelectPicker } from "./shared/select-picker.jsx";
import { CatalogList, CatalogListRow, CatalogListToolbar, groupIntoSections } from "./shared/catalog-list.jsx";

/** React view for the saved Kanban view catalog. The controller owns discovery and persistence. */
export function FormulaPageView({
  controller,
  views = [], loading = false, locked = false, discovering = false, message = "", success = false,
  display = "balanced", calculatedAt = "", onDisplay = () => {}, onReload = () => {}, onSelect = () => {},
}) {
  const [model, setModel] = useState(() => controller?.getSnapshot?.() || null);
  useEffect(() => controller?.subscribe?.(setModel), [controller]);
  const data = model || { views, loading, locked, discovering, message, success, display, calculatedAt };
  const displayChange = controller ? value => controller.setDisplay(value) : onDisplay;
  const reloadViews = controller ? () => controller.reloadViews() : onReload;
  const selectView = controller ? (view, row) => controller.selectView(view.target, row) : onSelect;
  const [query, setQuery] = useState("");
  const [kindFilters, setKindFilters] = useState([]);
  const spaces = useMemo(() => {
    if (Array.isArray(data.spaceCatalog) && data.spaceCatalog.length) return data.spaceCatalog.map(row => [row.id, row.name || "Unnamed space", row.icon]);
    return [...new Map(data.views.map(view => [view.target?.spaceId, view.space || "Unnamed space"]))]
      .filter(([id]) => id).sort((a, b) => a[1].localeCompare(b[1]) || a[0].localeCompare(b[0]));
  }, [data.views, data.spaceCatalog]);
  const spaceEntries = useMemo(() => spaces.map(([id, name, icon]) => ({ id, name, icon })), [spaces]);
  const kindOf = view => view.kind === "Query" ? "query" : view.kind === "Collection" ? "collection" : "objectType";
  const matching = data.views.filter(view => (!kindFilters.length || kindFilters.includes(kindOf(view)))
    && [view.name, view.space, view.source, view.kind].filter(Boolean).join(" ").toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  const sections = useMemo(() => groupIntoSections(matching, spaceEntries, view => view.target?.spaceId), [matching, spaceEntries]);

  return <>
    <CatalogListToolbar prefix="formula" searchLabel="Search Kanban views" query={query} onQueryChange={setQuery}
      reloadLabel="Reload available views" onReload={reloadViews} locked={data.locked} busy={data.discovering}
      filterLabel="Filter by view type" filterMultiple filterCloseOnSelect={false} filterValue={kindFilters} onFilterChange={setKindFilters}
      filterOptions={[{ value: "objectType", label: "Object type" }, { value: "query", label: "Query" }, { value: "collection", label: "Collection" }]} />
    <p id="formula-storage-message" className="field-error" role="status" aria-live="polite" data-status={data.success ? "success" : "error"}>{data.message}</p>
    <fieldset id="formula-fields" aria-label="Kanban formulas" disabled={data.locked}>
      <div id="formula-contents">
        <CatalogList sections={sections} emptyClassName="formula-empty-message" emptyMessage={query || kindFilters.length ? "No matching Kanban views." : data.loading || data.discovering ? "Loading Kanban views…" : "No accessible Kanban views found."}
          renderItem={(view, index) => <CatalogListRow key={JSON.stringify(view.target || index)} icon={view.icon} title={view.name} meta={view.source}
            warning={view.unsupported} active={!!view.active} onClick={event => selectView(view, event.currentTarget)} />} />
      </div>
    </fieldset>
  </>;
}

const mathKeys = [
  ["√", "sqrt", "Square root"], ["(", "(", "Open parenthesis"], [")", ")", "Close parenthesis"], ["ⁿ√", "root", "Nth root"], ["÷", "/", "Divide"],
  ["xʸ", "^", "Power"], ["7", "7"], ["8", "8"], ["9", "9"], ["×", "*", "Multiply"],
  ["x²", "square", "Square"], ["4", "4"], ["5", "5"], ["6", "6"], ["−", "-", "Subtract"],
  [",", ",", "Comma"], ["1", "1"], ["2", "2"], ["3", "3"], ["+", "+", "Add"],
  ["x/y", "fraction", "Fraction"], ["±", "sign", "Change sign"], ["0", "0"], [".", ".", "Decimal point"], ["Save", "save", "Save calculation"],
];
const logicKeys = [
  ["=", "==", "Equal"], ["≠", "!=", "Not equal"], ["<", "<", "Less than"], ["≤", "<=", "Less than or equal"], ["Save", "save", "Save calculation"],
  [">", ">", "Greater than"], ["≥", ">=", "Greater than or equal"], ["AND", "&&", "Logical AND"], ["OR", "||", "Logical OR"], ["NOT", "!", "Logical NOT"],
  ["True", "true", "True"], ["False", "false", "False"], ["(", "(", "Open parenthesis"], [")", ")", "Close parenthesis"], ["?", "?", "Conditional question mark"],
  [":", ":", "Conditional colon"],
];
const aggregateOptions = [["total", "Total"], ["average", "Average"], ["minimum", "Minimum"], ["maximum", "Maximum"], ["median", "Median"]];

/** Calculator presentation is React-owned. The controller keeps token math and source selection. */
export function FormulaCalculatorView({ controller }) {
  const [model, setModel] = useState(() => controller?.getSnapshot?.() || null);
  const [keypadMode, setKeypadMode] = useState("Math");
  const sourceRef = useRef(null);
  const displayRef = useRef(null);
  const current = model?.active || null;
  const viewKey = current?.view?.target ? JSON.stringify(current.view.target) : "";
  useEffect(() => controller?.subscribe?.(setModel), [controller]);
  useLayoutEffect(() => {
    setKeypadMode("Math");
  }, [viewKey]);
  useLayoutEffect(() => {
    controller?.setCalculatorDisplay?.(displayRef.current);
    controller?.setCalculatorSourceInput?.(sourceRef.current);
    if (current) (current.sourceMode ? sourceRef.current : displayRef.current)?.focus({ preventScroll: true });
    return () => {
      controller?.setCalculatorDisplay?.(null);
      controller?.setCalculatorSourceInput?.(null);
    };
  }, [controller, viewKey, current?.sourceMode]);

  if (!current) return <div id="formula-calculator-content" />;
  const view = current.view;
  const statusId = "formula-calculator-status";
  const keys = keypadMode === "Logic" ? logicKeys : mathKeys;
  const chooseProperty = value => {
    controller.calculatorChooseProperty(value);
    return !controller.getSnapshot()?.active?.pendingProperty;
  };
  return <div id="formula-calculator-content">
    <div className="formula-calculator-heading">
      <h2 className="formula-calculator-title">{view.name}</h2>
      <p className="formula-calculator-source">{[view.space, view.source, view.kind].filter(Boolean).join(" · ")}</p>
    </div>
    {current.unsupported ? <p className="formula-calculator-error field-error">{current.unsupported}</p> : <fieldset className="formula-calculator" disabled={current.locked}>
      <legend className="sr-only">Calculator for {view.name}</legend>
      <div className="formula-display-slot">
        <div ref={displayRef} className="formula-display formula-rendered" hidden={current.sourceMode} tabIndex={0}
          role="textbox" aria-readonly="true" aria-multiline="false" aria-describedby={statusId}
          aria-label="Rendered calculation. Read only. Show LaTeX to edit."
          onKeyDown={event => { if (event.key === "Enter" || event.key === "=") { event.preventDefault(); controller.calculatorEdit("save"); } }}
          dangerouslySetInnerHTML={{ __html: current.markup || "" }} />
        <textarea ref={sourceRef} className="formula-latex-input" aria-label="LaTeX source" spellCheck={false}
          hidden={!current.sourceMode} value={current.source} onChange={event => controller.calculatorSourceChanged(event.currentTarget)} />
      </div>
      <div className="formula-variable-actions">
        <SelectPicker key={viewKey} menuId="formula-property-options" label={current.pendingProperty ? "Choose aggregation" : "Insert property"}
          caption={current.pendingProperty ? "Choose aggregation" : "Insert property"}
          rootClassName="formula-property-picker" inactive={!current.sourceMode}
          disabled={current.locked} preservePointerFocus restoreFocusOnSelect={false}
          emptyMessage="No number or boolean properties in this view"
          options={current.pendingProperty
            ? aggregateOptions.map(([value, label]) => ({ value, label }))
            : current.properties.map(property => ({ value: property.key, label: property.name, description: property.description }))}
          onChange={current.pendingProperty ? value => controller.calculatorChooseAggregation(value) : chooseProperty} />
        <button type="button" className="formula-variable-button" tabIndex={-1} onMouseDown={event => event.preventDefault()}
          onClick={() => controller.calculatorInsertCount()}>Card count</button>
        <button type="button" className="formula-render-toggle neutral-icon-button" aria-label={current.sourceMode ? "Show rendered" : "Show LaTeX"}
          title={current.sourceMode ? "Show rendered" : "Show LaTeX"} aria-pressed={current.sourceMode} tabIndex={-1}
          onMouseDown={event => event.preventDefault()} onClick={() => controller.toggleCalculatorSource()}>
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M2.5 10s2.7-5 7.5-5 7.5 5 7.5 5-2.7 5-7.5 5-7.5-5-7.5-5Z" fill="none" stroke="currentColor" strokeWidth="1.5" /><circle cx="10" cy="10" r="2.25" fill="none" stroke="currentColor" strokeWidth="1.5" /></svg>
        </button>
      </div>
      <div className="formula-calculator-utility">
        <p id={statusId} className="formula-calculator-status" role="status" aria-live="polite" title={current.status}>{current.status}</p>
        <button type="button" className="formula-key" aria-label="Clear calculation" title="Clear calculation" tabIndex={-1}
          onMouseDown={event => event.preventDefault()} onClick={() => controller.calculatorEdit("clear")}>C</button>
        <button type="button" className="formula-key" aria-label="Backspace" title="Backspace" tabIndex={-1}
          onMouseDown={event => event.preventDefault()} onClick={() => controller.calculatorEdit("backspace")}><span aria-hidden="true">⌫</span></button>
      </div>
      <div className="formula-keypad-modes" role="group" aria-label="Calculator keypad">
        {['Math', 'Logic'].map(mode => <button key={mode} type="button" className="formula-key" aria-pressed={keypadMode === mode}
          onMouseDown={event => event.preventDefault()} onClick={() => setKeypadMode(mode)}>{mode}</button>)}
      </div>
      <div className="formula-keypad">
        {keys.map(([label, action, accessible], index) => <button key={`${keypadMode}-${index}`} type="button" className="formula-key" tabIndex={-1}
          aria-label={accessible || label} title={accessible || label} onMouseDown={event => event.preventDefault()}
          onClick={() => controller.calculatorEdit(action)}>{label}</button>)}
      </div>
    </fieldset>}
    <div className="formula-calculator-actions">
      {current.unsupported && current.hasSaved && <button type="button" className="formula-remove-calculation" disabled={current.locked}
        onClick={() => controller.removeCalculatorDefinition()}>Remove calculation</button>}
      {current.unsupported && <button type="button" id="formula-calculator-save" className="formula-calculator-save" disabled={current.locked}
        onClick={() => controller.calculatorEdit("save")}>Save</button>}
    </div>
  </div>;
}
