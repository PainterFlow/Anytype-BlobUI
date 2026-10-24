# BlobUI 1.3.0 - What's new?

Welcome to v1.3.0 of BlobUI. This update introduces a lot of polishing changes, but also some new features too.
The main big feature of this update is the kanban view rework.
You can now control the UI of kanban views, but even cooler is the custom formula support for kanban views!
Yes, you heard that right, kanban views now support custom formulas.
In addition to that, v1.3.0 also ships a few minor improvements & fixes and a few new options here and there.

## General Changes
- Added version tags to changelogs. (Issue#43)

## Kanban Views V2
- Added new "Board layout" option, which controls the UI density of inline & full-page boards:
    - Default: Anytypes default look, not much too say.
    - Balanced: Slightly more compact, definitely a good option for anyone who wants a bit more content on their page.
    - Compact: Very compact UI, properties now also wrap around instead of 1 new line for each property.
- Added new "Collapsed Categories" option, which toggles collapsible kanban views. Hovering over the header will un-collapse the category. (*Works with inline-kanban views too :D*)

### Kanban Custom Formulas (*Requires API key*)
- Kanban views now support custom formulas.
    - Inserting properties are supported
    - Per card calculations are supported
    - Boolean and number properties are supported
    - "Card Count" is supported
    - Per category calculations
    - Ternary operators are supported
    - Boolean operators are supported
    - LaTeX is supported

## Accent Color
- Added new "Include selections" option, which toggles whether selected text, objects, text blocks, etc. should be affected by the accent color.

## Calendar Event Colors
- Added new "Calendar Colors" option, which toggles whether calendar event colors render. (*on by default, the option is locked when the API key is unavailable*)

## Experimental: Settings Future Proof UI Refresh
- Adds tabs for settings.
- Adds collapsible/uncollapsible setting categories.
- Refreshes the look of setting tabs.

## Developer Changelogs
- the compiled version now no longer always shows the update button.
- the compiled version is now marked as the newest.
- updated `AGENTS.md` to include notes about UI design + removed some older stuff.
- renamed `Build-Settings.ps1` script to `Build-App.ps1`.

## Bug fixes
- Issue#47 - The red spelling underline thing is now no longer visible through the spoiler text effect.
- Issue#46 - The red spelling underline now no longer interrupts the neon & rgb wave text effects.
- Issue#44 - The today text in the date modal picker is not affected by the accent color.
- Issue#41 - The default font is set to "Libron".
- Issue#36 - The favorite buttons hover background color does not match the icons color (*when the item is favorited*)
- Issue#35 - The settings tab/category animation can bug out when switching to the "Advanced" settings tab for the first time after fully restarting the app.

---
*Developers Note: All changes without explicitly denoted credits are made by [@PainterFlow11](https://github.com/PainterFlow)*