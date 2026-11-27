#!/usr/bin/env bash
# Builds the Linux distributables (.deb, .rpm, and portable .tar.gz) for x86_64
# without altering macOS targets.
#
# Usage: scripts/package-linux.sh [extra "cargo tauri build" arguments]
# Env:   POPCORN_MAX_GLIBC  newest glibc symbol version the binary may need (default: 2.39,
#                           the glibc of Ubuntu 24.04 where release builds run)
#        POPCORN_DIST_DIR   where to collect the packages (default: dist/linux)
# Needs: Rust (x86_64-unknown-linux-gnu), tauri-cli v2, binutils (objdump), python3,
#        and the GTK3/WebKitGTK 4.1/libmpv development packages.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET=x86_64-unknown-linux-gnu
RELEASE="${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}/$TARGET/release"
BUNDLE="$RELEASE/bundle"
DIST="${POPCORN_DIST_DIR:-$ROOT/dist/linux}"
MAX_GLIBC="${POPCORN_MAX_GLIBC:-2.39}"
VERSION="$(python3 -c "import json; print(json.load(open('$ROOT/src-tauri/tauri.conf.json'))['version'])")"

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

# Build portable .tar.gz (for Arch AUR popcorn-bin and generic Linux installs).
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/popcorn-tar.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT
PKG_DIR="$STAGE/popcorn-${VERSION}-linux-x86_64"
install -d -m 0755 "$PKG_DIR" "$PKG_DIR/icons"

install -m 0755 "$RELEASE/popcorn" "$PKG_DIR/popcorn"
dpkg-deb --fsys-tarfile "$DIST"/*.deb | tar -xO usr/share/applications/Popcorn.desktop > "$PKG_DIR/Popcorn.desktop"
chmod 0644 "$PKG_DIR/Popcorn.desktop"
install -m 0644 "$ROOT/LICENSE" "$PKG_DIR/LICENSE"
install -m 0644 "$ROOT/src-tauri/icons/32x32.png" "$PKG_DIR/icons/32x32.png"
install -m 0644 "$ROOT/src-tauri/icons/128x128.png" "$PKG_DIR/icons/128x128.png"
install -m 0644 "$ROOT/src-tauri/icons/128x128@2x.png" "$PKG_DIR/icons/256x256.png"
install -m 0644 "$ROOT/src-tauri/icons/icon.png" "$PKG_DIR/icons/512x512.png"
install -m 0644 "$ROOT/src-tauri/icons/icon.svg" "$PKG_DIR/icons/popcorn.svg"

cat > "$PKG_DIR/install.sh" <<'EOF'
#!/usr/bin/env bash
# Installs Popcorn into $DESTDIR$PREFIX (defaults to ~/.local for non-root users).
# Usage: [PREFIX=/usr] [DESTDIR=...] ./install.sh [--no-check]
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
DESTDIR="${DESTDIR:-}"
CHECK_LIBS=1
[[ "${1:-}" == "--no-check" || -n "$DESTDIR" ]] && CHECK_LIBS=0

LDCONFIG="$(PATH="$PATH:/usr/sbin:/sbin" command -v ldconfig || true)"
if [[ "$CHECK_LIBS" -eq 1 && -n "$LDCONFIG" && -r /etc/ld.so.cache ]]; then
  cache="$("$LDCONFIG" -p 2>/dev/null || true)"
  if [[ -n "$cache" ]]; then
    missing=()
    for soname in libgtk-3.so.0 libwebkit2gtk-4.1.so.0 libmpv.so.2; do
      grep -Fq "$soname" <<<"$cache" || missing+=("$soname")
    done
    if [[ "${#missing[@]}" -gt 0 ]]; then
      echo "warning: missing runtime libraries: ${missing[*]}" >&2
      echo "  Arch:          sudo pacman -S gtk3 webkit2gtk-4.1 mpv" >&2
      echo "  Fedora:        sudo dnf install gtk3 webkit2gtk4.1 mpv-libs" >&2
      echo "  Ubuntu/Debian: sudo apt install libgtk-3-0 libwebkit2gtk-4.1-0 libmpv2" >&2
    fi
  fi
fi

ROOT_OUT="${DESTDIR}${PREFIX}"
install -Dm755 "$HERE/popcorn" "$ROOT_OUT/bin/popcorn"
install -Dm644 "$HERE/LICENSE" "$ROOT_OUT/share/licenses/popcorn/LICENSE"
for sz in 32x32 128x128 256x256 512x512; do
  install -Dm644 "$HERE/icons/${sz}.png" "$ROOT_OUT/share/icons/hicolor/${sz}/apps/popcorn.png"
done
install -Dm644 "$HERE/icons/popcorn.svg" "$ROOT_OUT/share/icons/hicolor/scalable/apps/popcorn.svg"

install -d "$ROOT_OUT/share/applications"
if [[ "$PREFIX" == "/usr" || "$PREFIX" == "/usr/local" ]]; then
  install -m644 "$HERE/Popcorn.desktop" "$ROOT_OUT/share/applications/Popcorn.desktop"
else
  sed "s|^Exec=popcorn$|Exec=${PREFIX}/bin/popcorn|" "$HERE/Popcorn.desktop" \
    > "$ROOT_OUT/share/applications/Popcorn.desktop"
  chmod 0644 "$ROOT_OUT/share/applications/Popcorn.desktop"
fi

echo "Installed Popcorn to $ROOT_OUT"
EOF
chmod 0755 "$PKG_DIR/install.sh"

cat > "$PKG_DIR/uninstall.sh" <<'EOF'
#!/usr/bin/env bash
# Removes Popcorn from $DESTDIR$PREFIX (defaults to ~/.local).
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
DESTDIR="${DESTDIR:-}"
ROOT_OUT="${DESTDIR}${PREFIX}"

rm -f "$ROOT_OUT/bin/popcorn" \
      "$ROOT_OUT/share/applications/Popcorn.desktop" \
      "$ROOT_OUT/share/icons/hicolor/32x32/apps/popcorn.png" \
      "$ROOT_OUT/share/icons/hicolor/128x128/apps/popcorn.png" \
      "$ROOT_OUT/share/icons/hicolor/256x256/apps/popcorn.png" \
      "$ROOT_OUT/share/icons/hicolor/512x512/apps/popcorn.png" \
      "$ROOT_OUT/share/icons/hicolor/scalable/apps/popcorn.svg"
rm -rf "$ROOT_OUT/share/licenses/popcorn"
echo "Removed Popcorn from $ROOT_OUT"
EOF
chmod 0755 "$PKG_DIR/uninstall.sh"

TARBALL="$DIST/Popcorn_${VERSION}_linux_x86_64.tar.gz"
tar -C "$STAGE" --owner=0 --group=0 --numeric-owner -czf "$TARBALL" "popcorn-${VERSION}-linux-x86_64"
test -s "$TARBALL"

echo
echo "Linux packages in $DIST:"
ls -lh "$DIST"
dpkg-deb -I "$DIST"/*.deb
if command -v rpm >/dev/null; then
  rpm -qip --requires "$DIST"/*.rpm
fi
