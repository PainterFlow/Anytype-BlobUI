---
category: getting-started
index: 7
icon: features
name: Experimental Settings layout
---

# Experimental Settings layout

v1.3.0 includes Settings Future Proof UI Refresh. It changes the Settings interface and starts off for existing installations. It does not change the layout of your Anytype objects.

## Try the layout

1. Open Advanced Settings > Experiments.
2. Enable Settings Future Proof UI Refresh.
3. Inspect the layout immediately, then press Save to keep it.

Undo changes restores your previous saved layout along with the other pending settings. Experiments may break things; use this option at your own risk.

![Refreshed Settings with Preferences tabs and a Space banner preview](@settings-ui-refresh)

## Navigate the refreshed interface

Preferences uses Accent, Kanban, Calendar, Typography, and Space banner tabs. Section links appear below Preferences and Advanced Settings in the sidebar. Category navigation can be expanded or collapsed. Accessibility remains a single page.

Advanced Settings contains API key, App settings, and Experiments. Tab controls support arrow keys, Home, and End. Switching a tab resets its content to the top and keeps pending edits. Opening another settings page does not save those edits automatically.

Kanban View Formulas moves from its original sidebar entry to a button under Preferences > Kanban. The view browser retains its own search and space filter. The refresh does not add search fields to ordinary settings pages.

## App settings and videos

App settings contains the Anytype Folder picker, Launch at startup, Autoplay videos, and update controls. Autoplay videos sits immediately below Launch at startup and above Updates. It starts on for both new and existing installations.

When autoplay is enabled, embedded documentation and release-note videos start muted when at least half the player is visible. They pause outside the reader or when the app is hidden, and stop at the end. A manual pause stays paused when you scroll away and return. Turn autoplay off and press Save to require manual playback. Unsaved changes preview immediately; Undo restores the saved choice.

Move the pointer over a video to show its controls. They fade after two seconds without movement and hide when you leave. Keyboard focus keeps them visible, and touch reveals them. Controls include play/pause, ten-second skips, seeking, time, mute/volume, and fullscreen. Focus the player and use Space or K to play/pause, the arrow keys to skip, or M to mute.

## Return to the original layout

Disable Settings Future Proof UI Refresh under Advanced Settings > Experiments and save. Both layouts share the same settings and drafts. Your fonts, accent, API connection, and saved appearance choices remain available.
