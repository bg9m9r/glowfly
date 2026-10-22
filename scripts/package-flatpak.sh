#!/usr/bin/env bash
# Builds Glowfly's Flatpak bundle (packaging/flatpak/io.github.bg9m9r.glowfly.yml) in two steps,
# because the Flatpak build has no network:
#
#   bash scripts/package-flatpak.sh vendor        # on a host with cargo: the crates into vendor/
#   bash scripts/package-flatpak.sh bundle v0.2.0  # where flatpak-builder is installed
#
# `bundle` writes dist/glowfly-<tag>-<triple>.flatpak and its .sha256. The bundle's runtime
# (org.freedesktop.Platform) installs from Flathub (--runtime-repo). The Release and Installers
# workflows run `bundle` in the Flathub builder image.
set -euo pipefail
cd "$(dirname "$0")/.."

APP_ID="io.github.bg9m9r.glowfly"
FLATHUB="https://dl.flathub.org/repo/flathub.flatpakrepo"
generated="packaging/flatpak/generated"

usage() {
  echo "usage: package-flatpak.sh vendor | bundle <tag>" >&2
  exit 2
}

sum() {
  (cd "$(dirname "$1")" && sha256sum "$(basename "$1")" > "$(basename "$1").sha256")
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
}

case "${1:-}" in
  vendor) [ "$#" -eq 1 ] || usage; vendor ;;
  bundle) [ "$#" -eq 2 ] || usage; bundle "$2" ;;
  *) usage ;;
esac
