#!/usr/bin/env bash
# Packages a macOS release build into dist/: Trellis.app in a disk image (with a link to
# /Applications), the tar.gz, and a .sha256 file for each. Run by the Release workflow after
# `cargo build --release --target <target>`.
#
#   bash scripts/package-macos.sh aarch64-apple-darwin v0.2.0
#
# Signing (optional): with APPLE_CERTIFICATE (a base64 .p12), APPLE_CERTIFICATE_PASSWORD and
# APPLE_SIGNING_IDENTITY the app is signed with the hardened runtime and a timestamp, then the
# dmg; with APPLE_ID, APPLE_TEAM_ID and APPLE_APP_PASSWORD too, the dmg is notarized and
# stapled. Without them the app is ad-hoc signed.
set -euo pipefail
cd "$(dirname "$0")/.."

target="$1"
tag="$2"
version="${tag#[vV]}"
name="trellis-$tag-$target"
bin="target/$target/release/glowfly"
dist="dist"
mkdir -p "$dist"

sum() {
  (cd "$(dirname "$1")" && shasum -a 256 "$(basename "$1")" > "$(basename "$1").sha256")
}

# --- The app bundle ----------------------------------------------------------------------------
app="$dist/Trellis.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
cp "$bin" "$app/Contents/MacOS/trellis"
cp assets/icon/trellis.icns "$app/Contents/Resources/trellis.icns"
printf 'APPL????' > "$app/Contents/PkgInfo"

# --- Signing -----------------------------------------------------------------------------------
identity=""
keychain=""
if [ -n "${APPLE_CERTIFICATE:-}" ] && [ -n "${APPLE_CERTIFICATE_PASSWORD:-}" ] && [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  keychain="$RUNNER_TEMP/trellis-signing.keychain-db"
  keychain_password="$(uuidgen)"
  p12="$RUNNER_TEMP/trellis-signing.p12"
  echo "$APPLE_CERTIFICATE" | base64 --decode > "$p12"
  security create-keychain -p "$keychain_password" "$keychain"
  security set-keychain-settings -lut 21600 "$keychain"
  security unlock-keychain -p "$keychain_password" "$keychain"
  security import "$p12" -P "$APPLE_CERTIFICATE_PASSWORD" -A -t cert -f pkcs12 -k "$keychain"
  security set-key-partition-list -S apple-tool:,apple: -k "$keychain_password" "$keychain" > /dev/null
  security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
  rm -f "$p12"
  identity="$APPLE_SIGNING_IDENTITY"
  codesign --force --options runtime --timestamp --sign "$identity" --keychain "$keychain" "$app"
else
  echo "::notice::macOS code signing skipped (no APPLE_CERTIFICATE); the app is ad-hoc signed"
  codesign --force --sign - "$app"
fi
codesign --verify --strict "$app"

# --- The disk image ----------------------------------------------------------------------------
root="$dist/dmg-root"
rm -rf "$root"
mkdir -p "$root"
cp -R "$app" "$root/"
ln -s /Applications "$root/Applications"
dmg="$dist/$name.dmg"
rm -f "$dmg"
hdiutil create -volname Trellis -srcfolder "$root" -ov -format UDZO "$dmg"
rm -rf "$root"

if [ -n "$identity" ]; then
  codesign --force --timestamp --sign "$identity" --keychain "$keychain" "$dmg"
  if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ] && [ -n "${APPLE_APP_PASSWORD:-}" ]; then
    xcrun notarytool submit "$dmg" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
    xcrun stapler staple "$dmg"
  else
    echo "::notice::macOS notarization skipped (no APPLE_ID)"
  fi
fi
sum "$dmg"

# --- The tar.gz (the plain program, as before) -------------------------------------------------
stage="$dist/$name"
rm -rf "$stage"
mkdir -p "$stage"
cp "$bin" LICENSE-MIT LICENSE-APACHE README.md THIRD-PARTY-NOTICES.md "$stage/"
if [ -f CHANGELOG.md ]; then cp CHANGELOG.md "$stage/"; fi
tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"
rm -rf "$stage"
sum "$dist/$name.tar.gz"

ls -l "$dist"
