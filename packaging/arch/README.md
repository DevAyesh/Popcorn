# Arch Linux (`popcorn-bin`) Packaging

This directory contains the `PKGBUILD` for installing Popcorn on **Arch Linux** (and Arch derivatives such as EndeavourOS, CachyOS, and Manjaro) from the release `.tar.gz`.

## Install from a GitHub Release

```bash
cd packaging/arch
makepkg -si
```

## Test a Local Build (Before Releasing)

After running `./scripts/package-linux.sh` from the repository root:

```bash
cp dist/linux/Popcorn_0.1.0_linux_x86_64.tar.gz packaging/arch/
cd packaging/arch
makepkg -si
```

## Publishing / Updating on the AUR

1. Update `pkgver` (and reset `pkgrel=1`) in `PKGBUILD` to match the tagged GitHub Release.
2. Replace `sha256sums_x86_64=('SKIP')` with the `.tar.gz` hash from the release's `SHA256SUMS` (or run `updpkgsums`).
3. Regenerate `.SRCINFO`:
   ```bash
   makepkg --printsrcinfo > .SRCINFO
   ```
4. Commit `PKGBUILD` and `.SRCINFO` to your AUR `popcorn-bin` repository and push.
