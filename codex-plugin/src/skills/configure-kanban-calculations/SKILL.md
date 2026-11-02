---
name: configure-kanban-calculations
description: Configure and publish BlobUI Kanban calculations using typed calculator tokens through permitted Anyblob tools.
---

# Configure Kanban calculations

Read [Anyblob workflow rules](../../references/workflow-rules.md) before using tools.

Resolve the permitted space and explicit target with list_kanban_views. Supply space_id on every request. Use object_id to narrow discovery when the user identifies a source. Ask only when names or requested behavior remain ambiguous.

Read get_kanban_calculation and retain its revision and publication time. Inspect eligible properties and supported token forms. Build typed tokens using stable property keys and existing calculator syntax. Use numeric or boolean properties only when supported by the view. Preserve category identities, empty-category behavior and aggregation semantics.

Call preview_kanban_calculation with the proposed tokens. Review category values or object colors and warnings. Never accept a result calculated from an inaccessible subset. Do not change permissions to bypass a denial.

Use set_kanban_calculation with the current revision to publish the requested change. Use dry_run for mutation validation when useful. For removal use remove_kanban_calculation; for a requested refresh of saved configuration use refresh_kanban_calculation. Both require the current revision. Refresh does not edit saved configuration. Configuration removal needs Read-Write because it removes BlobUI configuration, not Anytype content.

Inspect outcome, revision, publication_status, published_at, warnings and manual_reload_needed. Verify with get_kanban_calculation. On conflict reread and preview again. Never automatically retry an unknown outcome. Tell the user when Ctrl+R in Anytype is needed; never reload or automate Anytype.

Preserve unrelated configurations and preferences. Board layouts, collapsed categories and formula placement are outside this skill.
