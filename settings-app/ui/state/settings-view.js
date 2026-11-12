export const pageTitles = Object.freeze({
  preferences: "Preferences",
  accessibility: "Accessibility",
  "calendar-events": "Calendar Events",
  "kanban-views": "Kanban Views",
  advanced: "Advanced Settings",
  codex: "Anyblob",
});

export function createSettingsView() {
  const listeners = new Set();
  let snapshot = {
    page: "preferences", preferenceTab: 0, calendarTab: 0, kanbanTab: 0, advancedTab: 0,
    customThemes: [], themeDirectory: "", customThemeDisabled: true,
    documentationOpened: false, releaseNotesMedia: null,
    logRows: [], logSeverity: "info", copyLogsCopied: false, shortcutRows: [],
    startupMode: "off", startupModeDisabled: true,
    backgroundExperiment: "disabled", backgroundExperimentDisabled: true,
    kanbanStyle: "default", kanbanStyleDisabled: true,
    saveVisible: false, saveDisabled: true, discardDisabled: true, saveBusy: false,
    settingsFields: {
      "font-family": "", "code-font-family": "", "accent-value": "",
      "enable-custom-themes": false, "based-theme": "auto", "custom-theme": "", "theme-color-value": "#191918",
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
