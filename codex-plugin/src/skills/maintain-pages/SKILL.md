---
name: maintain-pages
description: Update Anytype sections, checklists and block order while preserving existing content.
---

# Maintain pages

Before using tools, read [Anyblob workflow rules](../../references/workflow-rules.md) for permissions, creation, mutation recovery and verification.

Read the current page with read_document in AnyBlock form and retain its ETag. Inspect get_schema and get_op_schema for intended edits. Use patch_object with small operations anchored to returned block IDs. Text locators must match exactly one block. Preserve unrelated text, marks, links, nesting and properties. A subtree or Markdown read is not a full replacement document.

Distinguish adding a checklist item from completing an existing one. Move existing blocks instead of recreating them when possible. On ETag conflict, reread and recompute against current content. Verify the requested sections and checklist states after the mutation when permitted. Report unresolved conflicts.
