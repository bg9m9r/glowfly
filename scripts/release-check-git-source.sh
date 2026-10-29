#!/usr/bin/env bash
# Checks that a release's files include git's source whenever they include a Flatpak bundle: the
# bundle ships git (GPL-2.0), so the exact archive it was built from (the manifest's url and
# sha256, `package-flatpak.sh git-source`) must sit beside it. Run by the Release workflow's
# checksums job on the downloaded files before SHA256SUMS is written, and by the Installers
# workflow on dist/ after the bundle is built.
#
#   bash scripts/release-check-git-source.sh dist
set -Eeuo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

[ "$#" -eq 1 ] || { echo "usage: release-check-git-source.sh <folder of release files>" >&2; exit 2; }
dir="$1"
[ -d "$dir" ] || { echo "no folder at $dir" >&2; exit 2; }

shopt -s nullglob
bundles=("$dir"/*.flatpak)
if [ "${#bundles[@]}" -eq 0 ]; then
  echo "git source check: no .flatpak, so no git source needed"
  exit 0
fi

src="$(bash "$here/package-flatpak.sh" git-source)"
url="${src% *}"
sha="${src#* }"
file="${url##*/}"
if [ ! -f "$dir/$file" ]; then
  echo "::error::git source check: ${bundles[*]##*/} ships git, but $file (its source, $url) isn't beside it"
  exit 1
fi
if ! (cd "$dir" && echo "$sha  $file" | sha256sum -c - > /dev/null); then
  echo "::error::git source check: $file doesn't match the manifest's sha256 $sha"
  exit 1
fi
echo "git source check: ok $file ($sha)"
