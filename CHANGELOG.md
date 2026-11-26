# Changelog

All notable changes to Popcorn are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- README with screenshots, installation steps, a usage guide and keyboard shortcuts.
- Contributing guide, security policy, code of conduct, and issue and pull request templates.
- Full reference of commands, events and data types in `docs/API.md`.
- Rust crate licenses in `docs/THIRD_PARTY.md`.

## [0.1.0] - 2026-11-26

First public release.

### Added

- **Library:** pick a media folder; Popcorn finds movies and episodes, groups episodes into shows
  and seasons, and watches the folder for changes. Files are never copied, moved or renamed.
- **File name parsing** for common naming styles, including release names, `S01E02`, `1x02`
  and `Season N` folders.
- **TMDB metadata:** posters, backdrops, ratings, genres, overviews, cast, directors and episode
  details, cached on disk so the library works offline. Accepts an API key or a read access
  token.
- **Fix match** to link a title to the correct TMDB entry, and refreshing of metadata.
- **Home screen** with a featured title and a Continue watching row; Movies and TV Shows
  screens with sorting and genre and watched filters.
- **Search** across titles, cast and episode names (<kbd>⌘</kbd> <kbd>K</kbd>).
- **Built-in player** based on libmpv, embedded in the main window, with a fallback window. Audio
  track and subtitle selection, external subtitle files, fullscreen, and keyboard shortcuts.
- **Watch progress:** resume where you left off, play the next episode, and automatic or manual
  watched state.
- **Dark and light themes.**
- Slim libmpv build from pinned, checksummed sources; a DMG under 50 MB for Apple Silicon and
  macOS 13 or newer; and a GitHub Actions release workflow.

[Unreleased]: https://github.com/dizzpy/Popcorn/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/dizzpy/Popcorn/releases/tag/v0.1.0
