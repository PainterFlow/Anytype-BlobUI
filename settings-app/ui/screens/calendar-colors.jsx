import React, { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { SelectPicker } from "../shared/select-picker.jsx";
import { ExpandableSearch } from "../shared/search.jsx";
import { FilterIcon, ReloadIcon } from "../shared/catalog-icons.jsx";
import { DeleteIcon } from "../shared/settings-controls.jsx";

const UI = () => window.BlobUI;


function Toolbar({ prefix, label, records, onReload, locked, busy, selectedSpace, setSelectedSpace, query, setQuery }) {
  const spaces = useMemo(() => [...new Map(records.map(item => [item.spaceId, item.space || "Unnamed space"])).entries()]
    .sort((a,b) => a[1].localeCompare(b[1]) || a[0].localeCompare(b[0])), [records]);
  const filterOptions = [{ value: "", label: "All spaces" }, ...spaces.map(([value, label]) => ({ value, label }))];
  return <div className={"formula-toolbar" + (prefix === "calendar-override" ? " calendar-sticky-toolbar" : "")}>
    <ExpandableSearch id={`${prefix}-search`} label={label} placeholder={label} disabled={locked} value={query} onChange={setQuery} />
    <button id={`${prefix}-reload`} type="button" className="neutral-icon-button" aria-label="Reload list" title="Reload list" disabled={locked || busy} aria-busy={String(busy)} onClick={onReload}><ReloadIcon /></button>
    <SelectPicker id={`${prefix}-filter`} rootId={`${prefix}-filter-control`} menuId={`${prefix}-filter-options`}
      rootClassName="formula-filter-control" icon={<FilterIcon />} label="Filter by space"
      title={selectedSpace ? `Filtered to ${selectedSpace}` : "Filter by space"} pressed={Boolean(selectedSpace)}
      disabled={locked || busy} options={filterOptions} value={selectedSpace} onChange={setSelectedSpace} />
  </div>;
}

function RuleRow({ controller, label, override, onRemove, disabled, dialogRef }) {
  const selected = controller.options(override);
  return <div className="calendar-rule-row">
    <div className="calendar-rule-name-group"><span className="calendar-rule-name">{label}</span>{onRemove && <button type="button" className="neutral-icon-button calendar-remove" aria-label="Remove override" title="Remove override" disabled={disabled} onClick={onRemove}><DeleteIcon /></button>}</div>
    <SelectPicker label={`${label} color property`} rootClassName="calendar-property-picker"
      triggerClassName="formula-insert-property" menuClassName="calendar-property-menu" portalRef={dialogRef} options={selected.options} value={selected.value}
      disabled={disabled} onChange={value => controller.setRule(override, value)} />
  </div>;
}

export function CalendarColors({ controller }) {
  const snapshot = useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot);
  const [query, setQuery] = useState("");
  const [space, setSpace] = useState("");
  const [overrideQuery, setOverrideQuery] = useState("");
  const dialogRef = useRef(null), backdropRef = useRef(null), closeRef = useRef(null), activeRow = useRef(null), modalRef = useRef(null);
  const types = controller.getTypes();
  const visibleTypes = types.filter(type => (!space || type.spaceId === space) && `${type.name} ${type.space}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));

  useEffect(() => {
    if (!backdropRef.current || !dialogRef.current) return undefined;
    const modal = UI().createModalController({ backdrop: backdropRef.current, dialog: dialogRef.current,
      initialFocus: () => closeRef.current,
      fallbackFocus: () => activeRow.current?.isConnected ? activeRow.current : document.getElementById("open-calendar-colors"),
      inertTargets: [document.getElementById("settings-shell"), document.getElementById("title-drag-region")].filter(Boolean),
      onClose() {
        if (controller.getSnapshot().screen === "calendars") { controller.setScreen("type"); modal.open(); }
        else controller.close();
      }
    });
    modalRef.current = modal;
    return () => { modal.destroy(); modalRef.current = null; };
  }, [controller]);
  useEffect(() => {
    if (snapshot.modal) modalRef.current?.open();
    else if (modalRef.current?.isOpen()) modalRef.current.close();
  }, [snapshot.modal]);
  useEffect(() => {
    if (!snapshot.modal || !dialogRef.current) return;
    const content = dialogRef.current.querySelector(".calendar-modal-content");
    const scrollbar = UI().Scrollbar.attach(content);
    return () => scrollbar.destroy();
  }, [snapshot.modal, snapshot.screen]);

  const portal = createPortal(<div id="calendar-rule-backdrop" className="confirm-backdrop" hidden ref={backdropRef}>
    <section id="calendar-rule-dialog" className="confirm-dialog calendar-rule-dialog" role="dialog" aria-modal="true" aria-labelledby="calendar-rule-title" tabIndex={-1} ref={dialogRef} data-screen={snapshot.screen}>
      <header className="calendar-modal-header"><h2 id="calendar-rule-title">{snapshot.screen === "calendars" ? "Choose calendar view" : snapshot.active?.name || "Color rules"}</h2><button ref={closeRef} type="button" className="neutral-icon-button calendar-modal-close" aria-label="Close color rules" onClick={() => modalRef.current?.close()}>×</button></header>
      <div className="calendar-modal-controls">{snapshot.screen === "calendars" && <Toolbar prefix="calendar-override" label="Search calendar views" records={controller.views().map(view => ({ spaceId: view.target.spaceId, space: view.space }))} onReload={() => controller.discover(true)} locked={snapshot.locked || snapshot.loading} busy={snapshot.discovering} selectedSpace="" setSelectedSpace={() => {}} query={overrideQuery} setQuery={setOverrideQuery} />}</div>
      <div className="calendar-modal-content">
        {snapshot.screen === "type" && snapshot.active && <>
          <RuleRow dialogRef={dialogRef} controller={controller} label="Default" disabled={snapshot.locked || snapshot.loading || snapshot.loadFailed} />
          {(controller.getSnapshot().state.types.find(item => item.spaceId === snapshot.active.spaceId && item.typeKey === snapshot.active.key)?.overrides || []).map((override, index) => {
            const view = controller.viewFor(override.target);
            return <RuleRow dialogRef={dialogRef} key={JSON.stringify(override.target) + index} controller={controller} label={view?.name || "Unavailable calendar"} override={override} disabled={snapshot.locked || snapshot.loading || snapshot.loadFailed} onRemove={() => controller.removeOverride(override)} />;
          })}
          <button type="button" className="formula-button" disabled={snapshot.locked || snapshot.loading || !snapshot.discovered} onClick={() => controller.setScreen("calendars")}>Add override</button>
          {!(snapshot.active.properties || []).length && <p className="input-hint">This type has no available select or checkbox properties.</p>}
        </>}
        {snapshot.screen === "calendars" && <div className="calendar-view-results">{controller.views().filter(view => `${view.name} ${view.space} ${view.source}`.toLocaleLowerCase().includes(overrideQuery.trim().toLocaleLowerCase())).map(view => <section className="formula-view" key={JSON.stringify(view.target)}><button type="button" className="formula-view-row" onClick={() => controller.addOverride(view)}><span className="formula-view-name">{view.name}</span><span className="formula-source">{view.source} · {view.space}</span></button></section>)}{!controller.views().length && <p className="input-hint">No more accessible calendar views in this space.</p>}</div>}
      </div>
    </section>
  </div>, document.body);

  return <>
    <Toolbar prefix="calendar" label="Search object types" records={types.map(type => ({ spaceId: type.spaceId, space: type.space }))} onReload={() => controller.discover(true)} locked={snapshot.locked || snapshot.loading || snapshot.loadFailed} busy={snapshot.discovering} selectedSpace={space} setSelectedSpace={setSpace} query={query} setQuery={setQuery} />
    <p className="field-error" id="calendar-rules-message" role="status" aria-live="polite" data-status={snapshot.status.includes("saved") ? "success" : "error"}>{snapshot.status}</p>
    <div id="calendar-type-list">{visibleTypes.map(type => <section className="formula-view" key={`${type.spaceId}/${type.key}`}><button type="button" className="formula-view-row" disabled={snapshot.locked || snapshot.loading} onClick={event => { activeRow.current = event.currentTarget; controller.open(type); }}><span className="formula-view-name">{type.name}</span><span className="formula-source">{type.space}</span>{type.unavailable && <span className="formula-view-warning">Saved rules are retained until you remove them.</span>}</button>{type.unavailable && <button type="button" className="formula-button" onClick={() => controller.removeType(type)}>Remove saved rules</button>}</section>)}
      {!types.length && <p className="input-hint">{snapshot.loading || snapshot.discovering ? "Loading object types…" : snapshot.loadFailed ? "Calendar rules could not be loaded." : snapshot.discoveryError ? "Could not load object types. Reload to retry." : "No accessible object types found."}</p>}
      {types.length > 0 && !visibleTypes.length && <p className="input-hint">No matching results.</p>}
    </div>
    {portal}
  </>;
}
