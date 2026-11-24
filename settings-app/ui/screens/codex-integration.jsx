import React, { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { clearStaleSaveStatus } from "../anyblob-status.js";
import { HelpTooltip } from "../shared/help-tooltip.jsx";
import { SettingsHeading, Switch } from "../shared/settings-controls.jsx";
import { SettingsDialog } from "../shared/settings-dialog.jsx";
import { loadMarkdownFile, MarkdownView } from "./documentation.jsx";
import "./codex-integration.css";

const UI = () => window.BlobUI;
const AGENTS_GUIDE_TOOLTIP = 'For each space, you can create an object of any type(unless it is a collection/query) called "agents guide" the agent will always read all objects with that name before using Anyblob and will follow that objects guidelines like AGENTS.md.';

export function CodexIntegration() {
  const page = UI().CodexPage;
  const api = useSyncExternalStore(page.subscribe, page.getSnapshot, page.getSnapshot);
  const draftController = UI().AnyblobDraft;
  const draft = useSyncExternalStore(draftController.subscribe, draftController.getSnapshot, draftController.getSnapshot);
  const { enabled } = draft;
  const [status, setStatus] = useState("");
  const [guideOpen, setGuideOpen] = useState(false);
  const [guide, setGuide] = useState(null);
  const [guideError, setGuideError] = useState(false);
  const refreshEpoch = useRef(0);

  useEffect(() => {
    if (!guideOpen || guide) return undefined;
    let active = true;
    loadMarkdownFile("./anyblob-guide.md")
      .then(result => { if (active) setGuide(result); })
      .catch(() => { if (active) setGuideError(true); });
    return () => { active = false; };
  }, [guideOpen, guide]);

  const refresh = async () => {
    if (!api) return;
    const epoch = ++refreshEpoch.current;
    try {
      await draftController.load();
      if (epoch === refreshEpoch.current) setStatus(clearStaleSaveStatus);
    } catch (error) {
      if (epoch === refreshEpoch.current) setStatus(String(error.message || error));
    }
  };

  useEffect(() => {
    if (!api) return undefined;
    void refresh();
    return () => { refreshEpoch.current++; };
  }, [api]);

  useEffect(() => UI().AppReload?.register(refresh), [api]);

  useEffect(() => {
    const listener = () => { void refresh(); };
    window.addEventListener("blobui-codex-integration-refresh", listener);
    return () => window.removeEventListener("blobui-codex-integration-refresh", listener);
  }, [api]);

  function configure(value) {
    draftController.change({ enabled: value });
  }

  if (!api) return null;
  return <>
    <SettingsHeading title="Anyblob" helpDialogId="anyblob-guide-dialog"
      onHelpClick={() => { setGuideError(false); setGuideOpen(true); }} />
    <div className="settings-list anyblob-integration-settings">
      <Switch id="enable-anyblob" label="Enable Anyblob" className="native-accent-control settings-row"
        checked={enabled} disabled={draft.locked} onChange={configure} />
    </div>
    <p className="input-hint" role="status" id="anyblob-integration-status">{status}</p>
    <SettingsDialog id="anyblob-guide-dialog" title="Anyblob setup guide" open={guideOpen}
      onClose={() => setGuideOpen(false)} className="release-notes-dialog">
      {guide ? <MarkdownView {...guide} className="release-notes-content markdown-content" />
        : <p role="status">{guideError ? "Could not load the Anyblob setup guide." : "Loading the setup guide…"}</p>}
    </SettingsDialog>
  </>;
}

export function AnyblobAdvancedSettings() {
  const draftController = UI().AnyblobDraft;
  const draft = useSyncExternalStore(draftController.subscribe, draftController.getSnapshot, draftController.getSnapshot);
  const { enabled, agentsGuide } = draft;
  return <>
    <SettingsHeading title="Advanced Settings" />
    <div className="settings-list anyblob-integration-settings">
      <Switch id="enable-agents-guide" className="native-accent-control settings-row"
        label={<>Enable AGENTS.md page <HelpTooltip label="Enable AGENTS.md page help" text={AGENTS_GUIDE_TOOLTIP} /></>}
        checked={agentsGuide} disabled={draft.locked || !enabled}
        onChange={value => draftController.change({ agentsGuide: value })} />
    </div>
  </>;
}
