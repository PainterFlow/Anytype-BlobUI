---
category: getting-started
index: 1
icon: introduction
name: Introduction
---

# Introduction

BlobUI is a custom CSS theme for Anytype with a Windows desktop app for changing its appearance. It changes fonts, accents, banners, calendar colors, text effects, and parts of Anytype's interface. It does not change the content of your Anytype objects.

## How it works

BlobUI Settings saves your preferences and generates Anytype's `custom.css`. Enable File > Apply Custom CSS in Anytype, then reload Anytype with Ctrl+R after saving changes. The Settings window follows Anytype's light, dark, or system theme and uses your saved interface and code fonts.

The native installer and Settings app support Windows. Using a published release does not require Rust, PowerShell, or a developer toolchain. Anytype must already be installed and have been opened once.

## Optional API access

Appearance settings work without an API key. Connect a read-only Anytype API v2 key if you want BlobUI to read Calendar Color properties and generate calendar styling. You do not need to import the old BlobUI Settings space or select a settings object.

Settings live locally in the selected Anytype data folder. The API key is encrypted with Windows DPAPI for your Windows account, separately from `data.json`. BlobUI reads calendar data through the configured Anytype API and does not edit your objects. Update checks contact GitHub to find published releases.

## Start here

1. Follow [Installation and removal](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/installation-and-removal.md) to install BlobUI and complete onboarding.
2. Open Preferences, choose your appearance, and press Save.
3. Reload Anytype with Ctrl+R and check the result.

Read [Features](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/features.md) for an overview, [QoL Features](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/text-effects-and-previews.md) for text effects, hover previews, and shortcuts, or [Updating](https://github.com/PainterFlow/Anytype-BlobUI/blob/dev/settings-app/ui/docs/updating.md) to maintain an existing installation.

## Documentation videos

Documentation and What's New use the same borderless video player. Move your pointer over a video, focus its controls, or touch it to show them. The large center button plays or pauses. Use the timeline to seek and the mute and volume controls to adjust sound. The colored part of each slider shows the playback position or volume. Fullscreen expands the original recording for a closer look. Autoplay videos is in Advanced Settings > App settings. When enabled, visible videos start muted. Save to keep the setting. A manual pause stays in effect when you scroll away and return.

Click an image to expand it over a darker background. Click the image again to toggle its actual size, and scroll around larger images. Click outside the image or press Escape to return to the article.

## Read offline

Open Help > Documentation in Settings to read the guides without an internet connection. Use the sidebar to change articles and the back button to return to Settings. GitHub and other website links open in your browser and require internet access. The same article files are readable in the repository.
