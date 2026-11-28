Kanban calculations evaluate one formula for each category in a board view and place the result in the category header. They use the optional Anytype Local API v2 connection. Results are saved snapshots rather than live values.

## Create a calculation

1. Open Kanban Views > Kanban Calculations.
2. Select a supported board view. Use the search, view type filter, or reload button if the list is long or out of date.
3. Build the formula with the keypad, Insert property, and Card count. Select the eye button when you want to edit the LaTeX source directly.
4. Select Save in the calculator. This keeps the calculation in your Settings draft.
5. Select Save in the Settings sidebar. BlobUI validates the formula, calculates every category, and publishes the snapshot.
6. Press Ctrl+R in Anytype to apply the new snapshot.

Only number and checkbox properties can be inserted. Use Insert property instead of typing a property reference when possible. The picker uses the property's stable key and handles duplicate names correctly.

## Edit the LaTeX source

The eye button switches between the rendered formula and its LaTeX source. Rendered mode is read only. Source mode is the only mode that accepts edits, including edits from the keypad. Keys insert text at the current source selection, even when the source is incomplete.

BlobUI supports a small LaTeX-style subset rather than the complete KaTeX language. The [KaTeX supported functions reference](https://katex.org/docs/supported.html) is useful for general notation, but commands missing from this guide do not work in Kanban calculations.

## LaTeX commands and operators

| Syntax | Meaning |
| --- | --- |
| `\property{"Estimate"}{"sum"}` | Aggregate the Estimate property across the cards in one category. |
| `\property{"Estimate"}{"sum"}{\property{"Estimate"} * 2}` | Calculate the third argument for each card, then aggregate those values. |
| `\property{"Estimate"}` | Inside a per-card calculation, read Estimate from the current card. |
| `\property{"Estimate"}#2{"sum"}` | Select the second property named Estimate. Insert property chooses the correct number automatically. |
| `\count` | Count the cards in the category. |
| `\sqrt{body}` | Square root of body. |
| `\sqrt[degree]{body}` | The root of body with the given degree. Degrees are whole numbers from 1 through 1024. |
| `\frac{numerator}{denominator}` | Divide numerator by denominator. |
| `base^{exponent}` | Raise base to exponent. |
| `\left(expression\right)` | Group an expression. Plain parentheses also work. |

Property and aggregation arguments use quoted strings inside braces. The aggregation can be `total` or `sum`, `average` or `avg`, `median`, `minimum` or `min`, and `maximum` or `max`. The calculator normally writes the canonical forms `total`, `average`, `median`, `minimum`, and `maximum`.

Arithmetic uses `+`, `-`, `*`, `/`, and `^`. The commands `\cdot` and `\times` mean multiplication, while `\div` means division.

Logic uses `true`, `false`, `&&`, `||`, and `!`. The commands `\land`, `\lor`, and `\neg` mean AND, OR, and NOT. Comparisons use `=`, `==`, `!=`, `<`, `<=`, `>`, and `>=`. The commands `\ne`, `\le`, and `\ge` mean not equal, less than or equal, and greater than or equal.

A conditional uses `condition ? yes : no`. Numbers can act as conditions. Zero is false and every other number is true. Arithmetic requires numbers, so convert a boolean result when you need to calculate with it.

```text
\property{"Estimate"}{"average"} >= 4 ? 1 : 0
```

## Examples

Total the Estimate property in each category.

```text
\property{"Estimate"}{"sum"}
```

Total the cost of each card by multiplying Price by Quantity before aggregating.

```text
\property{"Price"}{"sum"}{\property{"Price"} * \property{"Quantity"}}
```

Count checked Done cards. The per-card conversion turns each checked value into 1 and each unchecked value into 0 before the total is calculated.

```text
\property{"Done"}{"sum"}{\property{"Done"} ? 1 : 0}
```

Show whether the average Estimate is at least 4.

```text
\property{"Estimate"}{"average"} >= 4
```

## Ask Anyblob for help

AI is often the easiest way to write a custom calculation because you can describe the result in plain language. When Anyblob is connected to BlobUI Settings, it includes the `$configure-kanban-calculations` skill and dedicated Kanban calculation tools. It can list views, inspect supported properties, preview category results, publish a calculation, refresh it, or remove it while following your space permissions and approval settings.

You can ask Codex for a result like this.

> Use `$configure-kanban-calculations` to total Estimate for each column of the Launch plan board. Preview the results before publishing.

After Anyblob publishes the calculation, press Ctrl+R in Anytype when it tells you that a manual reload is needed.

## Refreshing and limits

Calculations do not update live when cards move, options change, or membership changes. Press Ctrl+R in BlobUI Settings to reload BlobUI and republish the saved calculations, then press Ctrl+R in Anytype. Unsaved draft formulas are not evaluated during this refresh.

Empty categories evaluate the formula with an empty card list. `\count` returns 0 there, and `total` returns 0. For numeric properties, `average`, `median`, `minimum`, and `maximum` have no value when there are no values to use.

LaTeX source is limited to 4,096 characters. A calculation can use at most 256 terms and can nest expressions up to 48 levels.
