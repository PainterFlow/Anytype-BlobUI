---
name: build-queries-and-views
description: Build Anytype dashboard queries and views with filters, sorting, grouping and layouts.
---

# Build queries and views

Before using tools, read [Anyblob workflow rules](../../references/workflow-rules.md) for permissions, creation, mutation recovery and verification.

Read the query or collection document, current views, type definitions and property options. Inspect get_op_schema before preparing view operations. Resolve live keys and option values. Match filters, sorts, grouping and layouts to the requested dashboard. Preserve unrelated views and their order.

Use create_query for new queries and patch_object with current ETag for existing views. View creation and deletion require Create-Delete permission. Changing a selected source definition or general membership is a separate space-wide operation even if members are editable. Never broaden filters to bypass access or creation restrictions. Validate with dry_run where supported and inspect the resulting views and explicit-view results. Report unsupported filters or layouts. For BlobUI property colors use $configure-calendar-colors. For Kanban calculations use $configure-kanban-calculations. Their configuration tools do not change Anytype views.
