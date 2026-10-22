#!/usr/bin/env bash
# Packages a Linux release build into dist/: the tar.gz, the .deb and a .sha256 file for each.
# Run by the Release workflow after `cargo build --release --target <target>`; needs cargo-deb.
#
#   bash scripts/package-linux.sh x86_64-unknown-linux-gnu v0.2.0
set -euo pipefail
cd "$(dirname "$0")/.."

target="$1"
tag="$2"
name="glowfly-$tag-$target"
bin="target/$target/release/glowfly"
dist="dist"
mkdir -p "$dist"

sum() {
  (cd "$(dirname "$1")" && sha256sum "$(basename "$1")" > "$(basename "$1").sha256")
}

# The desktop entries, when the validator is installed.
if command -v desktop-file-validate > /dev/null; then
  desktop-file-validate packaging/linux/glowfly.desktop packaging/linux/glowfly-links.desktop
fi

# --- The tar.gz --------------------------------------------------------------------------------
stage="$dist/$name"
rm -rf "$stage"
mkdir -p "$stage"
cp "$bin" LICENSE-MIT LICENSE-APACHE README.md THIRD-PARTY-NOTICES.md "$stage/"
if [ -f CHANGELOG.md ]; then cp CHANGELOG.md "$stage/"; fi
# Install and restart reads this layout (crates/glowfly-core/src/app_update/archive.rs).
tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"
rm -rf "$stage"
sum "$dist/$name.tar.gz"

# --- The .deb (assets and metadata: [package.metadata.deb] in crates/glowfly-app/Cargo.toml) ---
deb="$dist/$name.deb"
cargo deb -p glowfly --no-build --target "$target" --deb-version "${tag#[vV]}" -o "$deb"
sum "$deb"

ls -l "$dist"
