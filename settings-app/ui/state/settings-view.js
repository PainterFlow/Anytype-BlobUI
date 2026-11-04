export const pageTitles = Object.freeze({
  preferences: "Preferences",
  accessibility: "Accessibility",
  advanced: "Advanced Settings",
  formulas: "Kanban View Formulas",
  "calendar-colors": "Color by Property",
  codex: "Anyblob",
});

export function createSettingsView() {
  const listeners = new Set();
  let snapshot = {
    page: "preferences", preferenceTab: 0, advancedTab: 0,
    documentationOpened: false, releaseNotesMedia: null,
    logRows: [], logSeverity: "info", shortcutRows: [],
    kanbanStyle: "default", kanbanStyleDisabled: true,
    saveVisible: false, saveDisabled: true, discardDisabled: true, saveBusy: false,
    settingsFields: {
      "font-family": "", "code-font-family": "", "accent-value": "",
      "include-selections": false, "native-accent": false, "reduced-motion": false,
      "tab-hover-effect": false, "link-hover-effect": false, "object-mention-hover-effect": false,
      "collapsed-categories": false, "calendar-colors-enabled": true,
    },
  };
  function update(patch) {
    if (Object.entries(patch).every(([key, value]) => snapshot[key] === value)) return;
    snapshot = { ...snapshot, ...patch };
    listeners.forEach(listener => listener());
  }
  return {
    getSnapshot: () => snapshot,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    update,
    updateSettingsFields(patch) {
      if (Object.entries(patch).every(([key, value]) => snapshot.settingsFields[key] === value)) return;
      update({ settingsFields: { ...snapshot.settingsFields, ...patch } });
    },
  };
}
