import marked from "./vendor/marked.umd.js";
import "./visibility-field.js";
import "./modal.js";
import "./saved-state.js";
import "./edit-session.js";
import "./video-player.js";
import "./markdown.js";
import "./docs-data.js";
import "./docs.js";
import "./scrollbar.js";
import "./external-links.js";
import "./calculator-math.js";
import "./view-catalog.js";
import "./reload-controller.js";
import "./formula-page.js";
import "./project-permissions.js";
import "./object-mentions.js";
import "./codex-page.js";
import React, { useSyncExternalStore } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { Accessibility, Preferences } from "./screens/preferences-accessibility.jsx";
import { Advanced } from "./screens/advanced.jsx";
import { FormulaCalculatorView, FormulaPageView } from "./formula-page.jsx";
import { AppDialogs, Documentation, Onboarding, Sidebar, Titlebar } from "./screens/shell.jsx";
import { SettingsNavigation } from "./screens/settings-navigation.jsx";
import { CodexPage } from "./screens/codex-page.jsx";
import { createCalendarController } from "./calendar-controller.js";
import { CalendarColors } from "./screens/calendar-colors.jsx";
import { createSettingsView, pageTitles } from "./state/settings-view.js";
import { startSettingsApp } from "./app.js";
import { createNativeService } from "./native-service.js";
import { DocumentationReader, MarkdownImageViewer, MarkdownView } from "./screens/documentation.jsx";

window.marked = marked;
const calendarController = createCalendarController();
window.BlobUI.CalendarPage = calendarController;
const view = createSettingsView();
const updateView = patch => flushSync(() => view.update(patch));
let app;

function SettingsScreens() {
  const { page, preferenceTab, advancedTab, settingsFields, saveVisible, saveDisabled, discardDisabled, saveBusy,
    documentationOpened, releaseNotesMedia, logRows, logSeverity, shortcutRows, kanbanStyle, kanbanStyleDisabled } =
    useSyncExternalStore(view.subscribe, view.getSnapshot);
  const calendar = useSyncExternalStore(calendarController.subscribe, calendarController.getSnapshot);
  const settingsProps = { values: settingsFields, onChange: (id, value) => app?.changeSettingField(id, value) };
  return <>
    <Titlebar />
    <Documentation><DocumentationReader visible={documentationOpened} /></Documentation>
    <Onboarding />
    <div className="app-shell" id="settings-shell">
      <Sidebar navigation={<SettingsNavigation page={page} onNavigate={name => app?.showSettingsPage(name)}
        onSection={(name, index) => app?.openSection(name, index)} />}
        saveVisible={saveVisible} saveDisabled={saveDisabled} discardDisabled={discardDisabled} saveBusy={saveBusy}
        logRows={logRows} logSeverity={logSeverity}
        onSave={() => app?.save()} onDiscard={() => app?.openDiscardConfirmation()} />
      <main className="main-content">
        <section className="page-content" aria-labelledby="page-title">
          <div className="settings-content">
            <header className="page-heading"><h1 id="page-title" tabIndex="-1">{pageTitles[page]}</h1></header>
            <div id="preferences-page" className="settings-page" hidden={page !== "preferences"}>
              <Preferences {...settingsProps} selectedTab={preferenceTab} onSelectTab={index => view.update({ preferenceTab: index })}
                kanbanStyle={kanbanStyle} kanbanStyleDisabled={kanbanStyleDisabled || page !== "preferences" || preferenceTab !== 1}
                onKanbanStyleChange={value => app?.changeKanbanStyle(value)} formulaController={window.BlobUI.FormulaPage}
                calendarLocked={calendar.locked || calendar.loading || calendar.loadFailed}
                onOpenCalendar={() => app?.openFeature("calendar-colors")} />
            </div>
            <div id="calendar-colors-page" className="settings-page" hidden={page !== "calendar-colors"}><CalendarColors controller={calendarController} /></div>
            <div id="formulas-page" className="settings-page" hidden={page !== "formulas"}><FormulaPageView controller={window.BlobUI.FormulaPage} /></div>
            <div id="accessibility-page" className="settings-page" hidden={page !== "accessibility"}><Accessibility {...settingsProps} /></div>
            <div id="advanced-page" className="settings-page" hidden={page !== "advanced"}><Advanced selectedTab={advancedTab} onSelectTab={index => app?.selectAdvancedTab(index)} /></div>
            <div id="codex-page" className="settings-page" hidden={page !== "codex"}><CodexPage /></div>
          </div>
        </section>
        <div className="sr-only" id="save-status" role="status" aria-live="polite">
          <span id="status-message">Loading your current settings…</span>
        </div>
      </main>
    </div>
    <AppDialogs formulaContent={<FormulaCalculatorView controller={window.BlobUI.FormulaPage} />}
      shortcutRows={shortcutRows} onShortcutCapture={action => app?.beginShortcutCapture(action)}
      releaseContent={<MarkdownView id="release-notes-content" className="release-notes-content markdown-content"
        role="region" aria-label="Release notes" tabIndex={0} {...releaseNotesMedia} />} />
    <MarkdownImageViewer />
  </>;
}

const root = createRoot(document.getElementById("settings-react-root"));
flushSync(() => root.render(<SettingsScreens />));
await import("./sidebar-resize.js");
app = startSettingsApp({ createNativeService, view: {
  update: updateView,
  updateSettingsFields: patch => flushSync(() => view.updateSettingsFields(patch)),
} });
window.addEventListener("unload", () => {
  app.dispose();
  root.unmount();
}, { once: true });
