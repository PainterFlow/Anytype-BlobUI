# Anytype UI reference

The settings layout follows the official [Anytype desktop client](https://github.com/anyproto/anytype-ts) at revision `4b265d9564d70a45b030cfb64669b4508eb915ef`. The components remain local HTML and CSS so the Tauri settings app does not need Anytype's React runtime.

The following upstream files supplied the measurements and control behavior.

- [Settings layout](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/page/main/settings.scss) sets the 640px content limit, 28px page title, 16px settings rows, 14px/16px row padding, and 12px group corners.
- [Settings sidebar](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/component/sidebar/page/settings.scss) sets compact navigation rows, 20px icons, and 12px group labels.
- [Buttons](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/button.scss) use a 40px height, 20px corner radius, 14px type, and neutral disabled state.
- [Inputs](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/input.scss) supply the filled backgrounds and subtle hover/focus highlight. The text fields use the user's Anytype screenshot measurements of 48px height and 18px corners.
- [Switches](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/switch.scss) use a 40×24px track and a 20px thumb in their large variant.

Colors and font sizes also match the installed Anytype stylesheet. BlobUI's saved interface and code fonts still apply. The custom color picker and explicit Save action remain BlobUI controls. Decorative shadows, glows, theme cards, and font samples remain absent.

Scrollbars follow the user's Anytype screenshot, with a 12px width, rounded thumb ends, no arrow buttons, and a transparent track. Their gray thumbs match the current theme, appear while scrolling, and fade after scrolling stops. Scroll areas without overflow show no scrollbar. The settings page reserves its scrollbar gutter even for short panels so switching Preferences or Advanced tabs cannot change the content or tab-strip width. Browser layout tests must keep scrollbars enabled.

Advanced Settings uses API key and App settings tabs. App settings contains the folder, update, backup, and uninstall controls. The tabs support arrow keys, Home, and End, and preserve pending edits when switching. The Anytype Folder field opens the native folder picker and has no surrounding group border. The update check button keeps its width and height across all labels. The update switches reuse Accessibility's grouped settings rows. The backup and import section follows Updates and precedes Uninstall; it uses the same 40px rounded neutral buttons and opens native folder pickers for a dated full-folder copy or restore. The update action sits below Save and Undo and above Logs, with a fixed Blob Pink icon independent of the selected accent.

## Reusable settings controls

The UI uses plain JavaScript DOM components in `ui/components.js`. Screen definitions in `settings-pages.js`, `onboarding-view.js`, `dialog-views.js`, and `shell-view.js` provide their properties. Named `<template data-ui-mount="...">` slots in `index.html` hold custom content. `screens.js` mounts nested slots once, before Docs, formula tools, or application bindings run. Calling `BlobUI.Screens.mount()` again preserves existing controls, focus, and drafts.

Use `BlobUI.Components.ToggleSetting({ id, label, inputAttributes, onChange })` for a native switch row and `TextField({ inputAttributes, label, onInput })` for a labeled input. A visibility field adds `visibility.buttonAttributes`; its controller is attached later through `bindVisibility(options)` so secret retrieval stays in application code. Both factories expose `input`. Text fields also expose their visibility `field` and `button` when present.

Factories return `{ element, update(props), destroy() }`. `update()` changes supported values, labels, callbacks, and disabled states without replacing an input or emitting native events. Updating an unchanged input value preserves its selection. `destroy()` is idempotent, removes owned listeners and controllers, and cancels pending secret reveals. It leaves DOM removal to the caller. Structural slots and attributes are supplied at creation; build a new component when changing their structure. `Components.destroyTree(container)` tears down components before replacing dynamic content.

`Section`, `Group`, `SettingsRow`, and `Heading` provide the shared settings structure. `Button`, `IconButton`, and `NavigationItem` create actions without changing their existing classes. `OnboardingChoice` renders both API choices from one feature-list implementation. Ordinary strings use `textContent`; custom content is passed as DOM nodes. Keep Markdown in the existing sanitized renderer.

`Components.Tabs`, `Disclosure`, `Picker`, and `Search` attach the existing behavior controllers to supplied nodes and expose their methods plus the component lifecycle. `DialogShell` builds a dialog and backdrop; `Components.Modal(options)` attaches its modal controller. Keep the returned controller for open/close operations. `Components.Visibility(options)` attaches the matching mounted text field. Components do not invoke Tauri, save settings, or own a second draft store.

`BlobUI.Scrollbar.attach(container, contentSelector)` is shared by Settings, Docs, and the calculator. It returns `update()` and `destroy()`, observes replaced content and hidden panels, removes obsolete resize targets, and cancels timers and observers on teardown.

For example, add a switch definition and a matching mount slot, then bind its input through the existing draft flow.

```javascript
"example-enabled": () => C.ToggleSetting({
  id: "example-enabled",
  label: "Enable example",
  inputAttributes: { "aria-describedby": "example-help" }
})
```

```html
<template data-ui-mount="example-enabled"></template>
<p id="example-help" class="accessibility-note">Describe the setting.</p>
```

Load behavior controllers first, followed by `scrollbar.js`, `components.js`, the screen definitions, and `screens.js`. Load consumers such as Docs and `app.js` after mounting. Browser fixtures must use this same dependency order. The existing control IDs, CSS classes, native events, and original/refreshed layout state remain shared.

Load `visibility-field.js`, `modal.js`, `edit-session.js`, `tabs.js`, `pick-control.js`, and `expandable-search.js` before `app.js`.

`BlobUI.Tabs.attach({ container, buttons, panels, onSelect, scrollContainer })` owns tab selection, panel visibility, the animated underline, and keyboard navigation in both Advanced and Preferences. Use `select(index)`, `refresh()` after revealing a hidden page, `setEnabled(value)` to show all panels when tabs are disabled, and `destroy()` for cleanup. It measures the underline inside its own tab strip and scrolls only that strip when revealing a tab, including the last tab. Changing the selected tab resets its content scroll container to the top immediately. The controller defaults to the nearest `.page-content`; pass `scrollContainer` for another scroll layout. Clicking the selected tab, resizing, and font or underline refreshes preserve the current scroll position. Hidden-panel focus moves to the newly selected tab without scrolling. The settings scroll area disables browser scroll anchoring, and sidebar section links use this same selection behavior without a second scroll command. Resize, fonts, and reduced motion use the same behavior in both places.

`disclosure.js` exposes `BlobUI.Disclosure.attach(container, { onToggle })` for expandable panels. Use a native button with `data-disclosure-toggle`, `aria-controls`, and an initial `aria-expanded` value. Give its panel the matching `id` and `data-disclosure-panel`. Add `data-disclosure-content` to an inner wrapper when CSS animates panel height.

The controller updates `aria-expanded`, `data-disclosure-state`, `inert`, and `hidden`. If a panel closes while focus is inside it, the controller moves focus to its button. It skips transition delays when the system or app requests reduced motion. The returned controller exposes `setExpanded(button, value)` and `destroy()`. Call `destroy()` before replacing the container's contents. Load `disclosure.js` before the module that calls it. The `onToggle` callback receives `{ toggle, panel, expanded, previousExpanded }`.

`VisibilityField.bind({ input, button, field, showLabel, hideLabel, defaultVisible })` adds an eye toggle to a text field. Its returned controller exposes `reset()`, `sync()`, and `invalidate()`. Call `reset()` after replacing a field value or leaving a screen to restore its default visibility and cancel a pending reveal. Optional `shouldReveal`, `reveal`, `onHide`, `onRevealError`, and `onBusyChange` hooks keep saved-secret retrieval in the application. Resetting or editing the field invalidates a delayed reveal result.

`VisibilityIcon.create()` returns the same 20px eye SVG used by API text fields. Place it in a button with `aria-pressed="true"` when its source is visible and `false` when hidden. The shared `.visibility-icon` styles handle its stroke and slash. Set the button's label and title to describe its next action. Use `.neutral-icon-button` for a standalone 32px control.

`BlobUI.createModalController({ backdrop, dialog, initialFocus, fallbackFocus, inertTargets })` provides `open()`, `close()`, and `isOpen()`. It handles the shared animation, reduced motion, background interaction, focus restoration, Tab navigation, Escape, and backdrop dismissal. Bind action buttons separately so each dialog can choose whether an action dismisses, saves, or discards edits.

`BlobUI.createEditSession({ read, restore, equals })` tracks a saved baseline. Use `isDirty()` to show Save and Undo together, `discard()` to restore pending edits, and `commit()` after a successful save. `commit(patch)` advances only the successfully saved part when a save has multiple steps. Baseline snapshots contain settings and saved-key presence, without storing a decrypted API key.

The API-key warning keeps edits when Escape or its backdrop dismisses it. Its X and Undo restore the saved baseline directly. It has no labeled Close button. The sidebar Undo opens a separate confirmation whose X dismisses it and whose red Undo restores that baseline.

## Settings Future Proof UI Refresh experiment

Advanced > Experiments always contains Settings Future Proof UI Refresh. Its switch previews the layout immediately and participates in Save and Undo. Existing installations default to the original layout. The experiment changes only the Settings interface. Below the Experiments description, an amber warning icon and note explain that experiments may break things and are used at the user's own risk.

The enabled layout uses full-width category tabs in Preferences and icon section links under Preferences and Advanced. These links reuse the Docs hierarchy, animation, and disclosure controller without a chevron. Accessibility has no child navigation or category dividers. Preferences, Advanced Settings, Accessibility, and Docs have no search fields. Calendar explains Alt+R to rebuild followed by Ctrl+R in Anytype to apply color changes.

## Kanban formulas

The Kanban View Formulas page follows Anytype's UI philosophy and the measurements above. Its right-aligned toolbar uses 32px circular hover targets and exact 20px search and filter SVG paths. The search pill expands left to 200px while the reload and filter controls stay anchored. A single divider separates the toolbar from the view list. List rows use rounded hover backgrounds without dividers, 16px names, 14px metadata, and the saved interface font with a system sans-serif fallback. The original layout has a sidebar entry. The refreshed layout has a button in Preferences > Kanban instead. Search belongs only to the real Kanban view browser.

Each view is a flat row. Selecting it opens the shared modal with an X close control, a read-only rendered calculation, and a five-column keypad. The eye toggle opens the keyboard-editable LaTeX source. Buttons keep the same appearance in both modes, but formula mutations only run in source mode. Rendered mode has no editing caret or pointer. Calculator buttons insert LaTeX at the source selection, including when the surrounding source is incomplete or invalid. C clears the entire source and both fraction terms. There is no trash control. Insert property opens the shared property picker, followed by a required aggregation picker. A property reference includes both quoted arguments, such as `\property{"Points"}{"sum"}`. Aggregations include Total, Average, Minimum, Maximum, and Median. Property terms keep stable keys while showing readable names. Card count inserts `\count`. Square roots use `\sqrt{body}` and nth roots use `\sqrt[degree]{body}`. The calculator Save action closes the dialog while keeping edits in the shared draft. Sidebar Save publishes valid edits, and Undo restores the saved calculation. The compact dialog centers without a scroll container. Insert property and Card count form a left-aligned group with a fixed 6px gap. Only the 32px eye toggle aligns right and reuses the API field eye icon. Both modes reserve the same display height. Errors sit beside C.

Display formulas and Board layout share the reusable picker inside the same Kanban settings list. Their values remain independent. Balanced places the result beside the category title and wraps it onto a second line when needed. Spacious places it below the title. With Collapsed Categories enabled, results fade in while a category is hovered or focused and fade out when it is left. Anytype's fixed header row becomes auto-height when a result wraps or Spacious is selected, and existing header controls remain visible on hover.

A property reference can include an optional third argument with a calculation for each card. For example, `\property{"Price"}{"sum"}{\property{"Price"} * \property{"Quantity"}}` multiplies each card's Price by its Quantity, then totals those results. The second argument remains the required aggregation. Inside the third argument, a property without an aggregation reads that card's value. An explicitly aggregated nested property also uses only the current card. Insert property inserts a bare reference immediately when the source selection is inside a third argument, including incomplete drafts. At the root it still asks for an aggregation. Nested calculations retain stable property keys through rendering, draft edits, and saving.

The native backend reads the saved encrypted connection, discovers real view metadata, evaluates complete paginated membership, and publishes a snapshot through the CSS compiler. Unsupported views are omitted from the browser and each exclusion is recorded at info level in Logs. Their saved calculations remain intact. Save and Undo include calculation and display edits. The toolbar has a single neutral reload icon for available views, a space filter, and a magnifier that expands into a borderless search pill. The filter icon uses the accent only while a space filter is active. Its menu keeps a neutral selected row and stays open after selection until dismissed outside or with Escape. Saving requires Ctrl+R in Anytype to apply the snapshot. There is no background refresh or JavaScript injection. Configuration version 2 lives in `.blobui-kanban-formulas.json` in the workspace. Only the old prototype's display preference migrates; the prototype file stays untouched.


`BlobUI.PickControl.attach({ root, options, value, onChange, closeOnSelect })` handles the Board layout, Display formulas, and space-filter menus. The root contains an `aria-haspopup="listbox"` button and a `role="listbox"` menu. Optional options are `{ value, label, description }` objects. The optional description appears as a second line while the option keeps its name as its accessible label. It returns `setValue(value)`, `setOptions(options)`, `open()`, `close(restoreFocus)`, and `destroy()`. Selection updates the trigger value and fires a bubbling change event. `closeOnSelect` defaults to true; the space filter passes false. Arrow keys, Home, End, Escape, focus restoration, outside dismissal, and reduced motion use the same controller.

`BlobUI.ExpandableSearch.attach({ container, button, input })` expands the magnifier into a neutral pill. It preserves the native input event. Escape clears the query and collapses the field; leaving an empty field collapses it. The controller returns `setExpanded(expanded, focus)` and `destroy()`. Its input has no border, outline, or shadow in any state. Neutral icon controls share the circular `.neutral-icon-button` utility. Centered text actions use `.button-label-centered`. The formula page has a short entrance animation in the refreshed layout and respects reduced motion.

The calculator Math/Logic switch sits above its five-column keypad and defaults to Math on each open. It preserves the LaTeX selection. Logic keys insert comparisons, AND, OR, NOT, boolean literals, parentheses, and conditional delimiters. Both keypads keep Save available. Operator mutations remain restricted to source mode. Rendered logic uses mathematical symbols and boolean labels use True or False. Conditional expressions may return intermediate values. Numeric conditions use zero as false, while boolean arithmetic requires an explicit conditional conversion.

## Shared Markdown video player

App settings has General, Anytype Folder, Updates, Backup and import, and Uninstall sections with no dividers between sections in both layouts. General places Launch at startup and Autoplay videos in compact standalone rows without a shared box. Backup and import uses a rounded group with separate Export settings and Import settings rows. Each row has gray helper text below its label and a right-aligned action button. Operation status appears below the group. Use the shared ToggleSetting factory and existing Save/Undo flow. Missing saved preferences default to enabled.

All Markdown readers use Markdown.mount and the same VideoPlayer controller. Use one 64px circular play/pause button centered over the video. The bottom gradient contains the seek timeline, time, mute, volume, and a custom full-screen toggle. The player and its controls have no borders, outlines, or shadows; keyboard focus uses filled control states. Controls appear on pointer movement, keyboard focus, or touch and fade after two seconds without activity. They stay visible while focused. Ten-second jump controls and shortcuts are unavailable. Native video controls and the browser context menu remain disabled. Autoplay starts muted at 50% visibility, pauses when hidden, and stops at the end. Respect manual pauses and reduced motion.

The centered glyph is 40px with heavier pause strokes; its background stays 64px. Both sliders fill with the accent color up to the thumb center. Playback progress uses animation frames only while the player is visible and playing, without replacing icons every frame or overriding an active scrub.

Unlinked Markdown images in documentation and release notes open directly over a darker backdrop, without a panel, header, captions, dimensions, or visible controls. The image fits the window first; clicking it or pressing Enter or Space toggles actual size with scroll panning. Escape or clicking outside the image closes the viewer and restores focus without dismissing an underlying release-notes dialog. The background stays inert while open. Images inside links keep their normal link behavior. Images retain their original resolution; screenshots range from 441px to about 2K wide, and videos are already 1080p. Artificial upscaling cannot recover missing screenshot text.

Documentation groups Appearance, Kanban, Calendar Colors, and QoL Features under Features. API Guide is a separate guide. Experiment contains Settings UI as its child, with distinct experiment and layout icons. Features is a short overview that points to detailed guides. Reader playback instructions belong in Introduction, separate from Anytype's QoL settings.

Website links always open in the system's default browser. Documentation article selection and local heading anchors stay in the reader. Never navigate the Settings webview to a website or create an embedded browser window.

## Resizable sidebar

Variant 1 is the standard sidebar in Settings and Documentation in both layouts. Drag the divider or focus it and use Left and Right to resize in 8px steps. Home selects 160px and End selects the maximum of 320px, capped at 40% of the window with a 160px minimum. Escape cancels an active drag. Geometry persists automatically without marking settings dirty. Earlier Off and Pro selections now use this behavior, including restoring labels for saved icon rails.

Navigation labels use ellipsis and native full-name tooltips only when clipped. Resizing never brightens the divider or covers Logs and Help popovers. Tab geometry updates without underline animation during resizing, and horizontal tab scrollbars remain hidden while keyboard selection still reveals the selected tab.

## Calendar property colors and catalog browser

Calendar's Color by Property button appears in both Preferences layouts. It opens the shared catalog browser with all accessible object types. Type rows show the space and open a modal containing Default, calendar overrides, and Add override. The calendar chooser replaces the modal content and returns to the type rules after selection or cancellation. Each rule row uses an anchored Components.Picker menu and offers the type's single-select and checkbox properties. Multi-select is excluded. Modal edits belong to the shared Save/Undo draft.

`BlobUI.CatalogBrowser` in `ui/catalog-browser.js` provides the toolbar, rows, search, space filtering, loading/empty messages, and status presentation for Kanban, object types, and the calendar chooser. The toolbar is derived from the single mounted Kanban toolbar definition, with unique IDs for each instance. `attach({ prefix, toolbar, records, empty, onReload })` owns its search and picker controllers and exposes refresh, reset, filter, setLocked, and destroy. Records contain item, row, and view metadata. Destroy modal browsers before replacing their DOM. Keep formula-specific data, discovery, and editing outside this component.

The calendar modal uses DialogShell and the shared modal controller for background isolation, keyboard focus, Escape, dismissal, and reduced motion. Escape from a chooser returns to the type rules. The modal close control dismisses the type rules while preserving draft edits. Never use live Anytype automation for validation. Use dark workspace fixtures for dialogs and exact calendar CSS targets.

Calendar rule selectors keep their menus outside the scrolling list, within the same dialog. PickControl accepts an optional menu node for this placement. The calendar dialog has 16px padding, a fixed header, and a shared fading scrollbar only on its content. The calendar search toolbar stays fixed while its results scroll. Closing the calendar chooser plays the shared closing animation and reopens the type rules. Closing the type rules dismisses the dialog. There is no Back button. Focus stays keyboard accessible without outline rings.

Calculator Insert property and calendar rule selectors share Components.PropertyPickerShell, including the same rounded neutral trigger and menu. Calendar selectors show their current value without a chevron. Override removal uses the shared calculator DeleteIcon in a red icon button immediately after the calendar name.

Calendar discovery preserves its last successful catalog during reload, as Kanban does. Saved types are marked unavailable only after a successful discovery confirms their absence. Initial loading and failed discovery must never create unavailable rows. Shared catalog rows constrain both text columns, and warnings span the full row so long identifiers cannot squeeze names into a narrow strip.
