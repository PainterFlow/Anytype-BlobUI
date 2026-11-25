import React, { useSyncExternalStore } from "react";
import { AdvancedSelector } from "../shared/select-picker.jsx";
import { Switch } from "../shared/settings-controls.jsx";

const UI=()=>window.BlobUI;

function FilteredTypesPicker({controller,state}) {
  const selection=state.selections;
  const value=controller.selectedValues(selection);
  const selectedLabels=state.sections.flatMap(section=>section.options).filter(option=>value.includes(option.value)).map(option=>option.label);
  const joined=selectedLabels.join(", ");
  const caption=selection.types.all&&selection.spaces.all?"All":joined.length<=44?joined||"None":`${selectedLabels.length} selected`;
  return <AdvancedSelector label="Filtered Types" sections={state.sections} value={value} caption={caption}
    rootClassName="mention-picker" menuClassName="mention-picker-menu" multiple closeOnSelect={false}
    placeholder="Search object types" emptyMessage="No object types match the search."
    onChange={values => controller.change(values)} />;
}

export function ObjectMentions() {
  const controller=UI().ObjectMentions;
  const state=useSyncExternalStore(controller.subscribe,controller.getSnapshot,controller.getSnapshot);
  return <section id="object-mentions-section" className="settings-group">
    <div className="section-heading"><h2>Object Mentions</h2></div>
    <div id="object-mentions-group" className="settings-list">
      <div className="settings-row kanban-setting-row object-mention-row advanced-row">
        <div className="kanban-setting-copy"><span className="kanban-setting-title">Filtered Types</span></div>
        <FilteredTypesPicker controller={controller} state={state} />
      </div>
      <Switch id="filter-incomplete-tasks" label="Filter Incomplete Tasks" className="native-accent-control settings-row kanban-setting-row" inputAttributes={{"aria-label":"Filter Incomplete Tasks"}} checked={state.selections.filterIncompleteTasks} onChange={checked=>controller.setIncomplete(checked)} />
    </div>
    <p className="input-hint">Filters control suggestions. Only spaces with saved read permission appear. Reading an object still requires permission, including Always Ask. Save to apply changes.</p>
    <p className="input-hint" role="status" id="object-mentions-status">{state.status}</p>
  </section>;
}
