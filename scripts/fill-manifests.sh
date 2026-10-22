#!/usr/bin/env bash
# Fills the Homebrew cask (packaging/homebrew/glowfly.rb.in) and the winget manifests
# (packaging/winget/*.yaml.in) for a release from its SHA256SUMS, into <out dir>: glowfly.rb,
# Glowfly.Glowfly.yaml, Glowfly.Glowfly.installer.yaml and Glowfly.Glowfly.locale.en-US.yaml.
# Run by the Release workflow's manifests job; docs/RELEASING.md says how to submit them.
#
#   bash scripts/fill-manifests.sh v0.3.0 dist/SHA256SUMS out bg9m9r/glowfly
#
# A pre-release tag writes nothing. A file missing from SHA256SUMS (a failed build) fails.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"

[ "$#" -eq 4 ] || { echo "usage: fill-manifests.sh <tag> <SHA256SUMS> <out dir> <owner/repo>" >&2; exit 2; }
tag="$1"
sums="$2"
out="$3"
repo="$4"

if ! [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "::notice::no manifests for pre-release $tag"
  exit 0
fi
[[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || { echo "::error::'$repo' isn't owner/repo" >&2; exit 2; }
[ -f "$sums" ] || { echo "::error::no file at $sums" >&2; exit 2; }
version="${tag#v}"

# The SHA-256 SHA256SUMS lists for <file> (written `<hash>  <file>` or `<hash> *<file>`).
hash_of() {
  local file="$1" found
  found="$(awk -v f="$file" '{ name = $2; sub(/^\*/, "", name) } name == f && length($1) == 64 && $1 ~ /^[0-9a-fA-F]+$/ { print tolower($1); exit }' "$sums")"
  if [ -z "$found" ]; then
    echo "::error::SHA256SUMS has no checksum for $file" >&2
    exit 1
  fi
  printf '%s\n' "$found"
}

mac_arm="$(hash_of "glowfly-$tag-aarch64-apple-darwin.dmg")"
mac_x86="$(hash_of "glowfly-$tag-x86_64-apple-darwin.dmg")"
win_x64="$(hash_of "glowfly-$tag-x86_64-pc-windows-msvc.msi")"
win_arm="$(hash_of "glowfly-$tag-aarch64-pc-windows-msvc.msi")"

mkdir -p "$out"
fill() {
  local template="$1" target="$2"
  # Every value is hex, digits, dots or owner/repo: none holds '|'.
  sed -e "s|@VERSION@|$version|g" -e "s|@TAG@|$tag|g" -e "s|@REPO@|$repo|g" \
    -e "s|@SHA256_MAC_ARM64@|$mac_arm|g" -e "s|@SHA256_MAC_X86_64@|$mac_x86|g" \
    -e "s|@SHA256_WIN_X64@|$win_x64|g" -e "s|@SHA256_WIN_ARM64@|$win_arm|g" \
    "$template" > "$target"
  if grep -n '@[A-Z0-9_]*@' "$target"; then
    echo "::error::$target still has placeholders" >&2
    exit 1
  fi
  echo "wrote $target"
}

fill "$root/packaging/homebrew/glowfly.rb.in" "$out/glowfly.rb"
for manifest in Glowfly.Glowfly.yaml Glowfly.Glowfly.installer.yaml Glowfly.Glowfly.locale.en-US.yaml; do
  fill "$root/packaging/winget/$manifest.in" "$out/$manifest"
done
