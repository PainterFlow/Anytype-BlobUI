---
name: configure-calendar-colors
description: Configure and publish BlobUI calendar property colors and inheritance through permitted Anyblob tools.
---

# Configure calendar colors

Read [Anyblob workflow rules](../../references/workflow-rules.md) before using tools.

Resolve the permitted space and explicit target with list_calendar_color_targets. Supply space_id on every request. Use object_id to narrow discovery when the user identifies a source. Ask only when names or requested behavior remain ambiguous.

Read get_calendar_color_rule and retain its revision and publication time. Identify type_key and omit target only for a type default. Type defaults require a whole-space Read-Write grant. A view override uses its full space, object, block and view IDs. Inspect eligible properties and use inherit, none, or property with a stable key. Multi-select is unsupported. Inherit uses the type default for a view and Calendar Color for the type default. Missing values retain fallback behavior.

Call preview_calendar_color_rule with the proposed rule. Review category values or object colors and warnings. Never accept a result calculated from an inaccessible subset. Do not change permissions to bypass a denial.

Use set_calendar_color_rule with the current revision to publish the requested change. Use dry_run for mutation validation when useful. For removal use remove_calendar_color_rule; for a requested refresh of saved configuration use refresh_calendar_colors. Both require the current revision. Refresh does not edit saved configuration. Configuration removal needs Read-Write because it removes BlobUI configuration, not Anytype content.

Inspect outcome, revision, publication_status, published_at, warnings and manual_reload_needed. Verify with get_calendar_color_rule. On conflict reread and preview again. Never automatically retry an unknown outcome. Tell the user when Ctrl+R in Anytype is needed; never reload or automate Anytype.

Preserve unrelated configurations and preferences. Do not change the Calendar Colors switch.
