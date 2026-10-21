const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

// Exercise the actual event handlers without opening a browser or desktop window.
const elements = new Map();
let activeElement = null;
function element(id) {
  if (!elements.has(id)) {
    const classes = new Set();
    elements.set(id, {
      id,
      value: '', type: ['connection-key', 'advanced-api-key'].includes(id) ? 'password' : 'text',
      selectionStart: 0, selectionEnd: 0,
      setSelectionRange(start, end) { this.selectionStart = start; this.selectionEnd = end; },
      textContent: '', hidden: true, children: [], attributes: {}, style: { setProperty() {} }, dataset: {},
      clientHeight: 500, scrollHeight: 500,
      classList: {
        add: (...names) => names.forEach(name => classes.add(name)),
        remove: (...names) => names.forEach(name => classes.delete(name)),
        contains: name => classes.has(name),
        toggle(name, active) { active ? classes.add(name) : classes.delete(name); },
      },
      handlers: {}, files: [], clickCount: 0,
      addEventListener(name, handler) {
        const previous = this.handlers[name];
        this.handlers[name] = previous ? (...args) => {
          const previousResult = previous(...args);
          const result = handler(...args);
          return result === undefined ? previousResult : result;
        } : handler;
      },
      setAttribute(name, value) { this.attributes[name] = value; },
      removeAttribute(name) { delete this.attributes[name]; if (name === 'src') this.src = undefined; },
      click() { this.clickCount += 1; if (this.handlers.click) this.handlers.click({ target: this }); },
      getAttribute(name) { return this.attributes[name] || null; },
      appendChild(child) { this.children.push(child); },
      append(...children) { this.children.push(...children); },
      replaceChildren(...children) { this.children = [...children]; },
      contains(target) { return target === this || this.children.includes(target); },
      closest() { return null; },
      getClientRects() { return this.hidden ? [] : [{}]; },
      focus() { this.focused = true; activeElement = this; },
      querySelector(selector) {
        const match = selector.match(/^\[data-shortcut-action="([^"]+)"\]$/);
        if (id === 'keybind-list' && match) {
          return this.children.flatMap(row => row.children).find(child => child.dataset.shortcutAction === match[1]) || element(id + selector);
        }
        return element(id + selector);
      },
      querySelectorAll() { return this.children; },
    });
  }
  return elements.get(id);
}
const calls = [];
const savedSettings = [];
const savedUpdaterSettings = [];
const savedShortcuts = [];
const releaseNotesScenarioArg = process.argv.find(argument => argument.startsWith('--release-notes-scenario='));
const releaseNotesScenario = releaseNotesScenarioArg ? releaseNotesScenarioArg.split('=', 2)[1] : null;
const loadErrorScenarioArg = process.argv.find(argument => argument.startsWith('--load-error-scenario='));
const loadErrorScenario = loadErrorScenarioArg ? loadErrorScenarioArg.split('=', 2)[1] : null;
const releaseNotesPayload = releaseNotesScenario && releaseNotesScenario !== 'parse-error'
  ? { version: '1.2.0', markdown: '# Fixture release notes', imageManifest: 'images: []' }
  : releaseNotesScenario === 'parse-error' ? { version: '1.2.0', markdown: '# Fixture release notes', imageManifest: 'invalid: [' } : null;
const releaseNotesAcknowledgements = [];
const updaterPrefs = {
  installDirectory: '%APPDATA%\\anytype',
  startOnLogin: false,
  automaticUpdatesEnabled: false,
  automaticChecksEnabled: !!releaseNotesScenario,
};
let updaterCheckState = {
  currentVersion: '1.0.0', latestVersion: null, updateAvailable: false,
  lastCheckedAt: null, status: 'current', message: "You're up to date.",
};
let updaterApplyState = { ...updaterCheckState };
let selectedFolder = null;
let recoveryCompleted = false;
const recoveryRequests = [];
let deferGeneralSave = false;
let pendingGeneralSaveResolve = null;
let deferUpdateApply = false;
let pendingUpdateApplyReject = null;
let rebuildListener = null;
const timers = new Map();
const intervals = [];
let nextTimerId = 1;
let saveError = null;
let connectionError = null;
const connectionRequests = [];
const fileReaders = [];
let delayFileRead = false;
let copiedLogs = '';
let copyError = false;
const context = {
  navigator: { clipboard: { async writeText(text) { if (copyError) throw new Error('Clipboard unavailable.'); copiedLogs = text; } } },
  document: {
    handlers: {},
    get activeElement() { return activeElement; },
    addEventListener(name, handler) {
      const previous = this.handlers[name];
      this.handlers[name] = previous ? (...args) => {
        const previousResult = previous(...args);
        const result = handler(...args);
        return result === undefined ? previousResult : result;
      } : handler;
    },
    documentElement: element('root'),
    getElementById: element,
    querySelector: selector => selector === '.page-content' ? element('page-content') : null,
    createElement: () => element('row-' + elements.size),
    querySelectorAll: () => ['accent-hue', 'accent-saturation', 'accent-brightness', 'native-accent', 'accent-value', 'font-family', 'code-font-family', 'reduced-motion', 'tab-hover-effect', 'link-hover-effect', 'object-mention-hover-effect', 'install-directory', 'start-on-login', 'automatic-updates-enabled', 'automatic-checks-enabled', 'advanced-api-key', 'advanced-api-ip', 'advanced-api-port'].map(element),
  },
  CSS: { supports: () => true },
  atob,
  FileReader: class {
    readAsDataURL(file) {
      this.result = file.dataUrl;
      fileReaders.push(this);
      if (!delayFileRead) setImmediate(() => this.onload && this.onload());
    }
  },
  createImageBitmap: async file => ({ width: file.width || 1600, height: file.height || 900, close() { file.closed = true; } }),
  window: {
    handlers: {},
    CSS: { supports: () => true },
    setInterval(callback, delay) { intervals.push({ callback, delay }); return intervals.length; },
    setTimeout(callback, delay) {
      const id = nextTimerId++;
      timers.set(id, { callback, delay });
      return id;
    },
    clearTimeout(id) { timers.delete(id); },
    addEventListener(name, handler) { this.handlers[name] = handler; },
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    __TAURI__: {
      event: { async listen(name, handler) {
        if (name === 'blobui-rebuild-custom-css') rebuildListener = handler;
        else assert.equal(name, 'blobui-show-onboarding');
      } },
      core: { async invoke(name, args) {
      calls.push(name);
      if (name === 'get_keyboard_shortcuts') return [
        { action: 'openSettings', code: 'Numpad1', ctrl: false, alt: false, shift: false, meta: false },
        { action: 'rebuildCss', code: 'KeyR', ctrl: false, alt: true, shift: false, meta: false },
        { action: 'showOnboarding', code: 'F8', ctrl: false, alt: false, shift: false, meta: false },
        { action: 'simulateUpdate', code: 'F9', ctrl: false, alt: false, shift: false, meta: false },
        { action: 'quitSettings', code: 'KeyQ', ctrl: true, alt: false, shift: false, meta: false },
      ];
      if (name === 'save_keyboard_shortcuts') {
        savedShortcuts.push(args.shortcuts);
        return args.shortcuts;
      }
      if (name === 'get_anytype_version') return 'Anytype 0.1.0';
      if (name === 'select_anytype_folder') return selectedFolder;
      if (name === 'recover_install_directory') {
        recoveryRequests.push(args.installDirectory);
        recoveryCompleted = true;
        Object.assign(updaterPrefs, { installDirectory: args.installDirectory });
        return;
      }
      if (name === 'reveal_api_key') return 'fixture-revealed-key';
      if (name === 'get_app_state') {
        if (loadErrorScenario && !recoveryCompleted) throw new Error('BlobUI data.json is not valid JSON.');
        return {
        ready: true, theme: 'dark',
        apiPort: 31009, apiIp: '', hasSavedApiKey: true,
        defaultBannerPreview: 'data:image/png;base64,default-preview',
        settings: {
          fontFamily: 'Libron', codeFontFamily: 'Consolas', accentColor: 'Blob Pink',
          bannerImage: '',
          tabHoverEffect: false, linkHoverEffect: false, objectMentionHoverEffect: false,
        },
        };
      }
      if (name === 'get_updater_state') {
        if (loadErrorScenario === 'preferences' && !recoveryCompleted) throw new Error('The saved BlobUI updater settings are not valid JSON.');
        return {
        ...updaterPrefs, currentVersion: '1.0.0', latestVersion: null, updateAvailable: false,
        lastCheckedAt: null, status: 'idle', message: null,
        };
      }
      if (name === 'get_release_notes') return releaseNotesPayload;
      if (name === 'get_adjacent_release_notes') {
        if (releaseNotesScenario === 'offline') throw new Error('offline');
        if (releaseNotesScenario === 'history' && args.older) return {
          version: '1.1.0', tag: 'v1.1.0', markdown: '# Older fixture notes', imageManifest: 'images: []',
        };
        return null;
      }
      if (name === 'get_release_note_image') return 'data:image/png;base64,fixture';
      if (name === 'acknowledge_release_notes') {
        releaseNotesAcknowledgements.push(args.version);
        return true;
      }
      if (name === 'save_updater_settings') {
        Object.assign(updaterPrefs, args.settings);
        savedUpdaterSettings.push({ ...args.settings });
        return { ...updaterPrefs, ...updaterCheckState };
      }
      if (name === 'check_for_updates') return { ...updaterPrefs, ...updaterCheckState };
      if (name === 'apply_update') {
        if (deferUpdateApply) return new Promise((resolve, reject) => { pendingUpdateApplyReject = reject; });
        return { ...updaterPrefs, ...updaterApplyState };
      }
      if (name === 'uninstall_blobui') throw new Error('Uninstall fixture failed.');
      if (name === 'simulate_update') return {
        ...updaterPrefs, currentVersion: '1.0.0', latestVersion: '1.1.0', updateAvailable: true,
        lastCheckedAt: null, status: 'simulated', message: 'Preview update available.',
      };
      if (name === 'configure_connection') {
        connectionRequests.push(args.connection);
        if (connectionError) throw new Error(connectionError);
      }
      if (name === 'save_settings') {
        savedSettings.push(args.settings);
        if (saveError) throw new Error(saveError);
        if (deferGeneralSave) return new Promise(resolve => { pendingGeneralSaveResolve = resolve; });
        return { calendarStatus: 'cached' };
      }
    } } },
  },
};
const flush = () => new Promise(resolve => setImmediate(resolve));
function finishReleaseNotesTransition() {
  const pending = [...timers.entries()].find(([, timer]) => timer.delay === 140);
  assert(pending, 'Closing release notes must schedule the modal exit transition.');
  timers.delete(pending[0]);
  pending[1].callback();
}
const input = async (id, value) => {
  element(id).value = value;
  await element(id).handlers.input();
};
const changeToggle = async (id, checked) => {
  element(id).checked = checked;
  await element(id).handlers.change();
};
const status = () => element('status-message').textContent;
const settingsHtml = fs.readFileSync(path.join(__dirname, '../ui/index.html'), 'utf8');
const settingsCss = fs.readFileSync(path.join(__dirname, '../ui/styles.css'), 'utf8');
assert.match(settingsHtml, /id="help-button"[^>]*aria-controls="help-popup"/);
assert(settingsHtml.indexOf('id="help-whats-new"') < settingsHtml.indexOf('id="help-documentation"'));
assert(settingsHtml.indexOf('id="help-documentation"') < settingsHtml.indexOf('id="help-keybinds"'));
assert(settingsHtml.indexOf('class="help-divider"') < settingsHtml.indexOf('id="blobui-version"'));
for (const shortcut of ['openSettings', 'rebuildCss', 'showOnboarding', 'simulateUpdate', 'quitSettings']) assert.match(fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8'), new RegExp(`\\"${shortcut}\\"`));
assert(settingsHtml.indexOf('id="preferences-nav"') < settingsHtml.indexOf('id="accessibility-nav"'), 'Accessibility must be the second Settings navigation page.');
assert(settingsHtml.indexOf('id="accessibility-nav"') < settingsHtml.indexOf('id="advanced-nav"'), 'Advanced must be directly below Accessibility.');
assert(!settingsHtml.includes('id="app-settings-nav"'), 'App Settings belongs inside Advanced Settings.');
assert.match(settingsHtml, /role="tablist"/);
assert.match(settingsHtml, /id="advanced-app-panel"[^>]*role="tabpanel"[^>]*hidden/);
assert.match(settingsHtml, /Anytype data folder that contains custom\.css/);
assert(settingsHtml.indexOf('id="install-directory"') < settingsHtml.indexOf('id="start-on-login"'), 'Launch at startup must follow Anytype Folder.');
assert(settingsHtml.indexOf('id="start-on-login"') < settingsHtml.indexOf('id="updates-heading"'), 'Launch at startup must precede Updates.');
assert.match(settingsHtml, /<label class="native-accent-control settings-row accessibility-row" for="start-on-login">\s*<span>Launch at startup<\/span>/, 'The startup toggle uses the concise setting row label.');
assert.match(settingsHtml, /id="install-directory"[^>]*readonly[^>]*aria-haspopup="dialog"/);
for (const label of ['Anytype API key', 'API key IP', 'API key port']) {
  assert(settingsHtml.includes('>' + label + '</label>'), `${label} must have a visible label.`);
}
for (const [inputId, buttonId] of [
  ['connection-key', 'connection-key-visibility'], ['connection-port', 'connection-port-visibility'],
  ['advanced-api-key', 'advanced-api-key-visibility'], ['advanced-api-ip', 'advanced-api-ip-visibility'],
  ['advanced-api-port', 'advanced-api-port-visibility'],
]) {
  const fieldId = inputId + '-field';
  assert.match(settingsHtml, new RegExp(`<div id="${fieldId}" class="visibility-field" data-filled="false">\\s*<input id="${inputId}"[\\s\\S]*?<button id="${buttonId}"`), `${inputId} must have an adjacent visibility control outside its input.`);
  assert.match(settingsHtml, new RegExp(`id="${buttonId}"[^>]*aria-pressed="(?:true|false)"[^>]*aria-label="[^"]+"[^>]*title="[^"]+"`), `${buttonId} must expose its state and accessible name.`);
}
assert(!settingsHtml.includes('id="connection-ip"'), 'Onboarding must not add an IP field.');
assert.match(settingsHtml, /id="connection-port" type="text" inputmode="numeric"/);
assert.match(settingsHtml, /id="advanced-api-ip" type="text" inputmode="decimal"/);
assert.match(settingsHtml, /id="advanced-api-port" type="text" inputmode="numeric"/);
assert.match(settingsHtml, /id="banner-image-input" class="sr-only" type="file"[^>]*accept="[^"]*image\/png[^"]*image\/jpeg[^"]*image\/webp/);
assert.match(settingsHtml, /class="space-sidebar-preview" inert role="img"/);
for (const label of ['Space', 'Homepage', 'Private', 'Work']) assert(settingsHtml.includes('>' + label + '<'), `The inert space preview must show the generic ${label} label.`);
assert.match(settingsCss, /\.space-preview-row[^}]*font-family:\s*var\(--font-interface\)/);
assert.match(settingsCss, /\.visibility-field\s*\{[^}]*width:\s*54%/);
assert.match(settingsCss, /\.visibility-field:hover \.visibility-toggle/);
assert.match(settingsCss, /\.visibility-field:focus-within \.visibility-toggle/);
assert.match(settingsCss, /\.visibility-field\[data-filled="true"\] \.visibility-toggle/);
assert.match(settingsCss, /\.visibility-toggle:focus-visible/);
assert.match(settingsHtml, /role="alertdialog" aria-modal="true"/, 'Key removal must use an accessible modal dialog.');
assert.match(settingsHtml, /id="api-key-removal-title">Are you sure you want to do this\?<\/h2>/);
assert.match(settingsHtml, /id="api-key-removal-message">Removing the API key will make some features unavailable\.<\/p>/);
assert.match(settingsHtml, /id="discard-button"[^>]*title="Undo changes"/);
assert(!settingsHtml.includes('id="api-key-removal-dismiss"'), 'The API warning has no labeled Close button.');
assert.match(settingsHtml, /id="api-key-removal-undo"[^>]*>Undo<\/button>/);
assert.match(settingsHtml, /id="discard-confirmation-undo"[^>]*>Undo<\/button>/);
assert(!settingsHtml.includes('id="api-key-removal-cancel"'), 'The key removal warning uses a separate Close action.');
const confirmationDialogStyles = settingsCss.match(/\.confirm-dialog\s*\{([^}]*)\}/)?.[1] || '';
assert(!/opacity\s*:\s*0/.test(confirmationDialogStyles) && !/transform\s*:\s*translateY/.test(confirmationDialogStyles), 'The confirmation dialog stays visible when reduced motion disables animations.');
assert.match(settingsCss, /data-reduced-motion="true"[^}]*\.confirm-dialog[^}]*animation:\s*none/, 'Reduced Motion must disable confirmation animations.');
for (const id of ['reduced-motion', 'tab-hover-effect', 'link-hover-effect', 'object-mention-hover-effect', 'start-on-login', 'automatic-updates-enabled', 'automatic-checks-enabled']) {
  assert.match(settingsHtml, new RegExp(`id="${id}" type="checkbox" role="switch"`), `${id} must be a native switch.`);
}
assert.match(settingsCss, /\.update-now-button\s*\{[^}]*color:\s*#de50de/);
assert.match(settingsCss, /\.sidebar-actions\.has-update \.log-control\s*\{[^}]*grid-row:\s*4/);
assert(settingsHtml.indexOf('id="updates-heading"') < settingsHtml.indexOf('id="uninstall-button"'), 'Uninstall appears below Updates in Advanced Settings.');
assert.match(settingsHtml, /<button id="uninstall-button" class="banner-image-button danger-button"[^>]*>Uninstall BlobUI<\/button>/);
assert.match(settingsHtml, /id="uninstall-dialog" class="confirm-dialog" role="alertdialog" aria-modal="true"/);
assert.match(settingsHtml, /Your Anytype data, BlobUI settings, CSS backups, and installer will be kept/);
assert.match(settingsCss, /\.danger-button\s*\{[^}]*background:\s*#b8323c/);
assert.match(settingsCss, /\.danger-button:hover:not\(:disabled\)\s*\{[^}]*background:\s*#942630/);
assert(!settingsCss.includes('`n'), 'The stylesheet must contain real line breaks rather than literal PowerShell escapes.');
assert.match(fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8'), /5 \* 60 \* 1000/);

(async () => {
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/visibility-field.js'), 'utf8'), context);
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/modal.js'), 'utf8'), context);
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/edit-session.js'), 'utf8'), context);
  if (releaseNotesScenario) {
    context.window.BlobUI.Markdown = {
      parseImageManifest(value) {
        if (releaseNotesScenario === 'parse-error') throw new Error('Invalid image manifest.');
        assert.equal(value, 'images: []');
        return {};
      },
      render(markdown) { return `<p>${markdown}</p>`; },
    };
  }
  let helperState = { settings: { font: 'Libron' }, connection: { port: 31009 } };
  const helperSession = context.window.BlobUI.createEditSession({
    read: () => helperState,
    restore: snapshot => { helperState = snapshot; },
    equals: (left, right) => JSON.stringify(left) === JSON.stringify(right),
  });
  assert.equal(helperSession.isDirty(), false, 'A new edit session starts at its saved baseline.');
  helperState.settings.font = 'Arial';
  helperState.connection.port = 31010;
  helperSession.commit({ connection: { port: 31010 } });
  assert.equal(helperSession.isDirty(), true, 'A partial save advances only its saved baseline section.');
  helperSession.discard();
  assert.equal(JSON.stringify(helperState), JSON.stringify({ settings: { font: 'Libron' }, connection: { port: 31010 } }), 'Undo restores unsaved sections while retaining successful partial saves.');
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8'), context);
  const apiModalButtons = ['api-key-removal-close', 'api-key-removal-undo', 'api-key-removal-save'];
  const discardModalButtons = ['discard-confirmation-close', 'discard-confirmation-undo'];
  const uninstallModalButtons = ['uninstall-close', 'uninstall-cancel', 'uninstall-confirm'];
  const releaseNotesButtons = ['release-notes-previous', 'release-notes-after'];
  [...apiModalButtons, ...discardModalButtons, ...uninstallModalButtons, ...releaseNotesButtons].forEach(id => { element(id).hidden = false; });
  element('api-key-removal-dialog').hidden = false;
  element('api-key-removal-dialog').children.push(...apiModalButtons.map(element));
  element('discard-confirmation-dialog').hidden = false;
  element('discard-confirmation-dialog').children.push(...discardModalButtons.map(element));
  element('uninstall-button').hidden = false;
  element('uninstall-dialog').hidden = false;
  element('uninstall-dialog').children.push(...uninstallModalButtons.map(element));
  element('release-notes-dialog').hidden = false;
  element('release-notes-dialog').children.push(...releaseNotesButtons.map(element));
  assert.equal(element('log-button').hidden, true, 'Logs stay hidden before any messages arrive.');
  await flush();
  if (loadErrorScenario) {
    assert.match(status(), /not valid JSON/, 'The original state-load failure remains visible.');
    assert.equal(element('install-directory').disabled, false, 'The folder picker remains available after a profile load error.');
    assert.equal(element('start-on-login').disabled, true, 'Other updater controls stay disabled until the profile loads.');
    selectedFolder = 'C:\\Recovered Anytype';
    await element('install-directory').handlers.click();
    assert.deepEqual(recoveryRequests, [selectedFolder], 'Selecting a valid profile invokes the recovery command.');
    assert.equal(context.window.appReady, true, 'The selected profile is loaded after recovery.');
    assert.equal(element('install-directory').value, selectedFolder);
    assert.equal(status(), 'Settings loaded from this device.');
    console.log(`Profile recovery after ${loadErrorScenario} load errors passed.`);
    return;
  }
  if (releaseNotesScenario) {
    assert(calls.includes('get_release_notes'), 'Startup checks for pending release notes.');
    if (releaseNotesScenario === 'parse-error') {
      assert.equal(element('release-notes-backdrop').hidden, true, 'Invalid notes do not open a partially rendered dialog.');
      assert.deepEqual(releaseNotesAcknowledgements, [], 'A parse failure leaves the release notes pending.');
      assert.equal(calls.filter(name => name === 'check_for_updates').length, 0, 'Automatic update checks wait while pending notes cannot be rendered.');
      assert.match(status(), /Could not display the release notes/, 'The render failure remains visible in Settings.');
      console.log('Release notes parse failures stay pending and block automatic update checks.');
      return;
    }

    assert.equal(element('release-notes-backdrop').hidden, false, 'Pending notes open the release notes dialog after startup.');
    assert.equal(activeElement, element('release-notes-dialog'), 'The release notes dialog opens with its scrollable content focused.');
    await flush();
    await flush();
    if (releaseNotesScenario === 'offline') {
      assert.equal(element('release-notes-previous').disabled, true, 'History navigation stays disabled when GitHub cannot be reached.');
    }
    if (releaseNotesScenario === 'history') {
      assert.equal(element('release-notes-previous').disabled, false, 'An available older release enables Previous.');
      element('release-notes-previous').handlers.click();
      await flush();
      await flush();
      assert.equal(element('release-notes-content').innerHTML, '<p># Older fixture notes</p>', 'Previous loads the earlier release notes.');
      assert.equal(element('release-notes-after').hidden, false, 'Older notes expose the After navigation button.');
      context.navigator.onLine = false;
      context.window.handlers.offline();
      assert.equal(element('release-notes-previous').disabled, true, 'Going offline disables cached Previous navigation.');
      assert.equal(element('release-notes-after').disabled, true, 'Going offline disables After navigation.');
      context.navigator.onLine = true;
      context.window.handlers.online();
      await flush();
      assert.equal(element('release-notes-after').disabled, false, 'Reconnecting enables After navigation.');
      element('release-notes-after').handlers.click();
      await flush();
      await flush();
      assert.equal(element('release-notes-content').innerHTML, '<p># Fixture release notes</p>', 'After returns to the newer notes.');
    }
    assert.deepEqual(releaseNotesAcknowledgements, [], 'Opening the dialog does not acknowledge the notes.');
    const updaterInterval = intervals.find(interval => interval.delay === 5 * 60 * 1000);
    assert(updaterInterval, 'Automatic update checks use a five-minute interval.');
    await updaterInterval.callback();
    assert.equal(calls.filter(name => name === 'check_for_updates').length, 0, 'Automatic update checks wait until the release notes are dismissed.');

    if (releaseNotesScenario === 'backdrop') element('release-notes-backdrop').handlers.pointerdown({ target: element('release-notes-backdrop') });
    else if (['escape', 'history', 'offline'].includes(releaseNotesScenario)) context.document.handlers.keydown({ key: 'Escape', preventDefault() {} });
    else throw new Error(`Unknown release notes scenario: ${releaseNotesScenario}`);
    assert.deepEqual(releaseNotesAcknowledgements, [], 'Closing starts the modal transition but does not acknowledge early.');
    finishReleaseNotesTransition();
    await flush();
    await flush();
    assert.deepEqual(releaseNotesAcknowledgements, ['1.2.0'], 'Dismissal acknowledges only after the modal has closed.');
    assert(calls.indexOf('acknowledge_release_notes') < calls.indexOf('check_for_updates'), 'The automatic update check resumes only after acknowledgement.');
    console.log(`Release notes ${releaseNotesScenario} dismissal lifecycle passed.`);
    return;
  }
  assert.equal(status(), 'Settings loaded from this device.');
  assert.equal(element('install-directory').value, '%APPDATA%\\anytype', 'App Settings load the configured Anytype data folder.');
  assert.equal(element('start-on-login').checked, false, 'Startup is opt-in by default.');
  assert.equal(element('automatic-updates-enabled').checked, false);
  assert.equal(element('automatic-checks-enabled').checked, false);
  assert.equal(element('sidebar-actions').classList.contains('has-update'), false);
  const updaterInterval = intervals.find(interval => interval.delay === 5 * 60 * 1000);
  assert(updaterInterval, 'Automatic update checks use a five-minute interval.');
  element('advanced-nav').handlers.click();
  assert.equal(element('advanced-page').hidden, false, 'Advanced Settings shows all controls on one page.');
  assert.equal(element('page-title').textContent, 'Advanced Settings');
  const uninstallCallsBeforeCancel = calls.filter(name => name === 'uninstall_blobui').length;
  element('uninstall-button').focus();
  element('uninstall-button').handlers.click();
  assert.equal(element('uninstall-backdrop').hidden, false, 'Uninstall opens its confirmation dialog.');
  assert.equal(activeElement, element('uninstall-cancel'), 'The confirmation opens with Cancel focused.');
  element('uninstall-cancel').handlers.click();
  finishRemovalTransition();
  assert.equal(element('uninstall-backdrop').hidden, true, 'Cancel closes the uninstall dialog.');
  assert.equal(calls.filter(name => name === 'uninstall_blobui').length, uninstallCallsBeforeCancel, 'Cancel never invokes the native uninstaller.');
  element('uninstall-error').textContent = 'stale error';
  element('uninstall-button').handlers.click();
  assert.equal(element('uninstall-error').textContent, '', 'Opening the dialog clears an earlier failure.');
  await element('uninstall-confirm').handlers.click();
  assert.equal(calls.at(-1), 'uninstall_blobui', 'Confirm invokes the native uninstaller.');
  assert.match(element('uninstall-error').textContent, /Uninstall fixture failed/, 'A native uninstall error stays visible in the dialog.');
  assert.equal(element('uninstall-confirm').disabled, false, 'The confirmation button is restored after a failed or returned invocation.');
  element('uninstall-cancel').handlers.click();
  finishRemovalTransition();
  assert.equal(element('uninstall-backdrop').hidden, true, 'Cancel remains available after a native failure.');
  const initialFolder = element('install-directory').value;
  await element('install-directory').handlers.click();
  assert.equal(element('install-directory').value, initialFolder, 'Canceling the folder picker preserves the selected folder.');
  selectedFolder = 'C:\\Chosen Anytype';
  await element('install-directory').handlers.click();
  assert.equal(element('install-directory').value, selectedFolder, 'Selecting a folder updates the field.');
  context.window.BlobUI.discardChanges();
  element('check-updates-button').handlers.click();
  await flush();
  assert.equal(calls.at(-1), 'check_for_updates', 'The manual update action checks for updates.');
  assert.equal(element('check-updates-button').textContent, "You're up to date");
  const generalSavesBeforeUpdaterSave = savedSettings.length;
  await input('install-directory', 'C:\\BlobUI\\AnytypeData');
  await changeToggle('start-on-login', true);
  await changeToggle('automatic-updates-enabled', true);
  await changeToggle('automatic-checks-enabled', true);
  assert.equal(element('save-button').hidden, false, 'App Settings edits use the shared Save action.');
  await element('save-button').handlers.click();
  assert.equal(savedUpdaterSettings.length, 1);
  assert.deepEqual(savedUpdaterSettings.at(-1), {
    installDirectory: 'C:\\BlobUI\\AnytypeData', startOnLogin: true, automaticUpdatesEnabled: true, automaticChecksEnabled: true,
  });
  assert.equal(savedSettings.length, generalSavesBeforeUpdaterSave, 'Saving App Settings does not rebuild custom.css.');
  assert.match(status(), /Loaded settings from the selected Anytype data folder/);
  assert.equal(element('save-button').hidden, true);
  await input('font-family', 'Arial');
  await input('install-directory', 'C:\\BlobUI\\OtherProfile');
  const updaterSavesBeforeMixedTarget = savedUpdaterSettings.length;
  await element('save-button').handlers.click();
  assert.match(status(), /Save or undo your appearance and Anytype API changes/);
  assert.equal(savedUpdaterSettings.length, updaterSavesBeforeMixedTarget, 'Changing data folders cannot save a stale appearance draft into the new profile.');
  element('discard-button').handlers.click();
  element('discard-confirmation-undo').handlers.click();
  finishRemovalTransition();
  assert.equal(element('font-family').value, 'Libron');
  assert.equal(element('install-directory').value, 'C:\\BlobUI\\AnytypeData');
  await changeToggle('automatic-updates-enabled', false);
  await changeToggle('start-on-login', false);
  element('discard-button').handlers.click();
  element('discard-confirmation-undo').handlers.click();
  finishRemovalTransition();
  assert.equal(element('automatic-updates-enabled').checked, true, 'Undo restores saved App Settings toggles.');
  assert.equal(element('start-on-login').checked, true, 'Undo restores the startup preference.');

  element('help-button').handlers.click();
  assert.equal(element('help-popup').hidden, false, 'The Help button opens its mini-menu.');
  assert.equal(element('help-button').attributes['aria-expanded'], 'true');
  assert(element('help-whats-new').focused, 'Opening Help moves focus into the menu.');
  await element('help-keybinds').handlers.click();
  assert.equal(element('keybind-list').children.length, 5, 'Keybinds lists all five BlobUI shortcuts.');
  const shortcutButton = element('keybind-list').querySelector('[data-shortcut-action="showOnboarding"]');
  await shortcutButton.handlers.click();
  assert.equal(activeElement, element('keybind-list').querySelector('[data-shortcut-action="showOnboarding"]'), 'Shortcut capture keeps focus on the active action.');
  await context.document.handlers.keydown({
    key: 'z', code: 'KeyZ', ctrlKey: true, altKey: true, shiftKey: false, metaKey: false,
    preventDefault() {}, stopImmediatePropagation() {},
  });
  const saveShortcutCall = calls.lastIndexOf('save_keyboard_shortcuts');
  assert(saveShortcutCall >= 0, 'A valid captured combination is persisted.');
  assert.equal(savedShortcuts.length, 1);
  assert.equal(JSON.stringify(savedShortcuts[0].find(binding => binding.action === 'showOnboarding')),
    JSON.stringify({ action: 'showOnboarding', code: 'KeyZ', ctrl: true, alt: true, shift: false, meta: false }));
  assert.equal(element('keybind-status').textContent, 'Shortcut saved.');
  element('keybinds-close').handlers.click();
  finishRemovalTransition();

  const realApplyCallsBeforeSimulation = calls.filter(name => name === 'apply_update').length;
  const realCheckCallsBeforeSimulation = calls.filter(name => name === 'check_for_updates').length;
  let f9Prevented = false;
  context.document.handlers.keydown({ key: 'F9', code: 'F9', repeat: true, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, preventDefault() {} });
  context.document.handlers.keydown({ key: 'F9', code: 'F9', ctrlKey: true, altKey: false, shiftKey: false, metaKey: false, preventDefault() {} });
  assert.equal(calls.filter(name => name === 'simulate_update').length, 0, 'Repeated or modified F9 does not start a preview.');
  context.document.handlers.keydown({ key: 'F9', code: 'F9', ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, preventDefault() { f9Prevented = true; } });
  await flush();
  assert(f9Prevented);
  assert.equal(calls.at(-1), 'simulate_update', 'F9 uses the display-only update simulation command.');
  assert.equal(element('update-now-button').hidden, false, 'A simulated update reveals the pink sidebar action.');
  assert.equal(element('sidebar-actions').classList.contains('has-update'), true);
  assert.equal(element('check-updates-button').textContent, 'Update now');
  await updaterInterval.callback();
  assert.equal(calls.filter(name => name === 'check_for_updates').length, realCheckCallsBeforeSimulation, 'Scheduled checks preserve the F9 preview.');
  assert.equal(element('update-status').textContent, 'Preview update available.');
  await element('update-now-button').handlers.click();
  assert.equal(calls.filter(name => name === 'apply_update').length, realApplyCallsBeforeSimulation, 'Clicking a simulated update never invokes the installer.');
  assert.equal(element('update-now-button').hidden, true, 'Completing the preview clears its update action.');
  assert.equal(element('sidebar-actions').classList.contains('has-update'), false);
  assert.equal(element('update-status').textContent, 'Preview complete. No files were changed.');

  updaterCheckState = {
    currentVersion: '1.0.0', latestVersion: '1.2.0', updateAvailable: true,
    lastCheckedAt: null, status: 'available', message: 'Version 1.2.0 is available.',
  };
  updaterApplyState = {
    currentVersion: '1.2.0', latestVersion: '1.2.0', updateAvailable: false,
    lastCheckedAt: null, status: 'updated', message: 'Update completed.',
  };
  await input('font-family', 'Arial');
  const applyCallsBeforeDirtyCheck = calls.filter(name => name === 'apply_update').length;
  await updaterInterval.callback();
  assert.equal(calls.filter(name => name === 'apply_update').length, applyCallsBeforeDirtyCheck, 'Automatic updates wait while settings are unsaved.');
  assert.match(element('update-status').textContent, /Save or undo your pending settings/);
  await element('update-now-button').handlers.click();
  assert.equal(calls.filter(name => name === 'apply_update').length, applyCallsBeforeDirtyCheck, 'A manual update action also waits for unsaved settings.');
  element('discard-button').handlers.click();
  element('discard-confirmation-undo').handlers.click();
  finishRemovalTransition();
  await flush();
  assert.equal(calls.filter(name => name === 'apply_update').length, applyCallsBeforeDirtyCheck + 1, 'A deferred automatic update resumes after Undo clears unsaved changes.');

  updaterCheckState = { ...updaterCheckState, status: 'available' };
  await input('font-family', 'Arial');
  deferGeneralSave = true;
  const busySave = element('save-button').handlers.click();
  assert.equal(element('save-button').disabled, true, 'Save reports its busy state before awaiting the backend.');
  const applyCallsBeforeBusyCheck = calls.filter(name => name === 'apply_update').length;
  await updaterInterval.callback();
  assert.equal(calls.filter(name => name === 'apply_update').length, applyCallsBeforeBusyCheck, 'Automatic updates wait while Save is busy.');
  deferGeneralSave = false;
  pendingGeneralSaveResolve({ calendarStatus: 'cached' });
  await busySave;
  for (let turn = 0; turn < 10 && calls.filter(name => name === 'apply_update').length === applyCallsBeforeBusyCheck; turn += 1) await flush();
  assert.equal(calls.filter(name => name === 'apply_update').length, applyCallsBeforeBusyCheck + 1, 'A deferred automatic update resumes after Save completes.');
  await input('font-family', 'Libron');
  await element('save-button').handlers.click();
  assert.equal(element('save-button').hidden, true, 'The updater deferral fixture restores the original appearance baseline.');

  element('preferences-nav').handlers.click();
  assert.equal(element('reduced-motion').checked, false, 'Reduced Motion must load disabled by default.');
  assert.equal(element('kanban-view-style').value, 'default');
  assert.equal(element('collapsed-categories').checked, false);
  element('kanban-view-style').value = 'compact';
  element('kanban-view-style').handlers.change();
  await changeToggle('collapsed-categories', true);
  assert.equal(element('save-button').hidden, false, 'Kanban edits mark the draft dirty.');
  context.window.BlobUI.discardChanges();
  assert.equal(element('kanban-view-style').value, 'default', 'Undo restores the saved style.');
  assert.equal(element('collapsed-categories').checked, false, 'Undo restores collapse.');
  for (const style of ['balanced', 'compact', 'default']) {
    element('kanban-view-style').value = style;
    element('kanban-view-style').handlers.change();
    await changeToggle('collapsed-categories', style !== 'default');
    await element('save-button').handlers.click();
    await flush();
    assert.equal(savedSettings.at(-1).kanbanViewStyle, style);
    assert.equal(savedSettings.at(-1).collapsedCategories, style !== 'default');
    assert.equal(element('save-button').hidden, true, 'Save establishes the Kanban baseline.');
  }
  assert.equal(element('tab-hover-effect').checked, false, 'Tab hover previews must load disabled by default.');
  assert.equal(element('link-hover-effect').checked, false, 'Link hover previews must load disabled by default.');
  assert.equal(element('object-mention-hover-effect').checked, false, 'Object mention hover previews must load disabled by default.');
  assert.equal(element('preferences-page').hidden, false, 'Preferences must be the initial page.');
  element('accessibility-nav').handlers.click();
  assert.equal(element('accessibility-page').hidden, false, 'The Accessibility navigation item must open its page.');
  assert.equal(element('preferences-page').hidden, true, 'Opening Accessibility must hide Preferences.');
  assert.equal(element('page-title').textContent, 'Accessibility');
  assert.equal(element('accessibility-nav').attributes['aria-current'], 'page');
  await changeToggle('reduced-motion', true);
  assert.equal(element('save-button').hidden, false, 'Accessibility changes must make Save available.');
  await changeToggle('reduced-motion', false);
  assert.equal(element('save-button').hidden, true, 'Restoring an accessibility setting must clear its dirty state.');
  element('preferences-nav').handlers.click();
  assert.equal(element('accessibility-page').hidden, true, 'Preferences navigation must return to its page.');
  assert.equal(element('page-title').textContent, 'Preferences');
  element('advanced-nav').handlers.click();
  assert.equal(element('advanced-page').hidden, false, 'Advanced navigation must open its settings page.');
  assert.equal(element('accessibility-page').hidden, true, 'Opening Advanced must hide Accessibility.');
  assert.equal(element('preferences-page').hidden, true, 'Opening Advanced must hide Preferences.');
  assert.equal(element('page-title').textContent, 'Advanced Settings');
  assert.equal(element('advanced-api-key').value, '••••••••', 'A saved key is represented by a mask instead of plaintext.');
  assert.equal(element('save-button').hidden, true, 'Loaded API settings start clean.');
  assert.equal(element('advanced-api-key').type, 'password', 'The saved key mask starts concealed.');
  assert.equal(element('advanced-api-key-visibility').attributes['aria-pressed'], 'false');
  assert.equal(element('advanced-api-ip').type, 'text', 'IP starts readable.');
  assert.equal(element('advanced-api-ip-visibility').attributes['aria-pressed'], 'true');
  assert.equal(element('advanced-api-port').type, 'text', 'Port starts readable.');
  assert.equal(element('advanced-api-key-field').dataset.filled, 'true', 'A filled field keeps its eye control available.');
  assert.equal(element('advanced-api-ip-field').dataset.filled, 'false');
  const visibilityCallCount = calls.length;
  const visibilityConnectionCount = connectionRequests.length;
  element('advanced-api-key').focus();
  element('advanced-api-key').setSelectionRange(2, 5);
  await element('advanced-api-key-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('advanced-api-key').value, 'fixture-revealed-key', 'Reveal must load the saved key, not placeholder dots.');
  assert.equal(element('advanced-api-key').type, 'text', 'The key eye reveals only the current input value.');
  assert.equal(activeElement, element('advanced-api-key'), 'Mouse visibility toggles preserve input focus.');
  assert.deepEqual([element('advanced-api-key').selectionStart, element('advanced-api-key').selectionEnd], [2, 5], 'Visibility toggles preserve the caret or selection.');
  assert.equal(element('advanced-api-key-visibility').attributes['aria-pressed'], 'true');
  element('advanced-api-key-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('advanced-api-key').type, 'password');
  assert.equal(element('advanced-api-key').value, '••••••••', 'Hiding the saved key clears the revealed plaintext.');
  element('advanced-api-ip-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('advanced-api-ip').type, 'password', 'The IP eye can mask the readable value.');
  element('advanced-api-ip-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('advanced-api-ip').type, 'text');
  element('advanced-api-port-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('advanced-api-port').type, 'password');
  element('advanced-api-port-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('advanced-api-port').type, 'text');
  assert.equal(element('save-button').hidden, true, 'Visibility toggles do not make Advanced dirty.');
  assert.deepEqual(calls.slice(visibilityCallCount), ['reveal_api_key'], 'The eye only reads the key and does not save.');
  assert.equal(connectionRequests.length, visibilityConnectionCount, 'Visibility toggles do not change the connection save payload.');
  await input('advanced-api-ip', '192.168.1.24');
  assert.equal(element('advanced-api-ip-field').dataset.filled, 'true', 'Typing keeps the eye control available.');
  element('advanced-api-ip-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('advanced-api-ip').type, 'password');
  assert.equal(element('advanced-api-ip').value, '192.168.1.24', 'Masking preserves the IP value used by validation and saving.');
  assert.equal(element('save-button').hidden, false, 'Visibility does not clear unrelated dirty state.');
  element('save-button').handlers.click();
  await flush();
  assert.equal(connectionRequests.at(-1).enabled, true);
  assert.equal(connectionRequests.at(-1).apiKey, '');
  assert.equal(connectionRequests.at(-1).keepExistingKey, true, 'Changing the IP keeps the existing encrypted key.');
  assert.equal(connectionRequests.at(-1).ip, '192.168.1.24');
  assert.equal(connectionRequests.at(-1).port, null, 'A blank port uses the default port.');
  assert.equal(element('save-button').hidden, true, 'A successful connection save clears Advanced dirty state.');
  await input('advanced-api-key', '');
  element('save-button').handlers.click();
  assert.equal(element('api-key-removal-backdrop').hidden, false, 'Clearing a saved key must ask before removal.');
  element('api-key-removal-backdrop').handlers.pointerdown({ target: element('api-key-removal-dialog') });
  assert.equal(element('api-key-removal-backdrop').hidden, false, 'Pointer input inside the dialog must not cancel it.');
  element('api-key-removal-close').focus();
  context.document.handlers.keydown({ key: 'Tab', shiftKey: true, preventDefault() {} });
  assert.equal(activeElement, element('api-key-removal-save'), 'Keyboard focus must cycle inside the modal.');
  context.document.handlers.keydown({ key: 'Escape', preventDefault() {} });
  assert.equal(element('api-key-removal-backdrop').hidden, false, 'Escape starts the modal exit animation.');
  function finishRemovalTransition() {
    const pending = [...timers.entries()].find(([, timer]) => timer.delay === 140);
    assert(pending, 'Closing the modal must schedule its fade and slide out.');
    timers.delete(pending[0]);
    pending[1].callback();
  }
  finishRemovalTransition();
  assert.equal(element('api-key-removal-backdrop').hidden, true, 'Escape cancels the removal and closes the modal.');
  assert.equal(element('save-button').hidden, false, 'Cancelling removal leaves the key change pending.');
  element('save-button').handlers.click();
  assert.equal(element('api-key-removal-backdrop').hidden, false);
  element('api-key-removal-backdrop').handlers.pointerdown({ target: element('api-key-removal-backdrop') });
  finishRemovalTransition();
  assert.equal(element('api-key-removal-backdrop').hidden, true, 'Clicking the backdrop cancels saving.');
  element('save-button').handlers.click();
  element('api-key-removal-save').handlers.click();
  await flush();
  finishRemovalTransition();
  assert.equal(connectionRequests.at(-1).enabled, false, 'Confirming removal disables API features.');
  assert.equal(connectionRequests.at(-1).apiKey, '');
  assert.equal(connectionRequests.at(-1).keepExistingKey, false);
  assert.equal(element('save-button').hidden, true, 'A successful removal clears Advanced dirty state.');

  const requestsAfterRemoval = connectionRequests.length;
  await input('advanced-api-port', '31010');
  assert.equal(element('advanced-api-port-field').dataset.filled, 'true');
  element('save-button').handlers.click();
  assert.equal(element('api-key-removal-backdrop').hidden, false, 'Saving connection edits with no saved key still requires confirmation.');
  assert.equal(connectionRequests.length, requestsAfterRemoval, 'Unconfirmed empty-key saves must not reach the backend.');
  context.document.handlers.keydown({ key: 'Escape', preventDefault() {} });
  finishRemovalTransition();
  await input('advanced-api-port', '');

  await input('font-family', 'Arial');
  element('save-button').handlers.click();
  assert.equal(element('api-key-removal-backdrop').hidden, false, 'An Advanced-page settings save with an empty key requires confirmation.');
  assert.equal(connectionRequests.length, requestsAfterRemoval, 'Theme-only changes do not configure the API connection.');
  element('api-key-removal-close').handlers.click();
  finishRemovalTransition();
  await input('font-family', 'Libron');

  element('preferences-nav').handlers.click();
  assert.equal(typeof rebuildListener, 'function', 'The global rebuild shortcut must be connected to the UI save action.');
  assert.equal(element('save-button').hidden, true);
  assert.equal(element('log-button').hidden, false);
  assert.equal(element('log-button').dataset.severity, 'info');
  const logsBeforePreferenceEdits = element('log-entries').children.length;
  assert(logsBeforePreferenceEdits > 0);

  const pageContent = element('page-content');
  assert.equal(pageContent.classList.contains('has-overflow'), false, 'A page without overflow starts without a scrollbar.');
  pageContent.handlers.scroll();
  assert.equal(pageContent.classList.contains('is-scrolling'), false, 'A page without overflow must not show a scrollbar.');
  pageContent.scrollHeight = 900;
  context.window.handlers.resize();
  assert.equal(pageContent.classList.contains('has-overflow'), true, 'The page must expose its scrollbar when content overflows.');
  pageContent.handlers.scroll();
  assert.equal(pageContent.classList.contains('is-scrolling'), true, 'The scrollbar must appear during scrolling.');
  const firstScrollbarTimerId = [...timers.keys()][0];
  pageContent.handlers.scroll();
  assert.equal(timers.has(firstScrollbarTimerId), false, 'A new scroll event must restart the idle delay.');
  assert.equal(timers.size, 1);
  const [scrollbarTimerId, scrollbarTimer] = [...timers.entries()][0];
  assert.equal(scrollbarTimer.delay, 900);
  timers.delete(scrollbarTimerId);
  scrollbarTimer.callback();
  assert.equal(pageContent.classList.contains('is-scrolling'), false, 'The scrollbar must hide after scrolling stops.');
  pageContent.handlers.scroll();
  assert.equal(pageContent.classList.contains('is-scrolling'), true);
  pageContent.scrollHeight = 500;
  context.window.handlers.resize();
  assert.equal(pageContent.classList.contains('has-overflow'), false, 'The page must hide its scrollbar when content no longer overflows.');
  assert.equal(pageContent.classList.contains('is-scrolling'), false, 'The scrollbar must disappear if overflow ends during a scroll.');
  assert.equal(timers.size, 0, 'An obsolete idle timer must be cleared when overflow ends.');

  await input('font-family', 'Arial');
  assert.match(status(), /Unsaved changes/);
  assert.equal(element('save-button').disabled, false);
  assert.equal(element('save-button').hidden, false);
  await input('font-family', 'Libron');
  assert.equal(status(), 'No unsaved changes.');
  assert.equal(element('save-button').hidden, true);
  await input('accent-value', '#123456');
  assert.equal(element('save-button').hidden, false);
  await input('accent-value', '#DE50DE');
  assert.equal(element('save-button').hidden, true);
  assert.equal(element('log-entries').children.length, logsBeforePreferenceEdits, 'Editing should not fill the log.');

  await input('font-family', 'Arial');
  await changeToggle('reduced-motion', false);
  const pendingSave = element('save-button').handlers.click();
  assert.equal(element('api-key-removal-backdrop').hidden, true, 'Ordinary Preferences saves must not ask about an empty API key.');
  assert.equal(element('save-button').hidden, false, 'Pending changes remain visible while saving.');
  assert.equal(element('save-button').disabled, true, 'Repeated saves must be blocked.');
  await pendingSave;
  assert.equal(savedSettings.at(-1).reducedMotion, false, 'Accessibility settings must be included in the save request.');
  assert.equal(savedSettings.at(-1).tabHoverEffect, false);
  assert.equal(savedSettings.at(-1).linkHoverEffect, false);
  assert.equal(savedSettings.at(-1).objectMentionHoverEffect, false);
  assert.match(status(), /^Saved\./);
  assert.equal(element('save-button').hidden, true);
  assert.equal(element('log-button').dataset.severity, 'info', 'Successful saves with cached calendar colors are informational.');
  await input('font-family', 'Libron');
  saveError = '<script>Rebuild failed.</script>';
  await element('save-button').handlers.click();
  assert.equal(status(), saveError);
  assert.equal(element('save-button').hidden, false);
  assert.equal(element('log-button').dataset.severity, 'error');
  assert(element('log-entries').children.at(-1).textContent.endsWith(saveError), 'Log messages must remain plain text.');
  saveError = null;
  await element('save-button').handlers.click();
  assert.equal(element('log-button').dataset.severity, 'error', 'Retained errors take precedence over informational save messages.');
  assert.equal(element('save-button').hidden, true);

  let altRPrevented = false;
  const saveCallsBeforeAltR = calls.filter(name => name === 'save_settings').length;
  await context.document.handlers.keydown({
    key: 'r', code: 'KeyR', altKey: true, ctrlKey: false, metaKey: false, shiftKey: false, repeat: false,
    preventDefault() { altRPrevented = true; },
  });
  assert(altRPrevented, 'Alt+R must stay out of the focused input.');
  assert.equal(calls.filter(name => name === 'save_settings').length, saveCallsBeforeAltR + 1, 'The in-window shortcut must use the existing save/rebuild action.');
  const saveCallsBeforeGlobalEvent = calls.filter(name => name === 'save_settings').length;
  await Promise.all([rebuildListener(), rebuildListener()]);
  assert.equal(calls.filter(name => name === 'save_settings').length, saveCallsBeforeGlobalEvent + 1, 'A background hotkey event must start only one rebuild.');

  element('log-button').handlers.click();
  assert.equal(element('log-popup').hidden, false);
  assert.equal(element('log-button').attributes['aria-expanded'], 'true');
  assert(element('copy-logs').focused);
  await element('copy-logs').handlers.click();
  assert(copiedLogs.includes('Rebuild failed.'));
  assert.equal(copiedLogs.split('\n').length, element('log-entries').children.length);
  context.document.handlers.pointerdown({ target: element('log-control') });
  assert.equal(element('log-popup').hidden, false);
  context.document.handlers.pointerdown({ target: element('font-family') });
  assert.equal(element('log-popup').hidden, true);
  element('log-button').handlers.click();
  context.document.handlers.keydown({ key: 'Escape', preventDefault() {} });
  assert.equal(element('log-popup').hidden, true);
  assert(element('log-button').focused);
  element('log-button').handlers.click();
  context.document.handlers.focusin({ target: element('font-family') });
  assert.equal(element('log-popup').hidden, true);
  copyError = true;
  await element('copy-logs').handlers.click();
  assert.match(element('copy-status').textContent, /Could not copy/);
  assert.match(element('log-entries').children.at(-1).textContent, /Clipboard unavailable/);

  for (const [id, command] of [
    ['window-minimize', 'minimize_window'], ['window-close', 'hide_window'],
    ['open-repository', 'open_project_repository'],
    ['open-author', 'open_author_profile'],
  ]) {
    await element(id).handlers.click();
    assert.equal(calls.at(-1), command);
  }

  let prevented = false;
  const quitKey = {
    key: 'q', code: 'KeyQ', ctrlKey: true, altKey: false, shiftKey: false, metaKey: false, repeat: false,
    preventDefault() { prevented = true; },
  };
  context.document.handlers.keydown(quitKey);
  assert.equal(calls.at(-1), 'quit_app');
  assert(prevented);
  const beforeIgnoredKeys = calls.length;
  context.document.handlers.keydown({ ...quitKey, ctrlKey: false });
  context.document.handlers.keydown({ ...quitKey, repeat: true });
  context.document.handlers.keydown({ ...quitKey, altKey: true });
  assert.equal(calls.length, beforeIgnoredKeys);
  element('title-drag-region').handlers.pointerdown({ button: 0, target: { closest: () => null } });
  assert.equal(calls.at(-1), 'start_window_drag');
  const count = calls.length;
  element('title-drag-region').handlers.pointerdown({ button: 0, target: { closest: () => ({}) } });
  assert.equal(calls.length, count, 'Window control clicks must not start a drag.');
  const welcome = element('onboarding-welcome');
  const onboarding = element('onboarding');
  const api = element('onboarding-api');
  const shell = element('settings-shell');
  context.document.activeElement = element('font-family');
  const beforeOnboarding = calls.length;
  const unsavedFont = element('font-family').value;
  const debugKey = {
    key: 'z', code: 'KeyZ', ctrlKey: true, altKey: true, shiftKey: false, metaKey: false, repeat: false,
    preventDefault() {},
  };
  context.document.handlers.keydown({ ...debugKey, repeat: true });
  assert.equal(onboarding.hidden, true, 'Key repeat must not open onboarding.');
  context.document.handlers.keydown({ ...debugKey, metaKey: true });
  assert.equal(onboarding.hidden, true, 'Modified shortcuts must not open onboarding.');
  context.document.handlers.keydown(debugKey);
  assert.equal(onboarding.hidden, false);
  assert.equal(welcome.hidden, false);
  assert.equal(shell.inert, true);
  assert(element('root').classList.contains('onboarding-active'));
  welcome.handlers.click();
  assert.equal(api.hidden, true, 'Welcome fades out before the choice screen opens.');
  function finishOnboardingTransition() {
    const pending = [...timers.entries()].filter(([, timer]) => timer.delay === 320).at(-1);
    assert(pending, 'Onboarding must schedule a transition.');
    timers.delete(pending[0]);
    pending[1].callback();
  }
  finishOnboardingTransition();
  assert.equal(welcome.hidden, true);
  assert.equal(api.hidden, false);
  assert(element('onboarding-api-title').focused);
  element('onboarding-api-option').handlers.click();
  assert.equal(onboarding.dataset.apiChoice, 'api');
  element('onboarding-no-api-option').handlers.click();
  assert.equal(onboarding.dataset.apiChoice, 'api', 'Ignore repeated selections during transition.');
  finishOnboardingTransition();
  assert.equal(element('onboarding-connection').hidden, false, 'API choice must open connection setup.');
  assert.equal(element('connection-key').type, 'password', 'Onboarding keys start concealed.');
  assert.equal(element('connection-port').type, 'text', 'Onboarding ports start readable.');
  element('connection-key-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('connection-key').type, 'text');
  element('connection-port-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('connection-port').type, 'password');
  element('connection-port-visibility').handlers.click({ preventDefault() {} });
  assert.equal(element('connection-port').type, 'text');
  const beforeInvalidConnection = connectionRequests.length;
  const submit = () => element('connection-form').handlers.submit({ preventDefault() {} });
  submit();
  assert.match(element('connection-error').textContent, /Enter your Anytype/);
  element('connection-key').value = 'test-key';
  await element('connection-key').handlers.input();
  element('connection-port').value = '65536';
  await element('connection-port').handlers.input();
  assert.equal(element('connection-key-field').dataset.filled, 'true');
  assert.equal(element('connection-port-field').dataset.filled, 'true');
  submit();
  assert.match(element('connection-error').textContent, /Port must/);
  assert.equal(connectionRequests.length, beforeInvalidConnection, 'Invalid onboarding values must not start a connection request.');
  element('connection-port').value = '';
  await element('connection-port').handlers.input();
  connectionError = 'Could not connect to Anytype.';
  submit();
  await flush();
  assert.match(element('connection-error').textContent, /Could not connect/);
  assert.equal(onboarding.hidden, false, 'Failure must keep the setup page open.');
  assert.equal(connectionRequests.at(-1).port, null, 'Blank port uses the backend default.');
  connectionError = null;
  element('connection-port').value = '31010';
  await element('connection-port').handlers.input();
  submit();
  assert.equal(element('connection-confirm').disabled, true);
  assert.equal(element('connection-key-visibility').disabled, true, 'Connection visibility controls disable while saving.');
  await flush();
  assert.equal(connectionRequests.at(-1).port, 31010);
  assert.equal(connectionRequests.at(-1).apiKey, 'test-key');
  assert.equal(element('connection-key').value, '', 'Clear the key after saving.');
  assert.equal(element('connection-key').type, 'password', 'Clearing a submitted key resets it to concealed.');
  assert.equal(element('connection-key-visibility').attributes['aria-pressed'], 'false');
  assert.equal(element('connection-key-field').dataset.filled, 'false');
  finishOnboardingTransition();
  assert.equal(onboarding.hidden, true);
  assert.equal(shell.inert, false);
  assert(shell.classList.contains('onboarding-entering'));
  context.document.handlers.keydown(debugKey);
  welcome.handlers.click();
  finishOnboardingTransition();
  element('onboarding-no-api-option').handlers.click();
  await flush();
  assert.equal(connectionRequests.at(-1).enabled, false);
  assert.equal(connectionRequests.at(-1).apiKey, '');
  assert.equal(onboarding.dataset.apiChoice, 'no-api');
  finishOnboardingTransition();
  assert.equal(onboarding.hidden, true, 'No API mode skips connection setup.');
  context.document.handlers.keydown(debugKey);
  welcome.handlers.click();
  const pendingTimerIds = [...timers.keys()];
  context.document.handlers.keydown({ key: 'Escape', preventDefault() {} });
  assert(pendingTimerIds.some(id => !timers.has(id)), 'Exit cancels the pending transition.');
  assert.equal(onboarding.hidden, true);
  assert.equal(shell.inert, false);
  assert.equal(element('font-family').value, unsavedFont);
  element('advanced-nav').handlers.click();
  await input('advanced-api-key', 'replacement-test-key');
  await input('advanced-api-port', '31011');
  await element('save-button').handlers.click();
  await flush();
  assert.equal(connectionRequests.at(-1).apiKey, 'replacement-test-key');
  assert.equal(connectionRequests.at(-1).port, 31011);

  // Discard uses the latest saved connection, including Advanced Settings saves.
  assert.equal(element('discard-button').hidden, true);
  const savedFont = element('font-family').value;
  const savedAccent = element('accent-value').value;
  await input('font-family', 'invalid;font');
  await input('accent-value', '#ffffff');
  await input('advanced-api-port', '65536');
  await element('save-button').handlers.click();
  assert.equal(element('font-family').classList.contains('invalid'), true);
  assert.equal(element('discard-button').hidden, false, 'Undo appears with Save even for invalid edits.');
  element('discard-button').handlers.click();
  assert.equal(element('discard-confirmation-backdrop').hidden, false);
  element('discard-confirmation-close').handlers.click();
  finishRemovalTransition();
  assert.equal(element('font-family').value, 'invalid;font', 'Closing the undo warning preserves edits.');
  const callsBeforeDiscard = calls.length;
  element('discard-button').handlers.click();
  element('discard-confirmation-undo').handlers.click();
  finishRemovalTransition();
  assert.equal(element('font-family').value, savedFont);
  assert.equal(element('accent-value').value, savedAccent);
  assert.equal(element('advanced-api-port').value, '31011', 'Undo keeps the connection saved through Advanced Settings.');
  assert.equal(element('advanced-api-key').value, '••••••••');
  assert.equal(element('advanced-api-key').type, 'password');
  assert.equal(element('font-family').classList.contains('invalid'), false);
  assert.equal(element('font-error').textContent, '');
  assert.equal(element('accent-error').textContent, '');
  assert.equal(element('advanced-api-error').textContent, '');
  assert.equal(element('save-button').hidden, true);
  assert.equal(element('discard-button').hidden, true);
  assert.equal(status(), 'No unsaved changes.');
  assert.equal(calls.length, callsBeforeDiscard, 'Undo never writes to the backend.');

  element('advanced-nav').handlers.click();
  for (const discardAction of ['api-key-removal-close', 'api-key-removal-undo']) {
    await input('font-family', 'Arial');
    await input('advanced-api-key', '');
    await element('save-button').handlers.click();
    assert.equal(element('api-key-removal-backdrop').hidden, false);
    element(discardAction).handlers.click();
    finishRemovalTransition();
    assert.equal(element('font-family').value, savedFont, 'API warning discard restores every edited setting.');
    assert.equal(element('advanced-api-key').value, '••••••••');
    assert.equal(element('save-button').hidden, true);
    assert.equal(element('discard-confirmation-backdrop').hidden, true, 'API warning Undo never opens another warning.');
  }

  await input('font-family', 'Arial');
  await input('advanced-api-port', '31012');
  saveError = 'General settings save failed.';
  await element('save-button').handlers.click();
  assert.match(status(), /Connection saved\. Other settings could not be saved/);
  assert.equal(connectionRequests.at(-1).port, 31012);
  saveError = null;
  element('discard-button').handlers.click();
  element('discard-confirmation-undo').handlers.click();
  finishRemovalTransition();
  assert.equal(element('font-family').value, savedFont, 'Undo restores general settings after a failed save.');
  assert.equal(element('advanced-api-port').value, '31012', 'Undo preserves the part that saved successfully.');
  assert.equal(element('save-button').hidden, true);

  await input('font-family', 'Arial');
  await element('save-button').handlers.click();
  await input('font-family', 'Consolas');
  element('discard-button').handlers.click();
  element('discard-confirmation-undo').handlers.click();
  finishRemovalTransition();
  assert.equal(element('font-family').value, 'Arial', 'A successful save advances the undo baseline.');

  element('preferences-nav').handlers.click();
  const bannerButton = element('choose-banner-image');
  const bannerInput = element('banner-image-input');
  const bannerDataUrl = 'data:image/png;base64,iVBORw0KGgo=';
  assert.equal(element('space-preview-image').src, 'data:image/png;base64,default-preview', 'The preview starts with the default banner.');
  bannerButton.handlers.click();
  assert.equal(bannerInput.clickCount, 1, 'Choose image opens the native file input.');
  bannerInput.files = [{ type: 'image/png', size: 8, dataUrl: bannerDataUrl }];
  await bannerInput.handlers.change();
  await flush();
  assert.equal(element('space-preview-image').src, bannerDataUrl, 'Selecting an image updates the rendered sidebar preview.');
  assert.equal(element('discard-button').hidden, false, 'A selected banner participates in Undo tracking.');
  assert.equal(element('save-button').disabled, false);
  await element('save-button').handlers.click();
  await flush();
  assert.equal(savedSettings.at(-1).bannerImage, bannerDataUrl, 'Save persists the selected banner data.');

  element('reset-banner-image').handlers.click();
  assert.equal(element('space-preview-image').src, 'data:image/png;base64,default-preview', 'Use default switches the preview back.');
  element('discard-button').handlers.click();
  element('discard-confirmation-undo').handlers.click();
  finishRemovalTransition();
  assert.equal(element('space-preview-image').src, bannerDataUrl, 'Undo restores the saved banner preview.');

  element('reset-banner-image').handlers.click();
  await element('save-button').handlers.click();
  await flush();
  assert.equal(savedSettings.at(-1).bannerImage, '', 'Save persists the default banner selection.');
  assert.equal(element('reset-banner-image').hidden, true);

  bannerInput.files = [{ type: 'image/gif', size: 8, dataUrl: 'data:image/gif;base64,R0lGODlh' }];
  await bannerInput.handlers.change();
  assert.match(element('banner-error').textContent, /PNG, JPEG, or WebP/);
  assert.equal(element('save-button').hidden, true, 'An invalid image does not create a settings edit.');

  delayFileRead = true;
  bannerInput.files = [{ type: 'image/png', size: 8, dataUrl: bannerDataUrl }];
  await bannerInput.handlers.change();
  const staleReader = fileReaders.at(-1);
  assert.equal(element('save-button').disabled, true, 'Save waits while an image is loading.');
  context.window.BlobUI.discardChanges();
  await staleReader.onload();
  assert.equal(element('space-preview-image').src, 'data:image/png;base64,default-preview', 'Undo invalidates a pending image read.');
  delayFileRead = false;

  updaterCheckState = {
    currentVersion: '1.2.0', latestVersion: '1.3.0', updateAvailable: true,
    lastCheckedAt: null, status: 'available', message: 'Version 1.3.0 is available.',
  };
  assert.equal(element('update-now-button').hidden, true, 'The prior completed update clears its action before the deferred check.');
  element('check-updates-button').handlers.click();
  for (let turn = 0; turn < 10 && element('update-now-button').hidden; turn += 1) await flush();
  assert.equal(element('update-now-button').hidden, false, 'The manual check reveals the available update before the apply attempt.');
  deferUpdateApply = true;
  const applyCallsBeforeDeferredInstall = calls.filter(name => name === 'apply_update').length;
  const inFlightUpdate = element('update-now-button').handlers.click();
  assert.equal(calls.filter(name => name === 'apply_update').length, applyCallsBeforeDeferredInstall + 1, 'The deferred test reaches the native installer.');
  assert.equal(typeof pendingUpdateApplyReject, 'function', 'The test holds the native installer promise open.');
  assert.equal(element('font-family').disabled, true, 'Installing an update locks settings inputs while the native updater runs.');
  assert.equal(element('install-directory').disabled, true, 'Installing an update locks updater preferences.');
  assert.equal(element('save-button').disabled, true, 'Installing an update blocks Save.');
  assert.equal(element('discard-button').disabled, true, 'Installing an update blocks Undo.');
  const savesDuringUpdate = calls.filter(name => name === 'save_settings').length;
  await element('save-button').handlers.click();
  assert.equal(calls.filter(name => name === 'save_settings').length, savesDuringUpdate, 'A direct Save action is ignored during installation.');
  let forceRebuildPrevented = false;
  await context.document.handlers.keydown({
    key: 'r', code: 'KeyR', altKey: true, ctrlKey: false, metaKey: false, shiftKey: false, repeat: false,
    preventDefault() { forceRebuildPrevented = true; },
  });
  assert(forceRebuildPrevented, 'Alt+R remains handled during installation.');
  assert.equal(calls.filter(name => name === 'save_settings').length, savesDuringUpdate, 'Force rebuild is ignored during installation.');
  element('discard-button').handlers.click();
  assert.equal(element('discard-confirmation-backdrop').hidden, true, 'Undo confirmation cannot open during installation.');
  await context.document.handlers.keydown({
    key: 'z', code: 'KeyZ', altKey: true, ctrlKey: true, metaKey: false, shiftKey: false, repeat: false,
    preventDefault() {},
  });
  assert.equal(element('onboarding').hidden, true, 'The setup guide cannot open during installation.');
  pendingUpdateApplyReject(new Error('Fixture update failed.'));
  await inFlightUpdate;
  deferUpdateApply = false;
  assert.equal(element('font-family').disabled, false, 'Settings inputs are restored after the updater fails.');
  await input('font-family', 'Consolas');
  assert.equal(element('save-button').disabled, false, 'Save is restored after the updater fails.');
  assert.equal(element('discard-button').disabled, false, 'Undo is restored after the updater fails.');
  context.window.BlobUI.discardChanges();
  assert.equal(element('save-button').hidden, true, 'The failure fixture leaves no pending settings edits.');

  console.log('UI save, Alt+R rebuild, log severity, popup, copy, and window action tests passed.');
})().catch(error => { console.error(error); process.exitCode = 1; });
