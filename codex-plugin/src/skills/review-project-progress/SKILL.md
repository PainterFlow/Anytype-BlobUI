---
name: review-project-progress
description: Review Anytype project tasks for completion, overdue work, blockers and next actions.
---

# Review project progress

Before using tools, read [Anyblob workflow rules](../../references/workflow-rules.md) for permissions, creation, mutation recovery and verification.

Resolve the requested project or source and inspect its views and task schema. Retrieve the relevant tasks. For multiple query views, choose explicit views and deduplicate results by object ID. A default query read covers only the first stored view. Follow pagination before claiming complete counts.

Use actual status and completion properties rather than titles. Compare due dates with the current date in the user's timezone and state the review date. Separate explicit blockers from suspected risks. Missing dates and inaccessible dependencies are unknown, not overdue or resolved. Link findings to objects and state scope limits. Keep reviews read-only unless the user separately requests changes.
