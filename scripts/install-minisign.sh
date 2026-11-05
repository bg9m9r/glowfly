#!/usr/bin/env bash
# Installs minisign from its author's release into <dir>, on x86_64 Linux: the Release workflow
# signs SHA256SUMS with it, and the Installers workflow signs its local update feeds with the
# test key. The tarball is checked against the SHA-256 pinned here before anything in it runs.
# That digest is of the file Frank Denis signed: the tarball's .minisig was verified with his key
# (RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3, from minisign's README) when it was
# pinned, so a changed download fails. To update minisign, verify the new release's tarball and
# zip the same way and change the version and both digests here and in install-minisign.ps1.
#
#   bash scripts/install-minisign.sh "$RUNNER_TEMP/minisign"    # then "$RUNNER_TEMP/minisign/minisign"
set -Eeuo pipefail
version=0.12
sha256=9a599b48ba6eb7b1e80f12f36b94ceca7c00b7a5173c95c3efc88d9822957e73
dir="${1:?usage: install-minisign.sh <dir>}"
[ "$(uname -s) $(uname -m)" = "Linux x86_64" ] || { echo "::error::install-minisign.sh installs the x86_64 Linux build only"; exit 1; }
mkdir -p "$dir"
tarball="$dir/minisign-$version-linux.tar.gz"
curl -fsSL --retry 3 -o "$tarball" "https://github.com/jedisct1/minisign/releases/download/$version/minisign-$version-linux.tar.gz"
echo "$sha256  $tarball" | sha256sum -c --quiet -
# The tarball was made on macOS: its extended headers are skipped without a warning.
tar -xzf "$tarball" -C "$dir" --strip-components=2 --warning=no-unknown-keyword minisign-linux/x86_64/minisign
rm -f "$tarball"
"$dir/minisign" -v
