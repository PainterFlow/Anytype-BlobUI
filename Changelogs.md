# BlobUI 1.2.0

Welcome to BlobUI, BlobUI is an “addon” for Anytype that adds a lot of cool new long requested features.
For example, calendar event colors, text effects, changing the font and much more!
This is the first ever public release and it’s a big one, adding the BlobUI settings app, which is built on Rust + Tauri for maximum performance.
BlobUI only modifies Anytypes custom CSS(UI) and reads your data as context to use for modifying the UI in more advanced ways(*This part can also be disabled, but that will make some features unavailable*).
I haven’t had the chance to implement MacOS support, as I don’t have a MacBook, so yeah, if anyone wants to help, the project is open src :3.
*Developers Note: MacOS support might come soon though.*

## BlobUI App
- BlobUI no longer uses ps1 scripts as backend, everything is done through the app now. ([PainterFlow11](https://github.com/PainterFlow))
- BlobUI now supports changing the API key, together with the port, api key ip and Anytype instillation path. ([PainterFlow11](https://github.com/PainterFlow))
- BlobUI now supports toggling the hover effects. ([PainterFlow11](https://github.com/PainterFlow))
- You can now toggle reduced motion(*it’s on by default*). ([PainterFlow11](https://github.com/PainterFlow))
- And a lot of other cool stuff, before it was just some weird Anytype properties reading thing, now it’s a built-out app which obviously has way better UI and a lot more possibilities. ([PainterFlow11](https://github.com/PainterFlow))

## Smaller Improvements & QoL Changes
- You can now toggle whether you want to have hover effects for @ mentions and links. ([PainterFlow11](https://github.com/PainterFlow))
- You can now toggle whether you want to have a hover effect when hovering over a tab in the tab bar. ([PainterFlow11](https://github.com/PainterFlow))

## Bugfixes
- Fixed A LOT of issues related to the accent color not effecting xyz. ([PainterFlow11](https://github.com/PainterFlow))
- Fixed custom font not applying in the tab bar. ([PainterFlow11](https://github.com/PainterFlow))
- Fixed the scrollbar in the channel sidebar. ([PainterFlow11](https://github.com/PainterFlow))
*Developers Note: We basically removed it, so if you are in a lot of channels, you won’t have a scrollbar, but tbh, I’d rather have no scrollbar when I’m in a lot of channels then have a annoying big long grey box in my sidebar.*

## Developer Changelogs
- removed most non-testing related powershell scripts
- added new `.\scripts\Build-Settings.ps1` script to build the blob ui app, it also has a few arguments ([PainterFlow11](https://github.com/PainterFlow))
    - `-n` next time the app starts, it will start with onboarding.
    - `-s` starts the app once it finished compiling.
    - `-c` closes any old instances of the app.
- added new `.\scripts\Build-Installer.ps1` script to build the installer ([PainterFlow11](https://github.com/PainterFlow))
- integrated GitHub actions for deployment ([PainterFlow11](https://github.com/PainterFlow))