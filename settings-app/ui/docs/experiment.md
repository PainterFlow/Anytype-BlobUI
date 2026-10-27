---
category: getting-started
index: 9
icon: experiment
name: Experiment
---

# Experiment

Experiments let you try optional Settings interfaces, including Settings Future Proof UI Refresh and Resizable Sidebar.

## Try the refreshed Settings

1. Open Advanced Settings > Experiments.
2. Turn on Settings Future Proof UI Refresh.
3. Review the new layout, then select Save to keep it.

The switch previews the layout as soon as you turn it on. Until you save, the change remains part of your pending settings. Undo restores the saved layout and discards other pending settings too. Existing installations keep the original layout until you opt in. The experiment changes Settings only, not the layout of Anytype objects.

Experiments may break things. Use them at your own risk.

Read [Settings UI](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/settings-ui-refresh.md) for a screenshot and details about navigating the refreshed interface.

## Resizable Sidebar

Choose Off, Resizable Sidebar, or Resizable Sidebar pro under Advanced > Experiments. This experiment works independently of Settings Future Proof UI Refresh. Save commits your selection, and Undo restores the saved selection.

Drag the sidebar’s right edge to resize it. Settings and Documentation share the same width, which is remembered automatically across restarts without Save. Expanded widths range from 160px to 320px and are limited to 40% of the window, with a 160px minimum.

Pro snaps to a 56px icon rail when dragged below 160px. Drag back to 160px to restore labels. Hover over an icon to see its name. Expandable navigation keeps its child icons available.

Focus the divider and use Left or Right to resize by 8px. Home chooses the minimum width, including the icon rail in Pro, and End chooses the maximum. Escape cancels an active drag. Turning the experiment Off restores the original sidebar width while remembering your resized layout.
