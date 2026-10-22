## Rules:
- Never modify files outside this workspace.
- The custom.css is located at `C:\Users\{user}}\AppData\Roaming\anytype\custom.css` and the data is located at: `C:\Users\{user}}\AppData\Roaming\anytype\blob\*`
- Use `scripts/Build-Installer.ps1` to build the installer and `scripts/Build-App.ps1 -s -c` to build the app.
- When building the Settings app, always run `scripts/Build-App.ps1 -s -c`.
- Make sure to preserve existing changes!
- Don't modify the custom.css files in the `Releases` folder unless I explicitly give you permission to do so.

## Branch synchronization
- Dev must contain every feature and bugfix on main, plus ongoing development work.
- Target feature and bugfix pull requests at dev. Release main from dev.
- If a fix lands directly on main, merge main into dev before continuing development, then merge dev into active feature branches.
- Before handoff, verify that origin/main is an ancestor of dev and dev is an ancestor of the active feature branch. Preserve both branches' features when resolving conflicts.
