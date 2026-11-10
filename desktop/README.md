# SkillTape macOS application

Build the Console and native API, then build the independent macOS window from a clean source checkout:

```sh
npm ci --prefix apps/skilltape-console
npm --prefix apps/skilltape-console run build
cargo build --locked --release -p skilltape-console-api
python3 desktop/build_app.py --output /a/new/output/directory
```

The local application uses system WebKit and the existing read-only native Console API. It opens without a terminal or browser tab, while using this installed source workspace and compiled API runtime. The app includes its built UI; it does not embed user artifacts or credentials. The builder refuses to replace an existing application; working-tree previews require `--preview`.

Use **File → Open workspace** to select an existing evidence directory. The API remains read-only and does not execute capture, Skills, verification commands or scripts. Existing CLI workflows retain their own permissions and execution boundaries.

Explicitly saved quantity calculations persist in this application's local storage across window/application restarts, up to 20 records / 512 KB. Browser-only use keeps its original tab-session retention. Neither mode produces a verified Receipt. Unsaved local edits are checked before leaving the view or closing. Command-S invokes the current calculation's save button when available. Removing a saved calculation requires confirmation.

Only the API process started by this application is stopped on exit. Listener ownership is verified before loading the origin; other project services are not reused or killed. Startup and isolated native-window checks use different ports (19001 and 19011) and separate workspaces. The same App ID and normal port should remain stable across upgrades.

`--smoke-check /private/result.json --snapshot /private/view.png` uses nonpersistent WebKit storage and its own empty temporary evidence directory; it never reads the selected real workspace. Native asset and child lifecycle checks are in `desktop/tests/`. Local applications use ad-hoc signing, not a notarized redistribution release.
