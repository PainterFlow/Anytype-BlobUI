---
name: manage-types-and-templates
description: Create or update reusable Anytype types, properties and templates after checking existing structures.
---

# Manage types and templates

Before using tools, read [Anyblob workflow rules](../../references/workflow-rules.md) for permissions, creation, mutation recovery and verification.

Inspect list_types, get_type, list_properties, list_property_options and list_templates before proposing structures. Read relevant document and operation schemas. Reuse properties by key and compatible format. Names are not identities. Renaming cannot change a property's key or format. A duplicate display name requires an explicit available key and a clear need.

Schema changes need appropriate space-wide grants. Source membership does not authorize them. Use create_type, create_property, update_type, update_property and create_template for the requested structure. Prefer update_type operations for individual field changes. Sending property_definitions replaces the whole list. Check affected views before removing properties. Preserve unrelated fields and views. Set default_template only when requested. Verify the resulting structures and report usable keys and links.
