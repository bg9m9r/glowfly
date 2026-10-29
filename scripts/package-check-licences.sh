#!/usr/bin/env bash
# Checks that each package holds Glowfly's licence and notice files (LICENSE-MIT, LICENSE-APACHE
# and THIRD-PARTY-NOTICES.md), byte for byte the repository's. Run by the Release workflow after
# packaging and by the Installers workflow. The installed MSI is checked by
# installer-check-windows.ps1, and the Flatpak by package-flatpak.sh and installer-check-flatpak.sh.
#
#   bash scripts/package-check-licences.sh dist/glowfly-v0.3.0-x86_64-unknown-linux-gnu.tar.gz ...
#
# What it reads from each, by name:
#   *.tar.gz, *.zip   glowfly-<tag>-<target>/ (the archive's own name; unzip, which Git for
#                     Windows' bash has)
#   *.dmg, *.app      Glowfly.app/Contents/Resources/ (the dmg is attached read-only; macOS only)
#   *.AppImage        usr/share/doc/glowfly/ (--appimage-extract, so on its own architecture)
#   *.deb             usr/share/doc/glowfly/ (dpkg-deb)
#   *.rpm             usr/share/doc/glowfly/ (bsdtar, which reads the zstd payload and drops the
#                     leading / that cpio would extract to)
set -Eeuo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"

DOCS=(LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md)

[ "$#" -gt 0 ] || { echo "usage: package-check-licences.sh <package>..." >&2; exit 2; }

fail() {
  echo "::error::licence check: $*"
  exit 1
}

need() {
  command -v "$1" > /dev/null || { echo "licence check: $1 isn't installed (needed for $2)" >&2; exit 2; }
}

scratch="$(mktemp -d -t glowfly-licences.XXXXXX)"
mounted=""
cleanup() {
  local status=$?
  set +e
  [ -n "$mounted" ] && hdiutil detach -quiet "$mounted"
  rm -rf "$scratch"
  exit "$status"
}
trap cleanup EXIT

# Fails unless <dir> holds each of DOCS, the same as the repository's. <what> names the package.
expect_docs() {
  local dir="$1" what="$2" f
  for f in "${DOCS[@]}"; do
    [ -f "$dir/$f" ] || fail "$what has no ${dir#"$scratch"/*/}/$f"
    cmp -s "$dir/$f" "$root/$f" || fail "$what's $f differs from the repository's"
  done
}

n=0
for pkg in "$@"; do
  [ -e "$pkg" ] || fail "no package at $pkg"
  n=$((n + 1))
  work="$scratch/$n"
  mkdir -p "$work"
  abs="$(cd "$(dirname "$pkg")" && pwd)/$(basename "$pkg")"
  name="$(basename "$pkg")"
  case "$name" in
    *.tar.gz)
      tar -xzf "$abs" -C "$work"
      expect_docs "$work/${name%.tar.gz}" "$name"
      ;;
    *.zip)
      need unzip "$name"
      unzip -q "$abs" -d "$work"
      expect_docs "$work/${name%.zip}" "$name"
      ;;
    *.app)
      expect_docs "$abs/Contents/Resources" "$name"
      ;;
    *.dmg)
      need hdiutil "$name"
      mkdir -p "$work/mnt"
      hdiutil attach -quiet -readonly -nobrowse -noautoopen -mountpoint "$work/mnt" "$abs"
      mounted="$work/mnt"
      expect_docs "$work/mnt/Glowfly.app/Contents/Resources" "$name"
      hdiutil detach -quiet "$mounted"
      mounted=""
      ;;
    *.AppImage)
      (cd "$work" && "$abs" --appimage-extract > /dev/null)
      expect_docs "$work/squashfs-root/usr/share/doc/glowfly" "$name"
      ;;
    *.deb)
      need dpkg-deb "$name"
      dpkg-deb -x "$abs" "$work"
      expect_docs "$work/usr/share/doc/glowfly" "$name"
      ;;
    *.rpm)
      need bsdtar "$name"
      bsdtar -xf "$abs" -C "$work" || fail "couldn't unpack $name"
      expect_docs "$work/usr/share/doc/glowfly" "$name"
      ;;
    *) fail "$name isn't a package this check reads" ;;
  esac
  echo "licence check: ok $name"
done
