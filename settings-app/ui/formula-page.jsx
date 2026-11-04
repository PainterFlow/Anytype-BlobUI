import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import { SelectPicker } from "./shared/select-picker.jsx";
import { FilterIcon } from "./shared/catalog-icons.jsx";

const searchPath = "M8.5 2.5C11.8137 2.5 14.5 5.18629 14.5 8.5C14.5 9.79977 14.0852 11.0019 13.3828 11.9844L17.2227 15.8242C17.6132 16.2147 17.6132 16.8478 17.2227 17.2383C16.8321 17.6288 16.1991 17.6288 15.8086 17.2383L11.9658 13.3955C10.9869 14.0899 9.79171 14.5 8.5 14.5C5.18629 14.5 2.5 11.8137 2.5 8.5C2.5 5.18629 5.18629 2.5 8.5 2.5ZM8.5 4C6.01472 4 4 6.01472 4 8.5C4 10.9853 6.01472 13 8.5 13C10.9853 13 13 10.9853 13 8.5C13 6.01472 10.9853 4 8.5 4Z";

function IconButton({ id, label, children, ...props }) {
  return <button type="button" id={id} className="neutral-icon-button" aria-label={label} title={label} {...props}>{children}</button>;
}

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
  const [expanded, setExpanded] = useState(false);
  const [space, setSpace] = useState("");
  const searchInput = useRef(null);
  const searchToggle = useRef(null);
  const spaces = useMemo(() => [...new Map(data.views.map(view => [view.target?.spaceId, view.space || "Unnamed space"]))]
    .filter(([id]) => id).sort((a, b) => a[1].localeCompare(b[1]) || a[0].localeCompare(b[0])), [data.views]);
  const matching = data.views.filter(view => (!space || view.target?.spaceId === space)
    && [view.name, view.space, view.source, view.kind].filter(Boolean).join(" ").toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));

  useEffect(() => { if (!expanded) setQuery(""); }, [expanded]);
  return <>
    <div className="formula-toolbar">
      <div id="formula-search-control" className="expandable-search" data-expanded={String(expanded)}>
        <IconButton id="formula-search-toggle" ref={searchToggle} label="Search Kanban views" tabIndex={data.locked ? -1 : 0}
          aria-controls="formula-search" aria-expanded={expanded} onClick={() => {
            const next = !expanded; setExpanded(next);
            if (next) requestAnimationFrame(() => searchInput.current?.focus({ preventScroll: true }));
          }}>
          <svg aria-hidden="true" width="20" height="20" viewBox="0 0 20 20" fill="none"><path d={searchPath} fill="currentColor" /></svg>
        </IconButton>
        <label htmlFor="formula-search" className="sr-only">Search Kanban views</label>
        <input id="formula-search" ref={searchInput} type="search" placeholder="Search Kanban views" tabIndex={expanded ? 0 : -1}
          value={query} disabled={data.locked} onChange={event => setQuery(event.target.value)} onBlur={event => {
            if (!event.currentTarget.value) setExpanded(false);
          }} onKeyDown={event => {
            if (event.key === "Escape") { event.preventDefault(); setQuery(""); setExpanded(false); searchToggle.current?.focus({ preventScroll: true }); }
          }} />
      </div>
      <IconButton id="formula-reload" label="Reload available views" disabled={data.locked || data.discovering}
        aria-busy={data.discovering} onClick={reloadViews}>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M16 5.5v4h-4M15.8 9.2a6 6 0 1 0-.7 4" /></svg>
      </IconButton>
      <SelectPicker id="formula-filter" rootId="formula-filter-control" menuId="formula-filter-options"
        label="Filter by space" title="Filter by space" icon={<FilterIcon />} pressed={Boolean(space)}
        triggerClassName={space ? "is-filtered" : ""} disabled={data.locked || data.discovering}
        options={[{ value: "", label: "All spaces" }, ...spaces.map(([value, label]) => ({ value, label }))]}
        value={space} onChange={setSpace} closeOnSelect={false} />
    </div>
    <p id="formula-storage-message" className="field-error" role="status" aria-live="polite" data-status={data.success ? "success" : "error"}>{data.message}</p>
    <fieldset id="formula-fields" aria-label="Kanban formulas" disabled={data.locked}>
      <div id="formula-contents">
        {matching.map((view, index) => <section className="formula-view" key={JSON.stringify(view.target || index)}>
          <button type="button" className="formula-view-row" data-active={String(!!view.active)} aria-pressed={!!view.active} onClick={event => selectView(view, event.currentTarget)}>
            <span className="formula-view-name">{view.name}</span>
            <span className="formula-source">{[view.space, view.source, view.kind].filter(Boolean).join(" · ")}</span>
            {view.unsupported && <span className="formula-view-warning">{view.unsupported}</span>}
          </button>
        </section>)}
        {!matching.length && <p className="input-hint formula-empty-message">{query || space ? "No matching Kanban views." : data.loading ? "Loading Kanban views…" : "No accessible Kanban views found."}</p>}
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
