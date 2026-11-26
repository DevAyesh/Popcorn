## What does this change?

<!-- A short description of the change and why it's needed. -->

Fixes #

## How was it tested?

<!-- Steps you took to check it works. For UI changes, add screenshots in both the dark and light themes. -->

## Checklist

- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass
- [ ] `scripts/check-colors.sh` passes (if the UI changed)
- [ ] Tried in the running app, in both themes (if the UI changed)
- [ ] Player tests run with `--ignored` (if `src-tauri/src/player/` changed)
- [ ] `docs/API.md` updated (if a command, event or returned field changed)
- [ ] `CHANGELOG.md` updated under **Unreleased** (if users will notice the change)
- [ ] No new dependencies, or discussed in an issue first
