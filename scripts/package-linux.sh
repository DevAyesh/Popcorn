#!/usr/bin/env bash
# Builds the Linux distributables (.deb and .rpm) for x86_64 without altering macOS targets.
#
# Usage: scripts/package-linux.sh [extra "cargo tauri build" arguments]
# Env:   POPCORN_MAX_GLIBC  newest glibc symbol version the binary may need (default: 2.39,
#                           the glibc of Ubuntu 24.04 where release builds run)
#        POPCORN_DIST_DIR   where to collect the packages (default: dist/linux)
# Needs: Rust (x86_64-unknown-linux-gnu), tauri-cli v2, binutils (objdump),
#        and the GTK3/WebKitGTK 4.1/libmpv development packages.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET=x86_64-unknown-linux-gnu
RELEASE="${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}/$TARGET/release"
BUNDLE="$RELEASE/bundle"
DIST="${POPCORN_DIST_DIR:-$ROOT/dist/linux}"
MAX_GLIBC="${POPCORN_MAX_GLIBC:-2.39}"

(cd "$ROOT" && cargo tauri build --target "$TARGET" --bundles deb,rpm "$@")

# A binary only runs where glibc is at least as new as the newest symbol it uses.
# Fail instead of silently dropping support for older distributions.
NEEDED_GLIBC="$(objdump -T "$RELEASE/popcorn" | grep -oE 'GLIBC_[0-9]+(\.[0-9]+)+' |
  sed 's/^GLIBC_//' | sort -Vu | tail -n 1)"
if [[ -z "$NEEDED_GLIBC" ]]; then
  echo "error: could not read glibc requirements of $RELEASE/popcorn" >&2
  exit 1
fi
if [[ "$(printf '%s\n%s\n' "$NEEDED_GLIBC" "$MAX_GLIBC" | sort -V | tail -n 1)" != "$MAX_GLIBC" ]]; then
  echo "error: popcorn needs glibc $NEEDED_GLIBC, newer than the allowed $MAX_GLIBC" >&2
  exit 1
fi
echo "glibc requirement: $NEEDED_GLIBC (allowed: <= $MAX_GLIBC)"

rm -rf "$DIST"
mkdir -p "$DIST"
for kind in deb rpm; do
  mapfile -t found < <(find "$BUNDLE/$kind" -maxdepth 1 -type f -name "*.$kind" 2>/dev/null)
  if [[ "${#found[@]}" -ne 1 || ! -s "${found[0]}" ]]; then
    echo "error: expected exactly one non-empty .$kind under $BUNDLE/$kind, found ${#found[@]}" >&2
    exit 1
  fi
  cp "${found[0]}" "$DIST/"
done

echo
echo "Linux packages in $DIST:"
ls -lh "$DIST"
dpkg-deb -I "$DIST"/*.deb
if command -v rpm >/dev/null; then
  rpm -qip --requires "$DIST"/*.rpm
fi
