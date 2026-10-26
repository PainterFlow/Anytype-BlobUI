## Rules:
- Never modify files outside this workspace.
- The custom.css is located at `C:\Users\{user}}\AppData\Roaming\anytype\custom.css` and the data is located at: `C:\Users\{user}}\AppData\Roaming\anytype\blob\*`
- Use `scripts/Build-Installer.ps1` to build the installer and `scripts/Build-App.ps1 -s -c` to build the app.
- When building the Settings app, always run `scripts/Build-App.ps1 -s -c`.
- Make sure to preserve existing changes!
- When making UI, always check `./settings-app/UI_REFERENCE.md` and make sure to follow Anytypes UI philosophy.
- Make sure to always test and build/compile the app after you finished implementing the feature.

## Branch synchronization
- Dev must contain every feature and bugfix on main, plus ongoing development work.
- Target feature and bugfix pull requests at dev. Release main from dev.
- If a fix lands directly on main, merge main into dev before continuing development, then merge dev into active feature branches.
- Before handoff, verify that origin/main is an ancestor of dev and dev is an ancestor of the active feature branch. Preserve both branches' features when resolving conflicts.

## Temporary v1.3.0 branch exception
- Until v1.3.0 is released, do not merge main into dev or require origin/main to be an ancestor of dev. Main currently contains a problematic PR and its revert; dev intentionally retains the development features.
- Create feat/docsV2 from dev and keep dev as its ancestor.
- Once v1.3.0 is released, repair main/dev synchronization while preserving all intended features, before continuing development. Then restore the normal branch synchronization checks above.

## App safety and validation
- Never terminate, restart, or automate the user's running Anytype instance. Do not run live app smoke tests without explicit permission. Use isolated workspace fixtures for validation.
- Keep previews and validation in dark mode. Never switch the user's apps or system to light mode.
- Minimize compilations. Run focused checks first, batch changes, then build once for handoff. Rebuild only for a new compile failure or a material change that requires it; do not rebuild repeatedly for routine checks.
