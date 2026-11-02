---
name: search-and-answer
description: Answer questions from permitted Anytype content with supporting object links.
---

# Search and answer

Before using tools, read [Anyblob workflow rules](../../references/workflow-rules.md) for permissions, creation, mutation recovery and verification.

Resolve the space with list_spaces or the supplied object reference. Use search_objects, then read_object for likely matches. Follow relevant links only within permission. For named sources inspect get_query_views or get_collection_views and retrieve the relevant objects.

Answer from retrieved content, separate inference from fact, and link material claims to their supporting objects. Identify conflicting sources. State the searched scope and pagination limits. No matches means no matches in the searched permitted content, not proof that the information does not exist. Keep this workflow read-only.
