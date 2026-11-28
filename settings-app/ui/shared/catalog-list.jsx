import React, { useState } from "react";
import { SelectPicker, SelectorOptionIcon } from "./select-picker.jsx";
import { ExpandableSearch } from "./search.jsx";
import { FilterIcon, ReloadIcon } from "./catalog-icons.jsx";
import { IconButton } from "./settings-controls.jsx";

function flatFilterOptions(options) {
  return (Array.isArray(options) ? options : []).flatMap(option => Array.isArray(option?.options) ? option.options : [option]);
}

export function CatalogListToolbar({
  prefix, searchLabel, query, onQueryChange, reloadLabel = "Reload list", onReload, locked = false, busy = false,
  filterOptions = [], filterValue, onFilterChange = () => {}, filterMultiple = false, filterLabel = "Filter",
  filterCloseOnSelect, showFilter = true, className = "", portalRef,
}) {
  const flatOptions = flatFilterOptions(filterOptions);
  const selectedValues = filterMultiple ? (Array.isArray(filterValue) ? filterValue : []) : [filterValue];
  const selectedOption = !filterMultiple && selectedValues.length === 1 ? flatOptions.find(option => option.value === filterValue) : null;
  const active = filterMultiple ? selectedValues.length > 0 : Boolean(filterValue);
  return <div className={["formula-toolbar", className].filter(Boolean).join(" ")}>
    <ExpandableSearch id={prefix + "-search"} label={searchLabel} placeholder={searchLabel} value={query} onChange={onQueryChange} disabled={locked} />
    <IconButton id={prefix + "-reload"} aria-label={reloadLabel} title={reloadLabel} disabled={locked || busy} aria-busy={String(busy)} onClick={onReload}><ReloadIcon /></IconButton>
    {showFilter && <SelectPicker id={prefix + "-filter"} rootId={prefix + "-filter-control"} menuId={prefix + "-filter-options"}
      rootClassName="formula-filter-control" label={filterLabel} title={active && selectedOption ? filterLabel + ": " + selectedOption.label : filterLabel}
      icon={selectedOption?.icon ? <SelectorOptionIcon icon={selectedOption.icon} /> : <FilterIcon />} pressed={active}
      triggerClassName={active ? "is-filtered" : ""} disabled={locked || busy} options={filterOptions} value={filterValue}
      multiple={filterMultiple} closeOnSelect={filterCloseOnSelect} onChange={onFilterChange} portalRef={portalRef} />}
  </div>;
}

export function CatalogList({ sections = [], renderItem, emptyMessage = "", emptyClassName = "", className = "", collapsible = true }) {
  const [collapsedSections, setCollapsedSections] = useState(() => new Set());
  const visible = sections.filter(section => section.items?.length);
  function toggle(id) {
    setCollapsedSections(current => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  }
  return <div className={`catalog-list ${className}`.trim()}>
    {visible.map((section, index) => {
      const id = section.id || section.label || index;
      const collapsed = collapsible && collapsedSections.has(id);
      return <section className="catalog-list-section" key={id} data-collapsed={String(collapsed)}>
        {section.label && <button type="button" className="catalog-list-heading" aria-expanded={!collapsed}
          disabled={!collapsible} onClick={() => collapsible && toggle(id)}>
          {section.icon && <span className="catalog-list-heading-icon" aria-hidden="true"><SelectorOptionIcon icon={section.icon} /></span>}
          <span className="catalog-list-heading-label">{section.label}</span>
          {collapsible && <svg className="catalog-list-heading-arrow" viewBox="0 0 16 16" aria-hidden="true"><path d="m6 4 4 4-4 4" /></svg>}
        </button>}
        <div className="catalog-list-items-wrap"><div className="catalog-list-items">{(section.items || []).map((item, itemIndex) => renderItem(item, itemIndex, section))}</div></div>
      </section>;
    })}
    {!visible.some(section => section.items?.length) && emptyMessage && <p className={`input-hint catalog-list-empty ${emptyClassName}`.trim()}>{emptyMessage}</p>}
  </div>;
}

export function CatalogListCustomRow({ children, className = "" }) {
  return <section className={`catalog-list-row catalog-list-custom-row ${className}`.trim()}>{children}</section>;
}

export function CatalogListRow({
  icon, title, description, meta, warning, accessory, children, interactive = true, disabled = false,
  active = false, onClick, className = "", titleAttributes = {}, rowRef = null,
}) {
  const content = <>
    {icon && <span className="catalog-list-icon" aria-hidden="true"><SelectorOptionIcon icon={icon} /></span>}
    <span className="catalog-list-copy">
      <span className="catalog-list-title formula-view-name" {...titleAttributes}>{title}</span>
      {description && <span className="catalog-list-description">{description}</span>}
    </span>
    {meta && <span className="catalog-list-meta formula-source">{meta}</span>}
    {accessory}
    {children}
    {warning && <span className="catalog-list-warning formula-view-warning">{warning}</span>}
  </>;
  const classes = `catalog-list-row formula-view formula-view-row ${className}`.trim();
  if (!interactive) {
    return <div ref={rowRef} className={classes} data-active={String(active)}>{content}</div>;
  }
  return <button ref={rowRef} type="button" className={classes} data-active={String(active)} aria-pressed={active || undefined}
    disabled={disabled} onClick={onClick}>{content}</button>;
}

export function groupIntoSections(items, spaces, getSpaceId) {
  const icons = new Map((spaces || []).map(space => [space.id, space]));
  const sections = new Map();
  for (const item of items) {
    const id = getSpaceId(item) || "";
    const known = icons.get(id);
    const label = item.space || known?.name || "Unnamed space";
    if (!sections.has(id)) sections.set(id, { id, label, icon: known?.icon || item.spaceIcon || null, items: [] });
    sections.get(id).items.push(item);
  }
  return [...sections.values()].sort((left, right) => left.label.localeCompare(right.label) || left.id.localeCompare(right.id));
}
