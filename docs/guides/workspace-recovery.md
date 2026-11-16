# Back up and recover a stopped workspace

The release archive includes `tools/workspace_backup.py`. It uses Python 3's standard library; normal CLI, Console and macOS application use do not require Python. It never runs a Skill, restores output permissions, or recreates a successful Receipt.

Stop the owning application and all tasks first. Work from the parent directory containing the workspace and the new destination. Keep the original and any `INCOMPLETE` output until it has been inspected.

```sh
# Select the tool from the release you actually downloaded.
python3 /absolute/release/tools/workspace_backup.py backup \
  --source ./workspace --output ./backup-new --stopped
python3 /absolute/release/tools/workspace_backup.py check --backup ./backup-new
python3 /absolute/release/tools/workspace_backup.py restore \
  --backup ./backup-new --output ./recovered-new
```

Open `recovered-new/workspace` explicitly after successful recovery. The tool refuses an existing destination, links, paths outside the current invocation directory, source changes, running markers and capacity excess. Limits are 10,000 entries, 1 GiB per file and 4 GiB total. A complete snapshot contains a checked file manifest; a failed snapshot is not a valid backup. Do not delete or overwrite the original to retry.

In a portable macOS application, the same optional tool is under `SkillTape.app/Contents/Resources/runtime/tools/`. Private workspaces remain outside the application, in the selected user directory.
