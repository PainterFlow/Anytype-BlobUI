import React, { useEffect, useState } from "react";
import { Button, Checkbox, CopyIcon, CopySuccessIcon } from "../shared/settings-controls.jsx";
import { HelpTooltip } from "../shared/help-tooltip.jsx";
import { loadMarkdownFile, MarkdownView } from "./documentation.jsx";

function CloseIcon() {
  return <svg viewBox="0 0 12 12" aria-hidden="true"><path d="m2 2 8 8m0-8-8 8" /></svg>;
}

export function Titlebar({ backgroundExperiment = "disabled", fullscreenExperiment = "disabled" }) {
  return <header className="window-titlebar" id="title-drag-region" aria-label="Window title bar">
    <span className="window-title">BlobUI Settings</span>
    <div className="window-controls">
      <Button hidden={backgroundExperiment === "disabled"} id="window-background" className="window-control" aria-label="Run in background" title="Run in background"><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 1v7m-3-3 3 3 3-3M1 9v2h10V9" /></svg></Button>
      <Button id="window-minimize" className="window-control" aria-label="Minimize" title="Minimize"><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M1 6.5h10" /></svg></Button>
      <Button hidden={fullscreenExperiment !== "enable-full-screen"} id="window-fullscreen" className="window-control" aria-label="Full screen" title="Full screen"><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2 5V2h3M7 2h3v3M10 7v3H7M5 10H2V7" /></svg></Button>
      <Button id="window-close" className="window-control window-close" aria-label={backgroundExperiment === "disabled" ? "Close to background" : "Quit BlobUI Settings"} title={backgroundExperiment === "disabled" ? "Close to background. Numpad 1 reopens Settings in Anytype. Ctrl+Q quits." : "Quit BlobUI Settings. Ctrl+Q quits."}><CloseIcon /></Button>
    </div>
  </header>;
}

function OnboardingChoice({ id, inputId, title, description }) {
  return <label id={id} className="onboarding-choice" htmlFor={inputId}>
    <Checkbox id={inputId} shape="squircle" />
    <span className="onboarding-choice-copy">
      <span className="onboarding-choice-title">{title}</span>
      <span className="onboarding-choice-description">{description}</span>
    </span>
  </label>;
}

function PreviewArtwork({ kind }) {
  const accent = kind === "interface" ? "#de50de" : kind === "codex" ? "#40b8f5" : kind === "notion" ? "#4ecc4e" : "#a3a3a3";
  return <svg className="onboarding-preview-art" viewBox="0 0 320 150" role="img" aria-label="Feature preview">
    <rect width="320" height="150" rx="14" fill="#171717" />
    <rect x="18" y="18" width="284" height="114" rx="10" fill="#202020" />
    <rect x="30" y="30" width="72" height="90" rx="8" fill="#252525" />
    <rect x="114" y="30" width="176" height="14" rx="7" fill="#303030" />
    <rect x="114" y="55" width="128" height="10" rx="5" fill="#2a2a2a" />
    <rect x="114" y="75" width="150" height="10" rx="5" fill="#2a2a2a" />
    {kind === "interface" && <>
      <rect x="114" y="99" width="74" height="18" rx="9" fill={accent} />
      <circle cx="276" cy="108" r="10" fill={accent} />
    </>}
    {kind === "annoying" && <>
      <rect x="202" y="88" width="86" height="28" rx="8" fill="#2c2c2c" opacity=".45" />
      <path d="m208 94 74 16m0-16-74 16" stroke="#737373" strokeWidth="4" strokeLinecap="round" />
      <rect x="114" y="99" width="70" height="12" rx="6" fill="#343434" />
    </>}
    {kind === "codex" && <>
      <path d="M125 103h36l18-18m0 36-18-18" fill="none" stroke={accent} strokeWidth="5" strokeLinecap="round" strokeLinejoin="round" />
      <rect x="204" y="95" width="82" height="20" rx="10" fill="#263949" />
      <path d="m216 105 10 5 18-12" fill="none" stroke={accent} strokeWidth="4" strokeLinecap="round" strokeLinejoin="round" />
    </>}
    {kind === "notion" && <>
      {[0, 1, 2].map(row => <g key={row}>
        <rect x={114 + row * 58} y="94" width="48" height="28" rx="6" fill="#292929" />
        <rect x={120 + row * 58} y={101 + row * 3} width="26" height="6" rx="3" fill={row === 1 ? accent : "#686868"} />
      </g>)}
    </>}
  </svg>;
}

const onboardingPreviews = [
  {
    key: "interface",
    media: [{ type: "artwork" }],
    title: "Customize the interface",
    body: "BlobUI enables Dark Mode+, applies the Blob Pink accent, and leaves selections on the system color. Your fonts, banner, and other appearance choices stay unchanged.",
  },
  {
    key: "annoying",
    media: [{ type: "artwork" }],
    title: "A quieter Anytype",
    body: "Hover previews for tabs, links, and object mentions stay hidden. Kanban boards use balanced spacing, while your category collapse choice stays unchanged.",
  },
  {
    key: "codex",
    media: [{ type: "artwork" }],
    title: "Connect Codex through Anyblob",
    body: "BlobUI enables Anyblob, object mentions, and the AGENTS.md guide page. Codex still follows the permissions you save for each space.",
  },
  {
    key: "notion",
    media: [{ type: "artwork" }],
    title: "Data-powered Notion-style features",
    body: "Calendar Colors and data-powered Kanban features can use your Anytype API connection. BlobUI only reads the spaces allowed by your API key.",
  },
];

function OnboardingMediaItem({ item, fallbackKind }) {
  if (item.type === "video") {
    return <video className="onboarding-preview-video" src={item.src} autoPlay muted loop playsInline preload="metadata" aria-label={item.alt || "Feature preview video"} />;
  }
  if (item.type === "image") {
    return <img className="onboarding-preview-image" src={item.src} alt={item.alt || "Feature preview"} />;
  }
  return <PreviewArtwork kind={item.kind || fallbackKind} />;
}

export function OnboardingPreviewMedia({ preview }) {
  const configuredMedia = Array.isArray(preview.media) && preview.media.length
    ? preview.media
    : [{ type: "artwork", kind: preview.key }];
  const video = configuredMedia.find(item => item.type === "video");
  const slides = video ? [video] : configuredMedia;
  const [activeSlide, setActiveSlide] = useState(0);
  useEffect(() => {
    setActiveSlide(0);
    if (slides.length < 2) return undefined;
    const reduced = document.documentElement.dataset.reducedMotion === "true"
      || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    if (reduced) return undefined;
    const timer = window.setInterval(() => {
      setActiveSlide(current => (current + 1) % slides.length);
    }, 3200);
    return () => window.clearInterval(timer);
  }, [preview.key, slides.length]);
  if (video) return <OnboardingMediaItem item={video} fallbackKind={preview.key} />;
  return <div className="onboarding-preview-slideshow">
    {slides.map((item, index) => <div key={(item.src || item.kind || "slide") + "-" + index}
      className={"onboarding-preview-slide" + (index === activeSlide ? " active" : "")}
      aria-hidden={index === activeSlide ? undefined : "true"}>
      <OnboardingMediaItem item={item} fallbackKind={preview.key} />
    </div>)}
  </div>;
}

function OnboardingPreview() {
  return <aside className="onboarding-preview" aria-label="Feature preview">
    {onboardingPreviews.map((preview, index) => <div key={preview.key} id={"onboarding-preview-" + preview.key} className="onboarding-preview-page" hidden={index !== 0}>
      <div className="onboarding-preview-media"><OnboardingPreviewMedia preview={preview} /></div>
      <div className="onboarding-preview-text">
        <h2>{preview.title}</h2>
        <p>{preview.body}</p>
      </div>
    </div>)}
  </aside>;
}

function OnboardingGuideReader() {
  const [requested, setRequested] = useState(false);
  const [payload, setPayload] = useState(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    const open = () => setRequested(true);
    window.addEventListener("blobui-onboarding-guide-open", open);
    return () => window.removeEventListener("blobui-onboarding-guide-open", open);
  }, []);
  useEffect(() => {
    if (!requested || payload) return undefined;
    let active = true;
    loadMarkdownFile("./anyblob-guide.md").then(result => {
      if (!active) return;
      setPayload(result);
      setFailed(false);
    }).catch(() => {
      if (active) setFailed(true);
    });
    return () => { active = false; };
  }, [requested, payload]);
  if (!requested) return <p className="onboarding-guide-status" role="status">Loading the Anyblob guide…</p>;
  if (failed) return <p className="onboarding-guide-status" role="alert">Could not load the Anyblob guide.</p>;
  if (!payload) return <p className="onboarding-guide-status" role="status">Loading the Anyblob guide…</p>;
  return <MarkdownView id="onboarding-guide-content" className="documentation-article markdown-content" aria-label="Anyblob setup guide"
    markdown={payload.markdown} images={payload.images} videos={payload.videos} />;
}

function ArrowIcon({ direction }) {
  return <svg viewBox="0 0 20 20" aria-hidden="true"><path d={direction === "back" ? "M12 4 6 10l6 6M7 10h9" : "M8 4l6 6-6 6M4 10h9"} /></svg>;
}

export function Onboarding() {
  return <section id="onboarding" className="onboarding" aria-label="BlobUI onboarding" hidden>
    <Button id="onboarding-skip" variant="text" className="onboarding-skip">Skip</Button>
    <Button id="onboarding-welcome" className="onboarding-welcome" aria-labelledby="onboarding-title" aria-describedby="onboarding-hint">
      <span className="onboarding-copy">
        <span id="onboarding-title" className="onboarding-title"><span className="onboarding-title-glitch">Welcome to BlobUI</span></span>
        <span id="onboarding-hint" className="onboarding-hint">Click anywhere to continue.</span>
      </span>
    </Button>
    <section id="onboarding-preferences" className="onboarding-api" aria-labelledby="onboarding-preferences-title" hidden>
      <div className="onboarding-scroll" id="onboarding-scroll">
        <header className="onboarding-api-heading onboarding-question-heading">
          <div className="onboarding-title-row">
            <div className="onboarding-title-anchor">
              <h1 id="onboarding-preferences-title" tabIndex={-1}>How would you like to use BlobUI?</h1>
              <span id="onboarding-warning-write" className="onboarding-requirement" aria-hidden="true">
                <HelpTooltip label="Anyblob API requirement" variant="warning" size="large" text="Requires an Anytype API v2 key with likely read-write access to at least 1 space!" />
              </span>
              <span id="onboarding-warning-read" className="onboarding-requirement" aria-hidden="true">
                <HelpTooltip label="Anytype API requirement" variant="warning" size="large" text="Requires an Anytype API v2 key with read access to at least 1 space!" />
              </span>
            </div>
          </div>
          <p>Choose every goal that applies. Nothing is saved until onboarding finishes.</p>
        </header>
        <div className="onboarding-questions-layout">
          <div className="onboarding-choice-list" role="group" aria-label="BlobUI setup choices">
            <OnboardingChoice id="onboarding-choice-interface" inputId="onboarding-option-interface" title="I want to customise my UI interface"
              description="Dark Mode+, the Blob Pink accent, and system-colored selections." />
            <OnboardingChoice id="onboarding-choice-annoying" inputId="onboarding-option-annoying" title="I want to make Anytype less annoying"
              description="Hide hover previews and use balanced Kanban spacing." />
            <OnboardingChoice id="onboarding-choice-codex" inputId="onboarding-option-codex" title="I want to connect Anytype with Codex"
              description="Enable Anyblob, object mentions, and the AGENTS.md page." />
            <OnboardingChoice id="onboarding-choice-notion" inputId="onboarding-option-notion" title="I want to have more Notion features in Anytype"
              description="Connect the API features used by calendars and Kanban boards." />
          </div>
          <OnboardingPreview />
        </div>
        <p id="onboarding-choice-error" className="field-error" role="alert" />
      </div>
    </section>
    <section id="onboarding-connection" className="onboarding-api" aria-labelledby="connection-title" hidden>
      <div className="onboarding-scroll" id="connection-scroll">
        <div className="connection-content">
          <header className="onboarding-api-heading">
            <h1 id="connection-title" tabIndex={-1}>Connect to Anytype</h1>
            <p id="connection-intro">Add your read-only API key to enable Calendar Colors and data-powered features.</p>
          </header>
          <form id="connection-form" className="connection-form" noValidate>
            <div className="font-fields">
              <div className="input-field">
                <label htmlFor="connection-key">API key</label>
                <div id="connection-key-field" className="visibility-field" data-filled="false">
                  <input id="connection-key" type="password" autoComplete="off" spellCheck={false} maxLength={4096} placeholder="Paste your API v2 key" aria-describedby="connection-error" required />
                  <Button id="connection-key-visibility" className="visibility-toggle" aria-controls="connection-key" aria-pressed="false" aria-label="Show API key" title="Show API key">
                    <svg className="visibility-icon" viewBox="0 0 20 20" aria-hidden="true"><path className="visibility-eye" d="M2 10s3-5 8-5 8 5 8 5-3 5-8 5-8-5-8-5Z" /><circle className="visibility-eye" cx="10" cy="10" r="2.25" /><path className="visibility-slash" d="m3 3 14 14" /></svg>
                  </Button>
                </div>
              </div>
              <div className="input-field">
                <label htmlFor="connection-port">Port <span className="optional-label">(optional)</span></label>
                <div id="connection-port-field" className="visibility-field" data-filled="false">
                  <input id="connection-port" type="text" inputMode="numeric" autoComplete="off" maxLength={5} placeholder="31009" aria-describedby="connection-port-hint connection-error" />
                  <Button id="connection-port-visibility" className="visibility-toggle" aria-controls="connection-port" aria-pressed="true" aria-label="Hide port" title="Hide port">
                    <svg className="visibility-icon" viewBox="0 0 20 20" aria-hidden="true"><path className="visibility-eye" d="M2 10s3-5 8-5 8 5 8 5-3 5-8 5-8-5-8-5Z" /><circle className="visibility-eye" cx="10" cy="10" r="2.25" /><path className="visibility-slash" d="m3 3 14 14" /></svg>
                  </Button>
                </div>
              </div>
            </div>
            <p id="connection-port-hint" className="input-hint">Leave the port blank to use 31009. Leave the key blank to use the key already saved in BlobUI.</p>
            <section className="connection-guide" aria-labelledby="connection-guide-title">
              <h2 id="connection-guide-title">Create an API key</h2>
              <ol>
                <li>Open Anytype’s settings and choose <strong>Local API</strong>.</li>
                <li>Turn on <strong>Enabled</strong>. Keep the port at <strong>31009</strong>, or enter your custom port above.</li>
                <li>Click <strong>Create new</strong> and name the app <strong>BlobUI</strong>.</li>
                <li>Choose the spaces BlobUI can read. Select <strong>Allow access to all spaces</strong> for features across your workspace.</li>
                <li>Set <strong>Access</strong> to <strong id="connection-access-level">Read only</strong> and <strong>Expires</strong> to <strong>Never</strong>.</li>
                <li>Click <strong>Create key</strong>, copy the API v2 key, and paste it above.</li>
              </ol>
              <p id="connection-key-note">Your key stays on this computer, encrypted for your Windows account. BlobUI only reads the spaces you allow.</p>
            </section>
            <p id="connection-error" className="field-error" role="alert" />
          </form>
        </div>
      </div>
    </section>
    <section id="onboarding-guide" className="onboarding-api onboarding-guide-page" aria-labelledby="onboarding-guide-title" hidden>
      <div className="onboarding-scroll onboarding-guide-scroll" id="onboarding-guide-scroll">
        <header className="onboarding-api-heading">
          <h1 id="onboarding-guide-title" tabIndex={-1}>Anyblob setup guide</h1>
        </header>
        <OnboardingGuideReader />
      </div>
    </section>
    <div className="onboarding-navigation" aria-label="Onboarding navigation">
      <Button id="onboarding-back" variant="icon" aria-label="Back" title="Back" disabled><ArrowIcon direction="back" /></Button>
      <Button id="onboarding-forward" variant="icon" aria-label="Continue" title="Continue"><ArrowIcon direction="forward" /></Button>
    </div>
  </section>;
}

export function Sidebar({ navigation, saveVisible, saveDisabled, discardDisabled, saveBusy, onSave, onDiscard, logRows, logSeverity, copyLogsCopied = false,
  updateNowVisible = false, updateNowDisabled = false, updateNowTitle = "Update now", onUpdateNow,
  blobuiVersionLabel = "BlobUI version", anytypeVersionLabel = "Anytype version" }) {
  return <aside className="sidebar" aria-label="Settings navigation">
    {navigation}
    <div className="sidebar-bottom"><div id="sidebar-actions" className="sidebar-actions">
      <Button id="save-button" className={"sidebar-action save-button" + (saveBusy ? " busy" : "")} title="Save changes" hidden={!saveVisible} disabled={saveDisabled} onClick={onSave}><svg className="action-icon save-icon" viewBox="0 0 20 20" aria-hidden="true"><path d="m4 10 4 4 8-8" /></svg><span className="save-spinner" aria-hidden="true" /><span className="save-label sr-only">{saveBusy ? "Saving…" : "Save changes"}</span></Button>
      <Button id="discard-button" className="sidebar-action discard-button" title="Undo changes" aria-label="Undo changes" hidden={!saveVisible} disabled={discardDisabled} onClick={onDiscard}><svg className="action-icon" viewBox="0 0 20 20" aria-hidden="true"><path d="M4 8V3m0 5h5M4.3 7.8A7 7 0 1 1 4 12" /></svg></Button>
      <Button id="update-now-button" className="sidebar-action update-now-button" title={updateNowTitle} aria-label={updateNowTitle} hidden={!updateNowVisible} disabled={updateNowDisabled} onClick={onUpdateNow}><svg className="action-icon" viewBox="0 0 20 20" aria-hidden="true"><path d="M10 3v10m-4-4 4 4 4-4M4 16h12" /></svg></Button>
      <div className="log-control" id="log-control" hidden={logRows.length === 0}>
        <Button id="log-button" className="sidebar-action log-button" title={logSeverity === "error" ? "Logs contain errors" : logSeverity === "warning" ? "Logs contain warnings" : "Logs"} aria-expanded="false" aria-controls="log-popup" hidden={logRows.length === 0} data-severity={logSeverity}><svg className="action-icon" viewBox="0 0 20 20" aria-hidden="true">{logSeverity === "error" || logSeverity === "warning" ? <><path d="M10 4v8" strokeWidth="3" /><circle cx="10" cy="16" r="1.5" fill="currentColor" stroke="none" /></> : <path d="M7 5h10M7 10h10M7 15h10M3 5h.01M3 10h.01M3 15h.01" />}</svg><span id="log-label" className="sr-only">{logSeverity === "error" ? "Errors and logs" : logSeverity === "warning" ? "Warnings and logs" : "Logs"}</span></Button>
        <section id="log-popup" className="log-popup" aria-labelledby="log-title" hidden>
          <div className="log-heading"><h2 id="log-title">Logs</h2><div className="log-heading-actions"><Button id="copy-logs" className="copy-logs" aria-label={copyLogsCopied ? "Logs copied" : "Copy all logs"} title={copyLogsCopied ? "Logs copied" : "Copy all logs"} data-copied={String(copyLogsCopied)}><CopyIcon /><CopySuccessIcon /></Button><Button id="clear-logs" className="clear-logs" aria-label="Clear logs"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m5 5 10 10M15 5 5 15" /></svg></Button></div></div>
          <div id="log-entries" className="log-entries" role="log" aria-live="polite">{logRows.map(({ text, level }, index) => <p key={index} className={"log-entry " + level}>{text}</p>)}</div><span id="copy-status" className="sr-only" role="status" />
        </section>
      </div>
      <div className="help-control" id="help-control">
        <Button id="help-button" className="sidebar-action help-button" title="Help" aria-label="Help" aria-expanded="false" aria-controls="help-popup"><svg className="action-icon" viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7.25" /><circle cx="10" cy="6.4" r=".85" fill="currentColor" stroke="none" /><path d="M10 9v5" /></svg></Button>
        <section id="help-popup" className="help-popup" aria-label="Help" hidden>
          <Button id="help-whats-new" className="help-item"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 2.5 12 7l4.8.5-3.6 3.2 1 4.8-4.2-2.5-4.2 2.5 1-4.8-3.6-3.2L8 7z" /></svg><span>What’s New?</span></Button>
          <Button id="help-dev-guide" className="help-item"><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M15.9 3.5a5.2 5.2 0 0 0-6.7 6.3l-5.8 5.8a1.6 1.6 0 0 0 2.3 2.3l5.8-5.8a5.2 5.2 0 0 0 6.3-6.7l-3.2 3.2-2.6-2.6z" /></svg><span>Dev Guide</span></Button>
          <Button id="help-keybinds" className="help-item"><svg viewBox="0 0 20 20" aria-hidden="true"><rect x="2.5" y="4" width="15" height="12" rx="2" /><path d="M5 8h.01M8 8h.01M11 8h.01M14 8h.01M5 11h.01M8 11h4M14 11h.01M6 14h8" /></svg><span>Keybinds</span></Button>
          <div className="help-divider" aria-hidden="true" /><div className="help-meta"><span id="blobui-version">{blobuiVersionLabel}</span><span id="anytype-version">{anytypeVersionLabel}</span></div>
          <Button id="open-author" className="help-credit" aria-label="Made by @PainterFlow11" title="https://github.com/PainterFlow/Anytype-BlobUI">Made by @PainterFlow11</Button>
          <Button id="open-repository" className="help-credit" aria-label="Licensed under GPL V3" title="https://github.com/PainterFlow/Anytype-BlobUI">Licensed under GPL V3</Button>
        </section>
      </div>
    </div></div>
  </aside>;
}

function Dialog({ backdropId, dialogId, dialogClass = "confirm-dialog", role = "dialog", labelledBy, describedBy, label, children }) {
  return <div id={backdropId} className="confirm-backdrop" hidden>
    <section id={dialogId} className={dialogClass} role={role} aria-modal="true" aria-labelledby={labelledBy} aria-describedby={describedBy} aria-label={label} tabIndex={-1}>{children}</section>
  </div>;
}

function CloseButton({ id, label, title }) {
  return <Button id={id} className="confirm-close" aria-label={label} title={title}><CloseIcon /></Button>;
}

const keySymbols = { Shift: "⇧", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→" };

function ShortcutBindings({ rows, onCapture }) {
  return <div id="keybind-list" className="keybind-list">
    {rows.map(({ action, label, fixed, disabled, capturing, shortcut }) => {
      const labelId = "keybind-label-" + action;
      const id = "keybind-button-" + action;
      const content = capturing ? "Press a shortcut…" : !shortcut ? "Unavailable"
        : shortcut.split("+").map((key, index) =>
          <kbd key={index} className="keybind-keycap" aria-hidden="true">{keySymbols[key] || key}</kbd>
        );
      const attributes = { id, "aria-labelledby": labelId + " " + id, "aria-label": capturing || !shortcut ? undefined : shortcut };
      return <div key={action} className="keybind-row">
        <span id={labelId} className="keybind-label">{label}</span>
        {fixed
          ? <span {...attributes} className="keybind-button keybind-fixed" role="img" title="Fixed shortcut. Available in-app only.">{content}</span>
          : <Button {...attributes} className="keybind-button" disabled={disabled} data-shortcut-action={action}
              aria-pressed={capturing} onClick={() => onCapture(action)}>{content}</Button>}
      </div>;
    })}
  </div>;
}

export function AppDialogs({ formulaContent, releaseContent, guideContent, shortcutRows, onShortcutCapture }) {
  return <>
    <Dialog backdropId="formula-calculator-backdrop" dialogId="formula-calculator-dialog" dialogClass="confirm-dialog formula-calculator-dialog" label="Kanban calculation"><Button id="formula-calculator-close" className="confirm-close" aria-label="Close calculator" title="Close"><CloseIcon /></Button>{formulaContent}</Dialog>
    <Dialog backdropId="uninstall-backdrop" dialogId="uninstall-dialog" role="alertdialog" labelledBy="uninstall-title" describedBy="uninstall-message"><CloseButton id="uninstall-close" label="Cancel uninstall" /><h2 id="uninstall-title">Uninstall BlobUI?</h2><p id="uninstall-message">This will close Settings and remove BlobUI and its stylesheet. Your Anytype data, BlobUI settings, CSS backups, and installer will be kept. Unsaved changes will be lost.</p><p id="uninstall-error" role="alert" /><div className="confirm-actions"><Button id="uninstall-cancel" className="confirm-cancel">Cancel</Button><Button id="uninstall-confirm" className="confirm-save danger-button">Uninstall</Button></div></Dialog>
    <Dialog backdropId="blob-import-backdrop" dialogId="blob-import-dialog" role="alertdialog" labelledBy="blob-import-title" describedBy="blob-import-message"><CloseButton id="blob-import-close" label="Cancel backup import" /><h2 id="blob-import-title">Import this BlobUI backup?</h2><p id="blob-import-message">This replaces the complete BlobUI folder for the selected Anytype installation. The current folder is kept if the restore fails.</p><p id="blob-import-path" className="blob-import-path" /><p id="blob-import-error" role="alert" /><div className="confirm-actions"><Button id="blob-import-cancel" className="confirm-cancel">Cancel</Button><Button id="blob-import-confirm" className="confirm-save">Import backup</Button></div></Dialog>
    <Dialog backdropId="api-key-removal-backdrop" dialogId="api-key-removal-dialog" role="alertdialog" labelledBy="api-key-removal-title" describedBy="api-key-removal-message"><CloseButton id="api-key-removal-close" label="Undo all unsaved changes" title="Undo all changes" /><h2 id="api-key-removal-title">Are you sure you want to do this?</h2><p id="api-key-removal-message">Removing the API key will make some features unavailable.</p><div className="confirm-actions"><Button id="api-key-removal-undo" className="confirm-undo">Undo</Button><Button id="api-key-removal-save" className="confirm-save">Save</Button></div></Dialog>
    <Dialog backdropId="discard-confirmation-backdrop" dialogId="discard-confirmation-dialog" role="alertdialog" labelledBy="discard-confirmation-title" describedBy="discard-confirmation-message"><CloseButton id="discard-confirmation-close" label="Close without undoing" title="Close" /><h2 id="discard-confirmation-title">Undo all unsaved changes?</h2><p id="discard-confirmation-message">This will restore the settings from before your edits.</p><div className="confirm-actions"><Button id="discard-confirmation-undo" className="confirm-undo">Undo</Button></div></Dialog>
    <Dialog backdropId="documentation-backdrop" dialogId="documentation-dialog" dialogClass="confirm-dialog documentation-dialog" labelledBy="documentation-title"><CloseButton id="documentation-close" label="Close Developer Guide" title="Close" /><header className="documentation-dialog-heading"><h1 id="documentation-title">Developer Guide</h1></header>{guideContent}</Dialog>
    <Dialog backdropId="release-notes-backdrop" dialogId="release-notes-dialog" dialogClass="confirm-dialog release-notes-dialog" label="Changelog"><div className="release-notes-heading" aria-live="polite"><h2>Changelog</h2><span id="release-notes-version" className="release-notes-version" /><span id="release-notes-current-tag" className="release-notes-tag" hidden>Current</span><span id="release-notes-latest-tag" className="release-notes-tag latest" hidden>Latest</span></div><p id="release-notes-status" className="release-notes-status" role="status" hidden /><Button id="release-notes-retry" className="banner-image-button" hidden>Try again</Button><p id="release-notes-image-warning" className="release-notes-image-warning" role="status" hidden>Some release note media could not be loaded.</p>{releaseContent}<div className="release-notes-navigation" aria-label="Changelog navigation"><Button id="release-notes-previous" className="release-notes-nav-button" disabled>Previous</Button><Button id="release-notes-after" className="release-notes-nav-button" hidden>After</Button></div></Dialog>
    <Dialog backdropId="help-error-backdrop" dialogId="help-error-dialog" dialogClass="confirm-dialog help-error-dialog" role="alertdialog" labelledBy="help-error-title" describedBy="help-error-message"><CloseButton id="help-error-close" label="Close error" /><h2 id="help-error-title"><span className="help-error-code">404</span> Error</h2><p id="help-error-message">This help content could not be found.</p></Dialog>
    <Dialog backdropId="keybinds-backdrop" dialogId="keybinds-dialog" dialogClass="confirm-dialog keybinds-dialog" labelledBy="keybinds-title" describedBy="keybinds-description"><CloseButton id="keybinds-close" label="Close keyboard shortcuts" title="Close" /><h2 id="keybinds-title">Keyboard shortcuts</h2><div id="keybinds-content" className="keybinds-content"><p id="keybinds-description">Click a shortcut to change it. Changes are saved automatically.</p><ShortcutBindings rows={shortcutRows} onCapture={onShortcutCapture} /><p id="keybind-status" className="keybind-status" role="status" aria-live="polite" /></div></Dialog>
  </>;
}
