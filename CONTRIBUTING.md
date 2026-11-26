# Contributing to Popcorn

Thanks for your interest in Popcorn! Bug reports, ideas, documentation fixes and code are all
welcome.

By taking part, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Contents

- [Ways to help](#ways-to-help)
- [Reporting bugs](#reporting-bugs)
- [Suggesting features](#suggesting-features)
- [Setting up for development](#setting-up-for-development)
- [Making a change](#making-a-change)
- [Code guidelines](#code-guidelines)
- [Testing](#testing)
- [Submitting a pull request](#submitting-a-pull-request)
- [License](#license)

## Ways to help

- **Report a bug**, or a file that Popcorn doesn't recognize or matches to the wrong title.
- **Suggest a feature** or an improvement.
- **Improve the docs**: the README, this guide, or comments in the code.
- **Fix a bug or build a feature.** For anything bigger than a small fix, please open an issue
  first so we can agree on the approach before you spend time on it.

## Reporting bugs

Search the [existing issues](https://github.com/dizzpy/Popcorn/issues) first, then
[open a new one](https://github.com/dizzpy/Popcorn/issues/new/choose) using the bug report form.
The most helpful reports include:

- Your Popcorn version (**Settings → About**) and macOS version.
- Steps to reproduce the problem, and what you expected to happen.
- For library or matching problems, the **file and folder names** involved, for example
  `TV Shows/The Office (US)/Season 2/The.Office.S02E01.mkv`.
- **Logs**, if relevant. Popcorn writes its log to the terminal. To see it, quit Popcorn and
  start it from Terminal:

  ```sh
  /Applications/Popcorn.app/Contents/MacOS/popcorn
  ```

**Security problems** should not be reported in public issues. See [SECURITY.md](SECURITY.md).

## Suggesting features

Open a [feature request](https://github.com/dizzpy/Popcorn/issues/new/choose) and describe the
problem you want to solve, not only the solution. Popcorn aims to stay small, fast and simple, so
not every feature will fit, but every idea is read.

## Setting up for development

### Requirements

- A Mac with Apple Silicon, macOS 13 or newer
- Xcode Command Line Tools: `xcode-select --install`
- Rust (stable), via [rustup](https://rustup.rs)
- The Tauri CLI: `cargo install tauri-cli --version "^2" --locked`
- For video playback: `brew install meson ninja pkgconf`

### First run

```sh
git clone https://github.com/dizzpy/Popcorn.git
cd Popcorn

# Build the slim libmpv into build/ (takes a few minutes the first time)
scripts/build-libmpv.sh

# Run the app in development mode
POPCORN_LIBMPV="$PWD/build/prefix/lib/libmpv.2.dylib" cargo tauri dev
```

If you have Homebrew's `mpv` installed, you can skip building libmpv and run `cargo tauri dev`
directly. That is fine for UI and library work. Use the slim build when working on the player.

There is no npm, bundler or build step for the UI: edit files in `ui/` and restart
the app to see the change.

### Where things are

| Path | What it contains |
|---|---|
| `src-tauri/src/library/` | File name parsing, grouping into shows, scanning, watching, search |
| `src-tauri/src/tmdb/` | TMDB client, matching, metadata and image cache |
| `src-tauri/src/player/` | libmpv bindings, embedding the video, playback session |
| `src-tauri/src/db/` | SQLite schema, migrations and queries |
| `ui/js/views/` | One file per screen |
| `ui/js/components/` | Shared UI pieces |
| `ui/css/tokens.css` | Every color, for both themes |
| `docs/API.md` | Every command and event between the UI and Rust |

## Making a change

1. Fork the repository and create a branch from `main`:
   `git checkout -b fix-season-folder-parsing`.
2. Make your change, with tests where it makes sense.
3. Run the [checks](#testing).
4. Commit with a clear message (see below) and open a pull request.

### Commit messages

Write a short summary in the imperative mood and sentence case, without a trailing period,
describing what the change does:

```text
Recognize "Specials" folders as season 0
Keep the embedded video's scale in step with the window
```

Add a body after a blank line if the reason for the change isn't obvious.

## Code guidelines

Popcorn's main goals are a **small, fast and simple app**. Changes are judged with that in mind.

### General

- **Keep it light.** The packaged app must stay under 50 MB (`scripts/package.sh` enforces this).
  Please open an issue before adding a new dependency, Rust crate or JavaScript library.
- **Never modify the user's files.** Popcorn only reads the media folder.
- **Keep it offline-first.** Everything except fetching new metadata must work without internet.
- **Keep platform code isolated** behind `#[cfg(target_os = "macos")]`, so other platforms can
  be added later.

### Rust

- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` must pass.
- No `unwrap()` or `expect()` outside tests and startup. Return `AppResult<T>`, and add a variant
  to `AppError` in `error.rs` if needed. Its message is shown to the user, so make it friendly.
- Keep logic that doesn't need Tauri (parsing, grouping, matching, progress) in pure functions
  with unit tests.
- Commands are `#[tauri::command(async)]` so they don't block the UI.

### UI (HTML, CSS, JavaScript)

- Plain ES modules. No framework, bundler or npm packages.
- **Never insert untrusted text as HTML.** File names and TMDB data must go through
  `textContent` or the `h()` helper in `ui/js/dom.js`, never `innerHTML`.
- **Colors live only in `ui/css/tokens.css`.** `scripts/check-colors.sh` fails otherwise.
  Check that changes look right in both the dark and light themes.
- Keep everything usable with the keyboard.

### Adding or changing a command

A new command must be registered in four places, or the UI will not be allowed to call it. See
[Adding a command](docs/API.md#adding-a-command) in `docs/API.md`, and update that document
whenever a command, event or returned field changes.

## Testing

Run these before opening a pull request. CI runs the Rust checks on every release.

```sh
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cd ..
scripts/check-colors.sh
```

### Player tests

The player tests need a real libmpv and are skipped by default. Run them when you change
anything in `src-tauri/src/player/`:

```sh
cd src-tauri
POPCORN_LIBMPV="$PWD/../build/prefix/lib/libmpv.2.dylib" \
POPCORN_TEST_MEDIA=/path/to/clip.mkv \
  cargo test engine -- --ignored --nocapture
```

The requirements for the test clip are described at the top of
[`engine_tests.rs`](src-tauri/src/player/engine_tests.rs).

For an end-to-end check, debug builds also include a smoke test that plays through the real app.
It never touches your real library. See
[`player/smoke.rs`](src-tauri/src/player/smoke.rs) for how to run it.

### Manual testing

For UI changes, please also try the change in the running app, in both themes, and include a
screenshot in your pull request.

## Submitting a pull request

- Keep each pull request focused on one change. Small pull requests are reviewed faster.
- Fill in the pull request template, and link the issue it fixes (`Fixes #12`).
- Make sure all checks pass.
- Be patient: Popcorn is maintained in spare time. Feedback is about the code, never about you.

## License

Popcorn is licensed under [GPL-3.0](LICENSE). By submitting a contribution, you agree that it
will be released under the same license.
