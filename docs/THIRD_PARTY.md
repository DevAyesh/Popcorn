# Third-party software and licenses

Popcorn is free software, released under the
[GNU General Public License v3.0](../LICENSE) (`GPL-3.0-only`). It is built on the open-source
projects listed below. Thank you to everyone who maintains them.

## Contents

- [Summary](#summary)
- [Video playback (libmpv)](#video-playback-libmpv)
- [Build-time tools](#build-time-tools)
- [Rust crates](#rust-crates)
- [Data and artwork](#data-and-artwork)
- [Corresponding source](#corresponding-source)

## Summary

| What | Where it ends up | License notices |
|---|---|---|
| libmpv and the libraries linked into it | `Popcorn.app/Contents/Frameworks/libmpv.2.dylib` | `Popcorn.app/Contents/Resources/licenses/` |
| Rust crates | Compiled into the `Popcorn` executable | Exact versions in [`src-tauri/Cargo.lock`](../src-tauri/Cargo.lock) |
| Build-time tools | Not shipped | Listed below |

Popcorn as a whole is distributed under GPL-3.0, because mpv is built with `gpl=true`
(GPL-2.0-or-later). The other components linked into libmpv are under the LGPL or permissive
licenses (BSD, ISC, MIT, FTL).

## Video playback (libmpv)

Popcorn ships one shared library, `libmpv.2.dylib`.
[`scripts/build-libmpv.sh`](../scripts/build-libmpv.sh) builds it from the source archives below
and links every component statically into it. Each download is pinned to an exact version and
verified against a SHA-256 checksum stored in the script.

| Component | Version | License | Source archive |
|---|---|---|---|
| [mpv](https://mpv.io) | 0.42.0 | GPL-2.0-or-later (built with `gpl=true`) | [v0.42.0.tar.gz](https://github.com/mpv-player/mpv/archive/refs/tags/v0.42.0.tar.gz) |
| [FFmpeg](https://ffmpeg.org) | 9.0.2 | LGPL-2.1-or-later (built without GPL or non-free parts) | [ffmpeg-9.0.2.tar.xz](https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz) |
| [dav1d](https://code.videolan.org/videolan/dav1d) | 1.5.5 | BSD-2-Clause | [dav1d-1.5.5.tar.xz](https://download.videolan.org/pub/videolan/dav1d/1.5.5/dav1d-1.5.5.tar.xz) |
| [libplacebo](https://code.videolan.org/videolan/libplacebo) | 7.387.0 | LGPL-2.1-or-later | [libplacebo-v7.387.0.tar.gz](https://code.videolan.org/videolan/libplacebo/-/archive/v7.387.0/libplacebo-v7.387.0.tar.gz) |
| [libass](https://github.com/libass/libass) | 0.17.5 | ISC | [libass-0.17.5.tar.xz](https://github.com/libass/libass/releases/download/0.17.5/libass-0.17.5.tar.xz) |
| [FreeType](https://freetype.org) | 2.15.0 | FreeType License (FTL) or GPL-2.0-or-later | [freetype-2.15.0.tar.xz](https://download.savannah.gnu.org/releases/freetype/freetype-2.15.0.tar.xz) |
| [FriBidi](https://github.com/fribidi/fribidi) | 1.0.18 | LGPL-2.1-or-later | [fribidi-1.0.18.tar.xz](https://github.com/fribidi/fribidi/releases/download/v1.0.18/fribidi-1.0.18.tar.xz) |
| [HarfBuzz](https://harfbuzz.github.io) | 15.2.0 | MIT ("Old MIT") | [harfbuzz-15.2.0.tar.xz](https://github.com/harfbuzz/harfbuzz/releases/download/15.2.0/harfbuzz-15.2.0.tar.xz) |
| [fast_float](https://github.com/fastfloat/fast_float) (headers, via libplacebo) | 8.4.0 | MIT, Apache-2.0 or BSL-1.0 (used under MIT) | [v8.4.0.tar.gz](https://github.com/fastfloat/fast_float/archive/refs/tags/v8.4.0.tar.gz) |

**Not shipped:** zlib, libiconv, the Swift runtime and Apple's frameworks are part of macOS and
are used from the system.

### Modifications

The sources are used unmodified, with one exception: the build script applies a one-line compile
fix to mpv's `video/out/mac/view.swift`, needed because Popcorn builds mpv with Vulkan disabled.
The script stops with an error if the fix no longer applies.

## Build-time tools

These are used only while building libmpv. None of their code is shipped in the app.

| Component | Version | License | Source |
|---|---|---|---|
| [glad](https://github.com/Dav1dde/glad) (generates libplacebo's OpenGL loader) | 2.0.8 | MIT; generated code also under the Khronos specification licenses | [v2.0.8.tar.gz](https://github.com/Dav1dde/glad/archive/refs/tags/v2.0.8.tar.gz) |
| [Jinja](https://palletsprojects.com/projects/jinja/) (used by glad) | 3.1.6 | BSD-3-Clause | [PyPI: Jinja2 3.1.6](https://pypi.org/project/Jinja2/3.1.6/) |
| [MarkupSafe](https://palletsprojects.com/projects/markupsafe/) (used by Jinja) | 3.0.4 | BSD-3-Clause | [PyPI: MarkupSafe 3.0.4](https://pypi.org/project/MarkupSafe/3.0.4/) |
| [Vulkan-Headers](https://github.com/KhronosGroup/Vulkan-Headers) (needed to compile libplacebo, even with Vulkan off) | commit `74d8a6cb` | Apache-2.0 or MIT | [74d8a6cb930c68ef617b202c3ff3c59d919e086b](https://github.com/KhronosGroup/Vulkan-Headers/tree/74d8a6cb930c68ef617b202c3ff3c59d919e086b) |

The build also uses Meson, Ninja and pkgconf from the developer's machine. They are not shipped
or bundled.

## Rust crates

These are the crates Popcorn depends on directly. Together with their own dependencies, they are
compiled into the `Popcorn` executable. [`src-tauri/Cargo.lock`](../src-tauri/Cargo.lock) pins
the exact version of every crate. Each crate's license text is included in its source package
on [crates.io](https://crates.io).

| Crate | Version | License | Used for |
|---|---|---|---|
| [tauri](https://crates.io/crates/tauri) | 2.12.7 | Apache-2.0 OR MIT | App shell, window and web view |
| [tauri-plugin-dialog](https://crates.io/crates/tauri-plugin-dialog) | 2.8.4 | Apache-2.0 OR MIT | Native folder and file pickers |
| [rusqlite](https://crates.io/crates/rusqlite) | 0.43.0 | MIT | SQLite database |
| [notify-debouncer-full](https://crates.io/crates/notify-debouncer-full) | 0.8.0 | MIT OR Apache-2.0 | Watching the media folder |
| [ureq](https://crates.io/crates/ureq) | 3.5.2 | MIT OR Apache-2.0 | HTTPS requests to TMDB (with rustls) |
| [walkdir](https://crates.io/crates/walkdir) | 2.5.1 | Unlicense OR MIT | Scanning the media folder |
| [regex](https://crates.io/crates/regex) | 1.13.2 | MIT OR Apache-2.0 | Parsing file names |
| [serde](https://crates.io/crates/serde) | 1.0.229 | MIT OR Apache-2.0 | Serialization |
| [serde_json](https://crates.io/crates/serde_json) | 1.0.154 | MIT OR Apache-2.0 | JSON |
| [thiserror](https://crates.io/crates/thiserror) | 2.0.21 | MIT OR Apache-2.0 | Error types |
| [log](https://crates.io/crates/log) | 0.4.35 | MIT OR Apache-2.0 | Logging |

`rusqlite` is built with its `bundled` feature, which compiles in [SQLite](https://sqlite.org),
which is in the public domain.

To list the license of every crate in the dependency tree:

```sh
cd src-tauri
cargo metadata --format-version 1 --locked \
  | jq -r '.packages[] | "\(.name) \(.version)  \(.license)"' | sort
```

## Data and artwork

Movie and TV metadata and images come from [The Movie Database (TMDB)](https://www.themoviedb.org).
Popcorn downloads them at runtime with each user's own API key. No TMDB data is included in this
repository or in the app.

This product uses the TMDB API but is not endorsed or certified by TMDB.

## Corresponding source

This repository at a release tag, together with the pinned source archives linked above, is the
complete corresponding source for that release's binaries. The exact versions, checksums,
configure options and build steps are in
[`scripts/build-libmpv.sh`](../scripts/build-libmpv.sh), and the app is packaged by
[`scripts/package.sh`](../scripts/package.sh).

If a source link stops working, or you cannot get the source for a release, please
[open an issue](https://github.com/dizzpy/Popcorn/issues).
