---
name: audit-organization
description: Audit Anytype content for possible duplicates, missing properties and broken references.
---

# Audit organization

Before using tools, read [Anyblob workflow rules](../../references/workflow-rules.md) for permissions, creation, mutation recovery and verification.

Establish scope and audit criteria. Inspect live types so optional properties are not mislabeled as required. Retrieve permitted objects with pagination appropriate to the scope. Compare IDs, titles, types, content and links. Similar names alone do not prove duplication.

Report candidates with supporting object links and proposed fixes. An unreadable reference may be restricted or unavailable, not broken. State incomplete coverage and avoid definitive totals without all relevant pages. Keep the audit read-only and propose cleanup before deleting anything. If specific changes are later authorized, reread those objects and follow shared mutation rules. Creator and system-object deletion restrictions still apply. Never delete duplicates automatically.
