---
name: process-meeting-notes
description: Extract decisions and action items from Anytype meeting notes and create tasks when requested.
---

# Process meeting notes

Before using tools, read [Anyblob workflow rules](../../references/workflow-rules.md) for permissions, creation, mutation recovery and verification.

Read the meeting note and relevant permitted links. Separate decisions, action items and open questions. Preserve attribution and distinguish proposals from decisions. Set owners and due dates only from the notes or the user. Resolve relative dates only with a known meeting date and timezone.

A summary request needs no writes. Create tasks only when requested, after checking existing tasks and live schema and following the shared creation procedure. Include a source meeting link in each task. Add task links or decision sections back to the note only when that edit is within the request, using current block IDs and ETag. Report confirmed tasks and unresolved items.
