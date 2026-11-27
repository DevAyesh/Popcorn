#!/usr/bin/env bash
# Installs a Popcorn .deb or .rpm on a clean system (meant for a throwaway container)
# and checks that every library it needs, including the dlopen()ed libmpv, resolves.
#
# Usage: scripts/verify-linux-install.sh path/to/package.{deb,rpm}
# Example: docker run --rm -v "$PWD:/w" -w /w ubuntu:24.04 scripts/verify-linux-install.sh dist/linux/x.deb
set -euo pipefail

PACKAGE="${1:?usage: $0 path/to/package.{deb,rpm}}"
[[ -s "$PACKAGE" ]] || { echo "error: $PACKAGE is missing or empty" >&2; exit 1; }
case "$PACKAGE" in /*) ;; *) PACKAGE="./$PACKAGE" ;; esac

case "$PACKAGE" in
  *.deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq "$PACKAGE"
    ;;
  *.rpm)
    dnf install -y -q "$PACKAGE"
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
