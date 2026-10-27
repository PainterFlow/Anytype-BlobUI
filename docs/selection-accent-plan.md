# Include selections implementation plan

## Setting and placement

Add an enabled-by-default Include selections switch as the first row inside the
Accent color settings box, directly above Anytype Accent Color. Keep the color
picker in that same box. Missing settings on upgrades default to enabled;
explicitly saved false values remain false. The switch also works with the native
Anytype accent.

Use the existing draft, Save, Undo, settings locking and CSS compilation paths.
Persist `includeSelections` in both normalized settings and settingsV2. Add it to
the Rust draft and frontend dirty-state comparison. Gate a dedicated Dev CSS
module with the existing boolean condition mechanism.

## Selection UI inventory

- Selected text in titles, editors, properties, search, inputs, comments and chat.
- Code selections, including syntax-highlighted descendants and dark code themes.
- The separate selection background in outgoing chat bubbles.
- Rendered PDF text selection, preserving transparent line-break selections.
- Selected and keyboard-focused editor blocks of every type using shared selection.
- Selected database rows and cards across table, list, gallery, board, calendar and
  timeline views where the shared selection indicator is used.
- Selected object-list and picker rows using the shared selected-row state.
- Simple-table cell, range, row, column and whole-table frames and fills.
- Row and column handles in both narrow and expanded states, and the selected
  cell ellipsis outline. Text-property popup borders in menus and inline editors.
- Selected text inside the BlobUI Settings app, following the draft toggle and
  accent immediately, with Save and Undo using the existing settings workflow.
- Existing blue editing outlines around database and property cells.
- Drag selection rectangle fill and border, including popup rectangles.
- Editor block and column drop zones and insertion lines.
- Database row and card drop indicators.
- Sidebar and widget drop highlights and draggable-item insertion markers.
- Media, cover and image/icon upload drop zones.
- Comment drag insertion indicators.
- Blue selected-choice indicators in navigation, theme and database layout pickers.
- Blue focused input borders and keyboard focus outlines in forms and dialogs.
- The active text-search match.
- Any explicitly blue typing caret. None was found in the inspected stylesheet,
  so ordinary text-colored carets keep their current colors.

Existing BlobUI accents on date choices, checked controls and active filters keep
their current behavior independently of this switch. Neutral choice indicators
retain their native styling.

## CSS behavior and preservation

The table handle and cell-menu selectors follow
[Anytype table.scss](https://github.com/anyproto/anytype-ts/blob/develop/src/scss/block/table.scss).
The shared inline and menu text-property editor follows
[Anytype text.scss](https://github.com/anyproto/anytype-ts/blob/develop/src/scss/menu/dataview/text.scss).

Use the BlobUI accent with translucent selection fills and lighter frame borders.
Scope overrides to selection UI rather than redefining global Anytype selection
tokens, which also color banners and notices. Preserve selected text colors,
text effects, geometry, hidden indicators, pointer behavior and forced colors.
Turning the switch off removes the new overrides.

Preserve existing work, including the untracked calendar-settings test. Do not
modify live Anytype files outside this workspace or Releases custom.css files.

## Final validation only

Testing and compilation happen only at the end, after implementation and static
review. Add the checks during implementation without running them early.

Check migration, persistence, explicit false, Save, Undo, reload and native mode.
Check the real generated CSS in Chrome against native Anytype styles using light
and dark themes and native, preset, custom and translucent accents. Check keyboard
operation, text/code/chat/PDF selection, tables, records, drag feedback, choice and
focus indicators, unrelated notices and ordinary carets.

Run relevant Rust and frontend checks, build Settings with
`scripts/Build-App.ps1 -s -c`, then build the installer with
`scripts/Build-Installer.ps1`. Keep test profiles and generated files in the
workspace. Fix failures and repeat affected checks before handoff. Target any PR
at dev and verify origin/main is an ancestor of dev and dev of the feature branch.
