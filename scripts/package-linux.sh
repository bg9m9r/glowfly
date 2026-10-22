#!/usr/bin/env bash
# Packages a Linux release build into dist/: the tar.gz, the .deb, the .rpm and the AppImage,
# and a .sha256 file for each. Run by the Release workflow after
# `cargo build --release --target <target>`. Needs cargo-deb, cargo-generate-rpm, curl, and the
# network for the pinned AppImage tools (kept in $GLOWFLY_PACKAGING_TOOLS, by default
# target/packaging-tools, and checked against their pinned SHA-256s).
#
#   bash scripts/package-linux.sh x86_64-unknown-linux-gnu v0.2.0
#   bash scripts/package-linux.sh aarch64-unknown-linux-gnu v0.2.0
set -euo pipefail
cd "$(dirname "$0")/.."

# The AppImage tools: appimagetool and the type-2 runtime it embeds, pinned by release and
# SHA-256 so a changed file fails the build.
APPIMAGETOOL_VERSION="1.9.1"
APPIMAGETOOL_SHA256_X86_64="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
APPIMAGETOOL_SHA256_AARCH64="f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158"
RUNTIME_VERSION="20251108"
RUNTIME_SHA256_X86_64="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"
RUNTIME_SHA256_AARCH64="00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444"

target="$1"
tag="$2"
case "$target" in
  x86_64-*) arch="x86_64" ;;
  aarch64-*) arch="aarch64" ;;
  *) echo "Unsupported target $target" >&2; exit 2 ;;
esac
version="${tag#[vV]}"
# RPM versions can't hold '-': a pre-release's '~' sorts before the release (0.3.0~rc.1).
rpm_version="$(printf '%s' "$version" | tr -- '-' '~')"
name="glowfly-$tag-$target"
bin="target/$target/release/glowfly"
dist="dist"
tools="${GLOWFLY_PACKAGING_TOOLS:-target/packaging-tools}"
mkdir -p "$dist" "$tools"

sum() {
  (cd "$(dirname "$1")" && sha256sum "$(basename "$1")" > "$(basename "$1").sha256")
}

# Downloads <url> to $tools/<file> unless it is there, and checks it against <sha256>.
fetch() {
  local url="$1" file="$2" sha="$3"
  if [ ! -f "$tools/$file" ]; then
    curl -fsSL --retry 3 -o "$tools/$file.part" "$url"
    (cd "$tools" && echo "$sha  $file.part" | sha256sum -c -)
    chmod +x "$tools/$file.part"
    mv "$tools/$file.part" "$tools/$file"
  fi
  (cd "$tools" && echo "$sha  $file" | sha256sum -c - > /dev/null)
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
cargo deb -p glowfly --no-build --target "$target" --deb-version "$version" -o "$deb"
sum "$deb"

# --- The .rpm ([package.metadata.generate-rpm] in crates/glowfly-app/Cargo.toml) ---------------
# -p names the crate's folder.
rpm="$dist/$name.rpm"
cargo generate-rpm -p crates/glowfly-app --target "$target" -o "$rpm" -s "version = \"$rpm_version\""
sum "$rpm"

# --- The AppImage --------------------------------------------------------------------------------
if [ "$arch" = x86_64 ]; then
  tool_sha="$APPIMAGETOOL_SHA256_X86_64"
  runtime_sha="$RUNTIME_SHA256_X86_64"
else
  tool_sha="$APPIMAGETOOL_SHA256_AARCH64"
  runtime_sha="$RUNTIME_SHA256_AARCH64"
fi
tool_file="appimagetool-$APPIMAGETOOL_VERSION-$arch.AppImage"
runtime_file="runtime-$RUNTIME_VERSION-$arch"
fetch "https://github.com/AppImage/appimagetool/releases/download/$APPIMAGETOOL_VERSION/appimagetool-$arch.AppImage" "$tool_file" "$tool_sha"
fetch "https://github.com/AppImage/type2-runtime/releases/download/$RUNTIME_VERSION/runtime-$arch" "$runtime_file" "$runtime_sha"

# One desktop entry at the root (glowfly:// links are registered at run time, for the
# AppImage's own path), its icon beside it, and the program under usr/bin, which AppRun starts.
appdir="$dist/$name.AppDir"
rm -rf "$appdir"
install -D -m 755 "$bin" "$appdir/usr/bin/glowfly"
install -D -m 644 packaging/linux/glowfly.desktop "$appdir/usr/share/applications/glowfly.desktop"
install -D -m 644 packaging/linux/glowfly.desktop "$appdir/glowfly.desktop"
for size in 16 24 32 48 64 128 256 512; do
  install -D -m 644 "assets/icon/glowfly-$size.png" "$appdir/usr/share/icons/hicolor/${size}x$size/apps/glowfly.png"
done
install -D -m 644 assets/icon/glowfly-256.png "$appdir/glowfly.png"
for doc in LICENSE-MIT LICENSE-APACHE README.md THIRD-PARTY-NOTICES.md; do
  install -D -m 644 "$doc" "$appdir/usr/share/doc/glowfly/$doc"
done
ln -s usr/bin/glowfly "$appdir/AppRun"
ln -s glowfly.png "$appdir/.DirIcon"
appimage="$dist/$name.AppImage"
rm -f "$appimage"
# No FUSE on the build machines: the tool runs extracted.
ARCH="$arch" APPIMAGE_EXTRACT_AND_RUN=1 "$tools/$tool_file" --no-appstream --runtime-file "$tools/$runtime_file" "$appdir" "$appimage"
rm -rf "$appdir"
sum "$appimage"

ls -l "$dist"
