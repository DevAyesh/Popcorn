#!/usr/bin/env bash
# Builds the Linux distributable (.deb) for x86_64 without altering macOS targets.
#
# Usage: scripts/package-linux.sh [extra "cargo tauri build" arguments]
# Needs: Rust (x86_64-unknown-linux-gnu), tauri-cli v2, and GTK3/WebKitGTK/libmpv dev packages.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET=x86_64-unknown-linux-gnu
BUNDLE="${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}/$TARGET/release/bundle"

(cd "$ROOT" && cargo tauri build --target "$TARGET" --bundles deb "$@")

DEB="$(find "$BUNDLE/deb" -maxdepth 1 -type f -name '*.deb' | head -n 1)"
if [[ -z "$DEB" || ! -s "$DEB" ]]; then
  echo "error: expected non-empty .deb package under $BUNDLE/deb" >&2
  exit 1
fi

echo "Built Linux package: $DEB ($(du -h "$DEB" | cut -f1))"
dpkg-deb -I "$DEB"
