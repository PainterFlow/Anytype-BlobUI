# Kanban formulas

Open Kanban View Formulas in the Settings sidebar. With Settings Future Proof UI Refresh enabled, use Preferences > Kanban > Kanban View Formulas.

The page follows Anytype's UI philosophy. Search the real views available to your API key, then select its flat list row to open the calculator dialog. The magnifier expands into a search field, the filter narrows the list by space, and the reload icon reloads available views. Each entry shows its space, source name, and source kind so duplicate names remain distinguishable.

## Calculator

Use the keypad or type numbers, decimal points, parentheses, and `+ - * / ^`. Powers associate to the right. Square root uses `√(value)` and nth root uses `ⁿ√(value, degree)`. Root degrees must be whole numbers from 1 to 1024. Odd roots accept negative values; even roots require nonnegative values.

Insert property lets you choose a numeric property followed by Total, Average, Minimum, or Maximum. Each readable term stores the property's stable key. Card count counts all cards in the current category. For example, insert Total Estimate, press Divide, then insert Card count.

Arrow keys move between terms. Home and End move to the beginning or end. Backspace removes a digit or term; Delete removes the next term. C clears the calculation. Division appears as a stacked fraction. The five-column keypad supports keyboard editing. Use Save to publish the calculation. The dialog closes after a successful save; a failed save keeps the calculation open with its edits. The X, Escape, and backdrop close the dialog.

Each category calculates independently using all matching cards, including cards Anytype has not rendered yet. Numeric aggregates ignore missing values. Empty totals and card counts return zero; other empty aggregates return No value. Invalid operations, incompatible property values, and deleted properties produce inline errors.

## Saving calculations

The sidebar Save and Undo include calculations and the display preference. Searching, changing filters, or changing Settings pages preserves pending edits.

Save reads fresh data and publishes every saved calculation as one snapshot. Press Ctrl+R in Anytype after saving. Results remain snapshots until the next save. The toolbar reload action discovers views without publishing calculation results. Settings shows the last successful calculation time.

Publication updates the workspace configuration, installed BlobUI data, and stylesheet in one transaction. A failed API read or file replacement retains the previous snapshot and pending edits. Normal appearance saves preserve the snapshot. The API key remains in the native backend; the browser never receives it.

Display formulas under Preferences > Kanban shares the grouped settings list and picker control with Board layout. Its value is independent of Board layout. Balanced is the default and places results beside category titles. Spacious places them below the title. Compact uses a decorative f(x) indicator with a result bubble when the category header is hovered. Conservative container breakpoints use Compact where a result may not fit.

## Compatibility and storage

This uses the official Anytype app and its local API. It does not patch Anytype, inject JavaScript, enable debugging access, or poll in the background. LaTeX and the sample-data prototype are removed.

Results require an exact object, block, active-view, and category identity. Select grouping uses option IDs. Checkbox grouping uses true and false. Multi-select grouping follows Anytype's hash of sorted option IDs, including combination categories rather than counting each option separately.

Some API versions or grants expose option names without stable IDs. Some inline or type views lack a filtered membership endpoint. These views remain listed with an explanation, and BlobUI refuses to publish guessed results. A missing saved view or property retains its calculation until you remove or replace it and save. API loading warnings or incomplete pagination prevent publication.

The version 2 configuration is `.blobui-kanban-formulas.json` at the source workspace root and is ignored by Git. Definitions retain full space, object, block, and view IDs. A previous `.blobui-formula-prototype.json` stays unchanged; only its display choice migrates when no live configuration exists. A malformed live file stays unchanged until an explicit successful save replaces it.

Implementation and automated validation use workspace-local fixtures. A read-only live probe verifies discovery and calculation without publishing to the user's installed files.
