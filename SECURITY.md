# Security policy

## Supported versions

Popcorn is in early development. Security fixes are made on `main` and shipped in the next
release. Please make sure you are on the
[latest release](https://github.com/dizzpy/Popcorn/releases/latest) before reporting.

| Version | Supported |
|---|---|
| Latest release | ✅ |
| Older releases | ❌ |

## Reporting a vulnerability

**Please do not report security problems in public issues, discussions or pull requests.**

Report them privately through GitHub instead:

1. Go to the [**Security** tab](https://github.com/dizzpy/Popcorn/security) of this repository.
2. Click **Report a vulnerability**
   ([direct link](https://github.com/dizzpy/Popcorn/security/advisories/new)).
3. Describe the problem, the steps to reproduce it, and the impact you expect.

Please include:

- The Popcorn version and macOS version.
- A proof of concept, if you have one, such as a crafted file name, media file or TMDB response.
- Whether the problem is already publicly known.

### What to expect

- An acknowledgement within **7 days**.
- An assessment and, if confirmed, a plan for a fix. Progress updates at least every 14 days.
- Credit in the release notes and security advisory, unless you prefer to stay anonymous.

Please allow time for a fix to be released before disclosing the issue publicly.

## Scope

Popcorn is a local desktop app. Examples of what is in scope:

- A file name, folder name, subtitle or media file that runs code or script in the app.
- TMDB data (titles, overviews, image paths) that can inject HTML or script into the UI.
- The web UI being able to read or change files outside what it is allowed to.
- Popcorn modifying, moving or deleting files in the media folder.
- Weaknesses in how releases are built, signed or distributed.

Problems in libmpv, FFmpeg or other bundled libraries should also be reported to those projects.
Please still let us know, so we can update the bundled version.

## How Popcorn handles your data

- **TMDB key:** stored in Popcorn's local database at
  `~/Library/Application Support/Popcorn/`, not in the macOS Keychain. It is protected by your
  macOS user account permissions. The key is never sent back to the UI and is only sent to TMDB.
- **Network:** Popcorn only connects to `api.themoviedb.org` and `image.tmdb.org`, over HTTPS.
  There is no telemetry or analytics.
- **Media files:** opened read-only. Popcorn never writes to the media folder.
