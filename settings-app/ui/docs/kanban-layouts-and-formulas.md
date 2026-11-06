---
category: getting-started
parent: features
index: 5
icon: features
name: Kanban
---

# Kanban

v1.3.0 adds Board layout, Collapsed Categories, and custom calculations for Kanban views. Layout changes work without API access. Calculations require the optional Anytype API connection.

## Choose a board layout

Open Preferences > Kanban. Board layout applies to both inline and full-page boards.

| Layout | Result |
| --- | --- |
| Default | Keeps Anytype's original board layout. |
| Balanced | Reduces card and column spacing. |
| Compact | Uses 220px columns and puts properties on shared rows that wrap within each card. |

Default is the initial choice. Save in BlobUI and press Ctrl+R in Anytype to apply it.

![Balanced Kanban with Collapsed Categories disabled](@kanban-balanced-expanded)

## Collapse categories

Enable Collapsed Categories to keep category headers visible while hiding cards until you hover over a column or move keyboard focus into it. The option starts off and works with every Board layout. Devices without hover keep cards visible. Reduced Motion disables the expansion animation.

The recording shows Compact spacing with Collapsed Categories enabled.

@kanban-compact-collapsing

## Find a view

1. Configure the connection in Advanced Settings > API key.
2. Open Preferences > Kanban > Kanban View Formulas.
3. Use the magnifier to search, the filter to choose a space, or Reload to discover available views.
4. Select a view. Its space, source name, and source kind help distinguish duplicate names.

Unsupported views are hidden from the list. Each exclusion is recorded as an info entry in Logs with the view's name, source, and reason. Saved calculations for hidden views are preserved. BlobUI does not estimate results from incomplete data.

Views opened from a Type are included alongside query and collection views. Calculations support single-select, multi-select and checkbox grouping. Multi-select cards are grouped by their exact selected-option combination. Original option IDs are required to reproduce Anytype column IDs; missing IDs prevent publication. Duplicate option names or unresolved identities on options used by cards prevent publication.

## Use the calculator

The eye button switches between rendered notation and editable LaTeX. Enable source mode before inserting calculator keys. Math and Logic change the keypad without discarding the expression. C clears it. Insert property opens the property and aggregation pickers; Card count inserts \count.

![Kanban formula calculator with source and property controls](@kanban-formula-calculator)

Try these expressions with properties that exist in the selected view.

Count matching cards with \count, or total a numeric property with \property{"Points"}{"sum"}.

To multiply Price by Quantity on each card, then total the results, use this expression.

```latex
\property{"Price"}{"sum"}{\property{"Price"} * \property{"Quantity"}}
```

Count checked cards with a per-card conditional.

```latex
\property{"Done"}{"sum"}{\property{"Done"} ? 1 : 0}
```

Supported aggregations are Total, Average, Minimum, Maximum, and Median. Insert property asks for an aggregation at the root of a formula. Inside a per-card third argument it inserts the current card's property directly. References keep stable property keys internally while showing readable names.

Arithmetic supports +, -, *, /, powers, square roots, and nth roots. Logic supports comparisons, AND, OR, NOT, boolean values, and conditional expressions. Use a conditional such as condition ? 1 : 0 to convert a boolean into a number for arithmetic. Missing values and invalid operations can produce No value or an inline error. Numeric totals and card counts for empty categories return zero.

## Save and display results

Calculator Save closes the dialog and keeps the edit in the shared draft. The main Save action publishes valid calculations from fresh API data. Press Ctrl+R in Anytype afterward. Undo restores saved calculations and display choices. Search, filters, and page navigation preserve drafts.

Ctrl+R in BlobUI reloads saved calculations and settings before refreshing Anytype lists. Successful Codex publications update the open calculator automatically. Unrelated pending edits stay intact. When both Codex and a local draft change the same calculation, the saved calculation replaces that draft and Logs records a warning. This includes incomplete LaTeX drafts. Undo restores the latest saved version. Returning to BlobUI also checks for saved changes. Reloading saved settings does not evaluate calculations or publish CSS. Toolbar reload buttons refresh only their view lists.

Display formulas is separate from Board layout. Balanced places results beside category titles and wraps when needed. Spacious places them below titles. With Collapsed Categories enabled, results appear while the category is hovered or focused.

Results are saved snapshots, not live polling. Save again to refresh them. Reload in the view browser discovers views without publishing a new result snapshot. Failed API reads retain the previous published snapshot and your pending edits.

For expression rules and storage details, read the [formula reference](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/docs/kanban-formulas.md).

Select categories use exact, case-sensitive option names scoped to a space and stable property key. Names retain their whitespace. Duplicate names exclude the entire property from Kanban results and calendar property colors, with an error naming the space, property and duplicate. Saved formulas and rules remain intact. Unrelated properties continue to publish.

Multi-select calculations use the complete sorted, deduplicated combination, stored as a serialized name array. Checkbox categories use true and false. Missing options, incomplete membership and conflicting metadata block publication.

Version 3 results match every saved card object ID inside the column body, scoped to the host object, block and active view. Empty or unmatched columns show the formula evaluated on an empty card list. A populated column without any matching rendered card also shows that fallback for as long as no anchor is rendered.

Card moves, option renames and membership changes require refresh. Legacy version 1 and 2 results remain readable.
