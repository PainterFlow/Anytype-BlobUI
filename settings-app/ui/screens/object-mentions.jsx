import React, { useSyncExternalStore } from "react";
import { SelectPicker } from "../shared/select-picker.jsx";
import { Switch } from "../shared/settings-controls.jsx";

const UI=()=>window.BlobUI;

function MentionPicker({kind,label,controller,state}) {
  const options=state[kind]||[];
  const selection=state.selections[kind];
  const value=selection.all?["*"]:selection.items.map(row=>kind==="spaces"?row.id:JSON.stringify([row.spaceId,row.id]));
  const caption=selection.all?"All":selection.items.length?`${selection.items.length} selected`:"None";
  return <SelectPicker label={label} options={options} value={value} caption={caption}
    rootClassName="mention-picker" menuClassName="mention-picker-menu" multiple fixed
    onChange={values => controller.change(kind, values)} />;
}

export function ObjectMentions() {
  const controller=UI().ObjectMentions;
  const state=useSyncExternalStore(controller.subscribe,controller.getSnapshot,controller.getSnapshot);
  return <section id="object-mentions-section" className="settings-group">
    <div className="section-heading"><h2>Object Mentions</h2></div>
    <div id="object-mentions-group" className="settings-list">
      {[ ["spaces","Filtered spaces"],["types","Filtered object types"] ].map(([kind,label])=><div key={kind} className="settings-row kanban-setting-row object-mention-row"><div className="kanban-setting-copy"><span className="kanban-setting-title">{label}</span></div><MentionPicker kind={kind} label={label} controller={controller} state={state} /></div>)}
      <Switch id="filter-incomplete-tasks" label="Filter Incomplete Tasks" className="native-accent-control settings-row kanban-setting-row" inputAttributes={{"aria-label":"Filter Incomplete Tasks"}} checked={state.selections.filterIncompleteTasks} onChange={checked=>controller.setIncomplete(checked)} />
    </div>
    <p className="input-hint">Filters control suggestions. Reading an object still requires permission, including Always Ask. Save to apply changes.</p>
    <p className="input-hint" role="status" id="object-mentions-status">{state.status}</p>
  </section>;
}
