---
category: getting-started
index: 5
icon: features
name: Kanban layouts and formulas
---

# Kanban layouts and formulas

v1.3.0 adds Board layout, Collapsed Categories, and custom calculations for Kanban views. Layout changes work without API access. Calculations require the optional Anytype API connection.

## Choose a board layout

Open Preferences > Kanban in the refreshed layout, or the Kanban section of Preferences in the original layout. Board layout applies to both inline and full-page boards.

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
2. Open Kanban View Formulas from the sidebar. With Settings Future Proof UI Refresh enabled, use Preferences > Kanban > Kanban View Formulas.
3. Use the magnifier to search, the filter to choose a space, or Reload to discover available views.
4. Select a view. Its space, source name, and source kind help distinguish duplicate names.

A listed view can still be unavailable for calculations. BlobUI explains when the API cannot expose exact view membership, properties, or category identities. It does not estimate results from incomplete data.

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

Display formulas is separate from Board layout. Balanced places results beside category titles and wraps when needed. Spacious places them below titles. With Collapsed Categories enabled, results appear while the category is hovered or focused.

Results are saved snapshots, not live polling. Save again to refresh them. Reload in the view browser discovers views without publishing a new result snapshot. Failed API reads retain the previous published snapshot and your pending edits.

For expression rules and storage details, read the [formula reference](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/docs/kanban-formulas.md).
