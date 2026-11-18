import React, { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { clearStaleSaveStatus } from "../anyblob-status.js";
import { FolderPathInput, SettingsGroup, SettingsHeading, SettingsRow, Switch } from "../shared/settings-controls.jsx";
import { SettingsDialog } from "../shared/settings-dialog.jsx";
import { loadMarkdownFile, MarkdownView } from "./documentation.jsx";
import "./codex-integration.css";

const UI = () => window.BlobUI;

export function CodexIntegration() {
  const page = UI().CodexPage;
  const api = useSyncExternalStore(page.subscribe, page.getSnapshot, page.getSnapshot);
  const draftController = UI().AnyblobDraft;
  const draft = useSyncExternalStore(draftController.subscribe, draftController.getSnapshot, draftController.getSnapshot);
  const { enabled, folder } = draft;
  const [status, setStatus] = useState("");
  const [guideOpen, setGuideOpen] = useState(false);
  const [guide, setGuide] = useState(null);
  const [guideError, setGuideError] = useState(false);
  const [folderChanging, setFolderChanging] = useState(false);
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

  async function browseFolder() {
    if (!enabled || folderChanging) return;
    setFolderChanging(true);
    setStatus("");
    try {
      const selected = await api.invoke("select_anyblob_projects_folder", { currentDirectory: folder || null });
      if (selected == null) return;
      refreshEpoch.current++;
      draftController.change({ folder: selected });
    } catch (error) {
      setStatus(String(error.message || error));
    } finally {
      setFolderChanging(false);
    }
  }

  if (!api) return null;
  return <>
    <SettingsHeading title="Anyblob" helpDialogId="anyblob-guide-dialog"
      onHelpClick={() => { setGuideError(false); setGuideOpen(true); }} />
    <div className="settings-list anyblob-integration-settings">
      <Switch id="enable-anyblob" label="Enable Anyblob" className="native-accent-control settings-row"
        checked={enabled} disabled={draft.locked || folderChanging} onChange={configure} />
    </div>
    <SettingsGroup className="settings-list anyblob-integration-settings">
      <SettingsRow className="settings-row anyblob-folder-row" label="Local Projects Folder">
        <FolderPathInput id="anyblob-local-projects-folder" aria-label="Local Projects Folder" value={folder}
          placeholder="No folder selected" disabled={!enabled || draft.locked || folderChanging} onClick={browseFolder}
          onKeyDown={event => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              void browseFolder();
            }
          }} />
      </SettingsRow>
    </SettingsGroup>
    <p className="input-hint" role="status" id="anyblob-integration-status">{status}</p>
    <SettingsDialog id="anyblob-guide-dialog" title="Anyblob setup guide" open={guideOpen}
      onClose={() => setGuideOpen(false)} className="release-notes-dialog">
      {guide ? <MarkdownView {...guide} className="release-notes-content markdown-content" />
        : <p role="status">{guideError ? "Could not load the Anyblob setup guide." : "Loading the setup guide…"}</p>}
    </SettingsDialog>
  </>;
}
