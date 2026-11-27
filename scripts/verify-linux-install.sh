#!/usr/bin/env bash
# Installs a Popcorn .deb, .rpm, or .tar.gz on a clean system (meant for a throwaway container)
# and checks that every library it needs, including the dlopen()ed libmpv, resolves.
#
# Usage: scripts/verify-linux-install.sh path/to/package.{deb,rpm,tar.gz}
# Example: docker run --rm -v "$PWD:/w" -w /w ubuntu:24.04 scripts/verify-linux-install.sh dist/linux/x.deb
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 path/to/package" >&2
  exit 1
fi
PACKAGE="$1"
[[ -s "$PACKAGE" ]] || { echo "error: $PACKAGE is missing or empty" >&2; exit 1; }
case "$PACKAGE" in /*) ;; *) PACKAGE="./$PACKAGE" ;; esac

case "$PACKAGE" in
  *.deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get -o APT::Sandbox::User=root install -y -qq "$PACKAGE"
    ;;
  *.rpm)
    dnf install -y -q "$PACKAGE"
    ;;
  *.tar.gz)
    if command -v pacman >/dev/null 2>&1; then
      pacman -Syu --noconfirm --needed tar gzip gtk3 webkit2gtk-4.1 mpv hicolor-icon-theme >/dev/null
    elif command -v dnf >/dev/null 2>&1; then
      dnf install -y -q tar gzip gtk3 webkit2gtk4.1 mpv-libs
    elif command -v apt-get >/dev/null 2>&1; then
      export DEBIAN_FRONTEND=noninteractive
      apt-get update -qq
      apt-get -o APT::Sandbox::User=root install -y -qq tar gzip libgtk-3-0 libwebkit2gtk-4.1-0 libmpv2
    fi
    TMP_TAR="$(mktemp -d)"
    trap 'rm -rf "$TMP_TAR"' EXIT
    tar -xzf "$PACKAGE" -C "$TMP_TAR"
    PREFIX=/usr "$TMP_TAR"/popcorn-*/install.sh --no-check
    ;;
  *)
    echo "error: unsupported package type: $PACKAGE" >&2
    exit 1
    ;;
esac

BIN=/usr/bin/popcorn
test -x "$BIN"
test -f /usr/share/applications/Popcorn.desktop || test -f /usr/share/applications/popcorn.desktop

missing="$(ldd "$BIN" | grep 'not found' || true)"
if [[ -n "$missing" ]]; then
  echo "error: unresolved libraries for $BIN:" >&2
  echo "$missing" >&2
  exit 1
fi

# libmpv is loaded at runtime, so ldd on the binary cannot see it.
export PATH="$PATH:/usr/sbin:/sbin"
ldconfig
LIBMPV="$(ldconfig -p | sed -n 's/^[[:space:]]*libmpv\.so\.2[[:space:]].*=> //p' | head -n 1)"
if [[ -z "$LIBMPV" ]]; then
  echo "error: libmpv.so.2 is not installed; the package dependencies are incomplete" >&2
  exit 1
fi
missing="$(ldd "$LIBMPV" | grep 'not found' || true)"
if [[ -n "$missing" ]]; then
  echo "error: unresolved libraries for $LIBMPV:" >&2
  echo "$missing" >&2
  exit 1
fi

echo "OK: $(basename "$PACKAGE") installed; popcorn and $LIBMPV resolve all libraries"
