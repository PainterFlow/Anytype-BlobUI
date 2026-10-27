# Kanban formulas

Open Kanban View Formulas in the Settings sidebar. With Settings Future Proof UI Refresh enabled, use Preferences > Kanban > Kanban View Formulas.

The page follows Anytype's UI philosophy. Search the real views available to your API key, then select its flat list row to open the calculator dialog. The magnifier expands into a search field, the filter narrows the list by space, and the reload icon reloads available views. Each entry shows its space, source name, and source kind so duplicate names remain distinguishable.

## Calculator

The eye toggle switches between read-only notation and editable LaTeX. The Math/Logic switch changes the five-column keypad without moving the source selection. Each dialog opens with Math selected. Keys insert at the selection in source mode, including in incomplete drafts. C clears the source. Save closes the dialog and keeps valid edits in the shared draft. Sidebar Save publishes them and Undo restores the saved calculation.

Arithmetic supports `+ - * / ^`. Powers associate to the right. Use `\sqrt{value}` or `\sqrt[degree]{value}` for roots. Root degrees are whole numbers from 1 to 1024. Odd roots accept negative values.

Logic supports `&&`, `||`, `!` and the literals `true` and `false`. Comparisons support `==`, `!=`, `<`, `<=`, `>`, `>=`. The `=` alias also means equality. LaTeX aliases are `\land`, `\lor`, `\neg`, `\ne`, `\le`, and `\ge`. Boolean results display as True or False.

A conditional `condition ? a : b` selects one value and can appear inside a larger calculation. For example, `(\count > 0 ? 10 : 0) + 5` returns 15 for a nonempty category. Zero is false and other finite numbers are true in conditions. AND, OR, and conditional branches evaluate only as needed. A skipped branch cannot cause a runtime division error, but every branch must have valid syntax and available properties.

Arithmetic, ordering comparisons, roots, and aggregations require numeric values. Convert a boolean with `condition ? 1 : 0`. Equality does not convert types, so `true == 1` is false. An evaluated missing value produces No value, including in a condition.

Precedence runs from conditional, OR, AND, equality, ordering comparisons, addition/subtraction, multiplication/division, to powers. Prefix NOT binds above arithmetic. Existing unary signs keep their arithmetic precedence. Parentheses override precedence. Powers and conditionals associate to the right.

Insert property asks for a number or checkbox property and an aggregation. References use stable keys internally and readable quoted names in source, for example `\property{"Estimate"}{"sum"}`. Duplicate names use numbered selectors shown in the picker. Supported aggregations are Total, Average, Minimum, Maximum, and Median.

The optional third argument calculates a value per card before aggregation. To count checked cards, use `\property{"Done"}{"sum"}{\property{"Done"} ? 1 : 0}`. Bare property references are allowed inside that argument. Checkbox reads and ordinary checkbox aggregates retain their numeric behavior. Checked values are 1, unchecked or missing values are 0, and ordinary checkbox aggregates round their clamped result to 0 or 1. Per-card conditional totals can exceed 1.

Each category uses all matching cards. Numeric aggregates ignore missing values. Empty numeric totals and card counts return zero; other empty numeric aggregates return No value. Empty checkbox aggregates return zero. Invalid operations, incompatible values, and unavailable properties produce inline errors.

## Saving calculations

The sidebar Save and Undo include calculations and the display preference. Searching, changing filters, or changing Settings pages preserves pending edits.

Save reads fresh data and publishes every saved calculation as one snapshot. Press Ctrl+R in Anytype after saving. Results remain snapshots until the next save. The toolbar reload action discovers views without publishing calculation results.

Publication updates the workspace configuration, installed BlobUI data, and stylesheet in one transaction. A failed API read or file replacement retains the previous snapshot and pending edits. Normal appearance saves preserve the snapshot. The API key remains in the native backend; the browser never receives it.

Display formulas under Preferences > Kanban shares the grouped settings list and picker control with Board layout. Its value is independent of Board layout. Balanced is the default and places results beside category titles, wrapping results to a second line when needed. Spacious places them below the title. With Collapsed Categories enabled, results fade in while a category is hovered or focused and fade out when it is left.

## Compatibility and storage

This uses the official Anytype app and its local API. It does not patch Anytype, inject JavaScript, enable debugging access, or poll in the background. The sample-data prototype is removed.

Results require an exact object, block, active-view, and category identity. Single-select grouping uses option IDs resolved from the view's stored groups. Checkbox grouping uses true and false. Multi-select grouping is unavailable until API v2 exposes enough information to match its columns reliably. Duplicate option names prevent publication.

Views opened from a Type are discovered through API v2's type documents. Their cards are read through type-scoped search with the stored view's filters and sorts. Query and collection views use their view membership endpoints. Inline boards follow their referenced query, collection, or Type for membership and category identities while keeping their host page and block IDs for CSS targeting. Unreadable or unsupported sources remain unavailable.

Unused single-select options without known IDs do not block publication. A card using an unresolved option still prevents publication. A missing saved view or property retains its calculation until you remove or replace it and save. Incomplete pagination prevents publication independently of API notices.

### API warning handling

Warnings are classified for both paginated results and document metadata reads. Discovery first checks whether a document contains a Kanban view. Warnings on documents without boards do not affect Kanban discovery. On board documents, warnings confined to text, mentions, icons, or covers are also excluded because that content is not a calculation input. Membership, source, filter, property, and identity checks remain in place. Informational notices are deduplicated per discovery or publication and passed to Logs at info level. Blocking messages include the API's explanation and identify the affected part of the calculation.

| API finding | Handling |
| --- | --- |
| Date comparisons include undated objects | Info. Preserve the stored filter's existing semantics. |
| The first stored view was selected because no view was requested | Info. Block if BlobUI explicitly requested a view instead. |
| Transparent document containers are lifted without losing children | Info. This changes document structure, not card membership or option identity. |
| Deleted-reference notices or unreadable optional remote-file metadata | Info when identified by the corresponding semantic code. Calculations do not fetch remote file bytes. |
| Skipped spaces, loading stores, or omitted live targets | Block because the result set may be incomplete. |
| Omitted requested fields or dropped property values | Block because calculation inputs are missing. |
| Ignored placeholders, dropped filters, or normalized filter operands | Block because the requested filter was not applied exactly. |
| Unresolved target/type identities or mismatched type identities | Block because exact identity cannot be established. |
| Other dropped or clamped board metadata | Block because the board cannot be reconstructed completely. |
| Malformed or unrecognized warnings | Block with a distinct explanation of the uncertainty. Do not label them as incomplete results. |

The classifier follows Anytype's [v2 warning producers](https://github.com/anyproto/anytype-heart/tree/go-7383-apiv2-phase0/core/api/v2/service) and [AnyBlock issue codes](https://github.com/anyproto/any-block/blob/main/codec/anyblockjson/validate.go). Supplied semantic codes take precedence over message wording. Current v2 responses omit those codes, so known legacy warning families are matched by message and path. New warnings require classification before they can be treated as informational.

Result snapshots use version 2 with number, boolean, or null values. Version 1 numeric snapshots remain readable. Older app versions cannot read new boolean formulas or snapshots.

The version 2 configuration is `.blobui-kanban-formulas.json` at the source workspace root and is ignored by Git. Definitions retain full space, object, block, and view IDs. A previous `.blobui-formula-prototype.json` stays unchanged; only its display choice migrates when no live configuration exists. A malformed live file stays unchanged until an explicit successful save replaces it.

Implementation and automated validation use workspace-local fixtures. A read-only live probe verifies discovery and calculation without publishing to the user's installed files.
