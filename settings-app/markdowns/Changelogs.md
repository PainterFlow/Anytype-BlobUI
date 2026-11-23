# BlobUI v1.5.0 - What's new?

This update is a massive technical one, internal files are getting a remake, Anyblob got recoded to now support remote support, BlobUI now has a headless CLI version which supports linux for remote development and so much more. But don't worry, it's not just backend stuff. Documentation is getting removed in favor of in-app tooltips & help buttons and BlobUI now allows you to create your own custom themes.

## General

- The "Preferences" tab is now split up, kanban view & calendar event color settings have been moved out into their own tabs.

*Developers Note: I wanted to remove the requirement to press that stupid random button at the bottom of the tab to open kanban calculations / calendar coloring by property.*
- You can now toggle whether you want to enable the old legacy "Calendar Color" property to color calendars (*default is on*).
- The "Calendar Events", "Kanban Views" and "Chat Themes" settings tabs now all feature a new child tab called: "Showcase".
    - Showcase allows you to see a preview of your settings in the app.
- A variety of labels & titles now feature small help icons that explain restrictions, options, api requirements etc..
- Removed in-app documentation.

*Developers Note: I don't think we need that much documentation for options this simple, most of the documentations have now been moved to showcase tabs, small help icons and the new developer guide.*

## Custom Themes

- The "Colors" tab now features a new section called: "Custom Themes".
    - You can choose a color to generate a theme from.
    - You can also toggle the based theme(*controls text color, etc.*).
    - There are also 2 built-in themes: "OLED" and "Dark Mode+".

## Anyblob (0.2.25 -> 1.2.0)

- Added Linux support. (User#pxlarified)
- The `read_object` tool now returns e-tags.
- Added agents guide feature, by enabled it you can create as many objects as you want that are called "Agents Guide" which will have to be read by any agents wanting to access your space. Supports multiple guides and is per-space. The agent has to follow these rules just like AGENTS.md. All other tools(besides listing spaces) are also looked until the agent has read your guide. (Can be toggled in settings, off by default)
- I hope I fixed a few stale mcp servers that could spawn when re-compiling the app.
- You should no longer experience a variety of cases where an agent would ask you twice for approval.
- Removed project specific settings.

*Developers Note: I'm really sorry to remove this, but I had to, it was too janky to set up in the CLI and I've been working on remote support for like 1 week straight with like 3 recodes of Anyblob.*

## Introducing the all new BlobUI CLI Headless (v1.0.0) :O

- Allows you to use a variety of commands to manage Anyblob remotely.
- Supports Linux & Windows.

*Developers Note: We put guides in the app, so don't worry about documentation.*

## Bugfixes

*Developers Note: Fixed a variety of undocumented backend issues with settings, you shouldn't notice any difference though.*

- Issue#117: The today day in the calendar widget is not effected by the custom accent color setting.
- Issue#115: Some boxed options are weirdly thick at the bottom. 
- Issue#108: When an agent uses "dry_run" it still asks for approval even though no anytype changes happen. 
- Issue#103: There can sometimes be duplicate blob ui mcp servers.
- Issue#100: Rename "Accent color" section to "Accent Color".
- Issue#99: The in-app text selection color uses windows default color.
- Issue#97: Validate Anyblob tool response before asking for permission.
- Issue#83: +/- Image zoom doesn't work.
- Issue#16: None of the text effects or hyperlink modifications work in chats at the moment.

## Developer Changelogs

- Added Linux support. (User#pxlarified)
- Added Linux versions of all scripts. (User#pxlarified)
- Added a new F10 hotkey that opens a component catalog.
- Added public GitHub project so you can see the development process easier.
- Improved the GitHub repo overall.
- Settings are now saved at the location of where the app was installed at (*old settings will migrate, don't worry*).
- Settings are now more structured when saved.
- When exporting settings it will now only export settings.
- All internal app files like cache etc. are now inside the apps instillation folder.
- Improved UI re-usability.

### Logs refresh

- The copy button now changes to a checkmark for a short amount of time after clicked.
- Added a clear button.
- The logs button now changes state depending on whether there are errors or warnings in the logs.

### Developer Guide

- There is now a in-app developer guide which explains:
    - Programming language requirements, instillation etc..
    - Compilation script usage.
    - GitHub rules.
    - Markdown renderer.
    - Custom Themes.
- Explanations are brief and short so you don't have to read that much.

---
*Developers Note: All changes without explicitly denoted credits are made by User#PainterFlow*
