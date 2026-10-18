# Anytype UI reference

The settings layout follows the official [Anytype desktop client](https://github.com/anyproto/anytype-ts) at revision `4b265d9564d70a45b030cfb64669b4508eb915ef`. The components remain local HTML and CSS so the Tauri settings app does not need Anytype's React runtime.

The following upstream files supplied the measurements and control behavior.

- [Settings layout](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/page/main/settings.scss) sets the 640px content limit, 28px page title, 16px settings rows, 14px/16px row padding, and 12px group corners.
- [Settings sidebar](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/component/sidebar/page/settings.scss) sets compact navigation rows, 20px icons, and 12px group labels.
- [Buttons](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/button.scss) use a 40px height, 20px corner radius, 14px type, and neutral disabled state.
- [Inputs](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/input.scss) supply the filled backgrounds and subtle hover/focus highlight. The text fields use the user's Anytype screenshot measurements of 48px height and 18px corners.
- [Switches](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/switch.scss) use a 40×24px track and a 20px thumb in their large variant.

Colors and font sizes also match the installed Anytype stylesheet. BlobUI's saved interface and code fonts still apply. The custom color picker and explicit Save action remain BlobUI controls. Decorative shadows, glows, theme cards, and font samples remain absent.

Scrollbars follow the user's Anytype screenshot, with a 12px width, rounded thumb ends, no arrow buttons, and a transparent track. Their gray thumbs match the current theme, appear while scrolling, and fade after scrolling stops. Scroll areas without overflow show no scrollbar.

Advanced Settings uses API key and App settings tabs. App settings contains the folder and update controls. The tabs support arrow keys, Home, and End, and preserve pending edits when switching. The Anytype Folder field opens the native folder picker and has no surrounding group border. The update check button keeps its width and height across all labels. The update switches reuse Accessibility's grouped settings rows. The update action sits below Save and Undo and above Logs, with a fixed Blob Pink icon independent of the selected accent.

## Reusable settings controls

Load `visibility-field.js`, `modal.js`, and `edit-session.js` before `app.js`.

`VisibilityField.bind({ input, button, field, showLabel, hideLabel, defaultVisible })` adds an eye toggle to a text field. Its returned controller exposes `reset()`, `sync()`, and `invalidate()`. Call `reset()` after replacing a field value or leaving a screen to restore its default visibility and cancel a pending reveal. Optional `shouldReveal`, `reveal`, `onHide`, `onRevealError`, and `onBusyChange` hooks keep saved-secret retrieval in the application. Resetting or editing the field invalidates a delayed reveal result.

`BlobUI.createModalController({ backdrop, dialog, initialFocus, fallbackFocus, inertTargets })` provides `open()`, `close()`, and `isOpen()`. It handles the shared animation, reduced motion, background interaction, focus restoration, Tab navigation, Escape, and backdrop dismissal. Bind action buttons separately so each dialog can choose whether an action dismisses, saves, or discards edits.

`BlobUI.createEditSession({ read, restore, equals })` tracks a saved baseline. Use `isDirty()` to show Save and Undo together, `discard()` to restore pending edits, and `commit()` after a successful save. `commit(patch)` advances only the successfully saved part when a save has multiple steps. Baseline snapshots contain settings and saved-key presence, without storing a decrypted API key.

The API-key warning keeps edits when Escape or its backdrop dismisses it. Its X and Undo restore the saved baseline directly. It has no labeled Close button. The sidebar Undo opens a separate confirmation whose X dismisses it and whose red Undo restores that baseline.
