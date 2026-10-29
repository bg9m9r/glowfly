#!/usr/bin/env bash
# Builds Glowfly's Flatpak bundle (packaging/flatpak/io.github.bg9m9r.glowfly.yml) in two steps,
# because the Flatpak build has no network:
#
#   bash scripts/package-flatpak.sh vendor        # on a host with cargo: the crates into vendor/
#   bash scripts/package-flatpak.sh bundle v0.2.0  # where flatpak-builder is installed
#   bash scripts/package-flatpak.sh git-source     # prints the git archive's `<url> <sha256>`
#
# `bundle` writes dist/glowfly-<tag>-<triple>.flatpak and its .sha256, and beside them the git
# source archive the bundle's git was built from (git-<version>.tar.xz, the manifest's url and
# sha256) and its .sha256: git is GPL-2.0, so its source goes out with every release that has a
# .flatpak (packaging/flatpak/GIT-NOTICE.md). The bundle's runtime (org.freedesktop.Platform)
# installs from Flathub (--runtime-repo). The Release and Installers workflows run `bundle` in
# the Flathub builder image.
set -euo pipefail
cd "$(dirname "$0")/.."

APP_ID="io.github.bg9m9r.glowfly"
FLATHUB="https://dl.flathub.org/repo/flathub.flatpakrepo"
generated="packaging/flatpak/generated"

usage() {
  echo "usage: package-flatpak.sh vendor | bundle <tag> | git-source" >&2
  exit 2
}

sum() {
  (cd "$(dirname "$1")" && sha256sum "$(basename "$1")" > "$(basename "$1").sha256")
}

# The git module's source archive in the manifest, as `<url> <sha256>`: the only archive source
# there, with git's kernel.org address.
git_source() {
  local manifest="packaging/flatpak/$APP_ID.yml" found
  found="$(awk '
    { sub(/[[:cntrl:]]+$/, "") }
    $1 == "url:" { url = $2; urls++ }
    $1 == "sha256:" { sha = $2; shas++ }
    END { if (urls == 1 && shas == 1) print url, sha }
  ' "$manifest")"
  case "$found" in
    "https://www.kernel.org/pub/software/scm/git/git-"*".tar.xz "*) ;;
    *) echo "expected one git archive (url and sha256) in $manifest" >&2; exit 1 ;;
  esac
  if ! [[ "${found#* }" =~ ^[0-9a-f]{64}$ ]]; then
    echo "the git archive's sha256 in $manifest isn't 64 hex digits" >&2
    exit 1
  fi
  printf '%s\n' "$found"
}

# Copies the git archive flatpak-builder downloaded (else downloads it again) into dist/, checked
# against the manifest's sha256, with its .sha256.
export_git_source() {
  local src url sha file cached
  src="$(git_source)"
  url="${src% *}"
  sha="${src#* }"
  file="${url##*/}"
  cached="target/flatpak/state/downloads/$sha/$file"
  if [ -f "$cached" ]; then
    cp "$cached" "dist/$file.part"
  else
    curl -fsSL --retry 3 -o "dist/$file.part" "$url"
  fi
  if ! (cd dist && echo "$sha  $file.part" | sha256sum -c - > /dev/null); then
    echo "dist/$file doesn't match the manifest's sha256 $sha" >&2
    exit 1
  fi
  mv "dist/$file.part" "dist/$file"
  sum "dist/$file"
}

# The licence and notice files the build put into /app, checked before it is bundled.
check_docs() {
  local files="target/flatpak/build/files" f
  for f in glowfly/LICENSE-MIT glowfly/LICENSE-APACHE glowfly/THIRD-PARTY-NOTICES.md glowfly/GIT-NOTICE.md \
    git/COPYING git/LGPL-2.1 git/reftable-LICENSE git/sha1dc-LICENSE.txt; do
    [ -s "$files/share/doc/$f" ] || { echo "the build has no /app/share/doc/$f" >&2; exit 1; }
  done
  grep -q 'GNU GENERAL PUBLIC LICENSE' "$files/share/doc/git/COPYING" || { echo "/app/share/doc/git/COPYING isn't the GPL" >&2; exit 1; }
  grep -q 'GNU LESSER GENERAL PUBLIC LICENSE' "$files/share/doc/git/LGPL-2.1" || { echo "/app/share/doc/git/LGPL-2.1 isn't the LGPL" >&2; exit 1; }
}

vendor() {
  mkdir -p "$generated"
  # cargo prints the [source] config that points at vendor/; the manifest copies it into the
  # build root's .cargo/config.toml, where the relative path resolves against the build root.
  cargo vendor --locked vendor > "$generated/cargo-vendor.toml"
  if ! grep -q 'vendored-sources' "$generated/cargo-vendor.toml"; then
    echo "cargo vendor printed no [source] config to $generated/cargo-vendor.toml" >&2
    exit 1
  fi
}

bundle() {
  local tag="$1"
  if [ ! -d vendor ] || [ ! -f "$generated/cargo-vendor.toml" ]; then
    echo "vendor/ or $generated/cargo-vendor.toml is missing: run 'bash scripts/package-flatpak.sh vendor' first" >&2
    exit 1
  fi
  local version="${tag#[vV]}"
  local date
  date="$(date -u +%F)"
  sed -e "s/@VERSION@/$version/g" -e "s/@DATE@/$date/g" \
    "packaging/flatpak/$APP_ID.metainfo.xml.in" > "$generated/$APP_ID.metainfo.xml"

  if command -v desktop-file-validate > /dev/null; then
    desktop-file-validate "packaging/flatpak/$APP_ID.desktop" "packaging/flatpak/$APP_ID.links.desktop"
  fi
  if command -v appstreamcli > /dev/null; then
    appstreamcli validate --no-net "$generated/$APP_ID.metainfo.xml"
  fi

  flatpak remote-add --user --if-not-exists flathub "$FLATHUB"
  flatpak-builder --user --install-deps-from=flathub --disable-rofiles-fuse --force-clean \
    --default-branch=stable --state-dir target/flatpak/state --repo target/flatpak/repo \
    target/flatpak/build "packaging/flatpak/$APP_ID.yml"
  check_docs

  local arch triple
  arch="$(flatpak --default-arch)"
  case "$arch" in
    x86_64) triple="x86_64-unknown-linux-gnu" ;;
    aarch64) triple="aarch64-unknown-linux-gnu" ;;
    *) echo "Unsupported Flatpak architecture $arch" >&2; exit 2 ;;
  esac
  mkdir -p dist
  local out="dist/glowfly-$tag-$triple.flatpak"
  flatpak build-bundle --runtime-repo="$FLATHUB" --arch="$arch" target/flatpak/repo "$out" "$APP_ID" stable
  sum "$out"
  export_git_source
}

case "${1:-}" in
  vendor) [ "$#" -eq 1 ] || usage; vendor ;;
  bundle) [ "$#" -eq 2 ] || usage; bundle "$2" ;;
  git-source) [ "$#" -eq 1 ] || usage; git_source ;;
  *) usage ;;
esac
