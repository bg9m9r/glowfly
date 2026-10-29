#!/usr/bin/env bash
# Installs the Flatpak bundle for this user, checks what it exports, its licence and notice
# files (Glowfly's and git's), its version, its update check, its bundled git and the runtime's
# ssh, then uninstalls it. Run by the Installers workflow (.github/workflows/installers.yml)
# after scripts/package-flatpak.sh built the bundle.
#
# Usage: scripts/installer-check-flatpak.sh <bundle.flatpak> <version>
# Environment:
#   INSTALLER_OUT  logs and program output (default: ${RUNNER_TEMP:-$HOME}/installers-flatpak)
#
# The update feeds and download folder are made under $RUNNER_TEMP (else $HOME), never /tmp: the
# sandbox has its own /tmp and wouldn't see them.
set -Eeuo pipefail

APP_ID="io.github.bg9m9r.glowfly"
FLATHUB="https://dl.flathub.org/repo/flathub.flatpakrepo"

fail() {
  echo "::error::flatpak check: $*"
  exit 1
}
log() {
  echo "flatpak check: $*"
}

[ "$#" -eq 2 ] || { echo "usage: installer-check-flatpak.sh <bundle.flatpak> <version>" >&2; exit 2; }
bundle="$1"
version="$2"
[ -f "$bundle" ] || { echo "no file at $bundle" >&2; exit 2; }
command -v flatpak > /dev/null || { echo "flatpak isn't installed" >&2; exit 2; }

out="${INSTALLER_OUT:-${RUNNER_TEMP:-$HOME}/installers-flatpak}"
mkdir -p "$out"
scratch="$(mktemp -d -p "${RUNNER_TEMP:-$HOME}" glowfly-flatpak.XXXXXX)"
INSTALLED=0
cleanup() {
  local status=$?
  set +e
  if [ "$INSTALLED" -eq 1 ]; then
    flatpak uninstall --user -y --noninteractive "$APP_ID" > "$out/cleanup.log" 2>&1
  fi
  rm -rf "$scratch"
  exit "$status"
}
trap cleanup EXIT

# --- Install --------------------------------------------------------------------------------
log "installing $bundle"
flatpak remote-add --user --if-not-exists flathub "$FLATHUB"
flatpak install --user -y --noninteractive "$bundle" > "$out/install.log" 2>&1 || fail "install failed (install.log)"
INSTALLED=1

exports="$HOME/.local/share/flatpak/exports/share"
for f in "applications/$APP_ID.desktop" "applications/$APP_ID.links.desktop" "icons/hicolor/256x256/apps/$APP_ID.png"; do
  [ -f "$exports/$f" ] || fail "the install exported no $f"
done
if command -v desktop-file-validate > /dev/null; then
  desktop-file-validate "$exports/applications/$APP_ID.desktop" "$exports/applications/$APP_ID.links.desktop" \
    || fail "an exported desktop file doesn't validate"
fi

# --- The licences and notices ----------------------------------------------------------------
# Glowfly's, the same as the repository's, and the bundled git's (GPL-2.0, its source attached to
# the release: packaging/flatpak/GIT-NOTICE.md).
root="$(cd "$(dirname "$0")/.." && pwd)"
files="$(flatpak info --user --show-location "$APP_ID")/files"
for f in LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md; do
  cmp -s "$files/share/doc/glowfly/$f" "$root/$f" || fail "/app/share/doc/glowfly/$f is missing or differs from the repository's"
done
cmp -s "$files/share/doc/glowfly/GIT-NOTICE.md" "$root/packaging/flatpak/GIT-NOTICE.md" \
  || fail "/app/share/doc/glowfly/GIT-NOTICE.md is missing or differs from packaging/flatpak/GIT-NOTICE.md"
grep -q 'GNU GENERAL PUBLIC LICENSE' "$files/share/doc/git/COPYING" 2> /dev/null || fail "the bundle has no git COPYING (the GPL)"
grep -q 'GNU LESSER GENERAL PUBLIC LICENSE' "$files/share/doc/git/LGPL-2.1" 2> /dev/null || fail "the bundle has no git LGPL-2.1"
for f in reftable-LICENSE sha1dc-LICENSE.txt; do
  [ -s "$files/share/doc/git/$f" ] || fail "the bundle has no /app/share/doc/git/$f"
done
log "ok licences and notices, Glowfly's and git's"

# --- The program ----------------------------------------------------------------------------
feed="$scratch/feed"
newer="$scratch/newer"
dl="$scratch/dl"
mkdir -p "$feed" "$newer" "$dl"
printf '%s\n' '{"tag_name":"v99.0.0","name":"v99.0.0","assets":[]}' > "$newer/release.json"

run() {
  flatpak run --env=GLOWFLY_UPDATE_FEED="$1" "$APP_ID" "${@:2}"
}

got="$(run "$feed" --version)"
[ "$got" = "glowfly $version" ] || fail "--version printed '$got', expected 'glowfly $version'"

log "update check with an empty feed"
status=0
text="$(run "$feed" --check-update "$dl")" || status=$?
echo "$text" | tee "$out/check-empty.log"
[ "$status" -eq 3 ] || fail "--check-update exited $status with an empty feed, expected 3"
grep -qxF "install flatpak" <<<"$text" || fail "--check-update didn't print 'install flatpak'"

log "update check with a newer release"
status=0
text="$(run "$newer" --check-update "$dl")" || status=$?
echo "$text" | tee "$out/check-newer.log"
[ "$status" -eq 4 ] || fail "--check-update exited $status with a newer release, expected 4"
for line in "install flatpak" "offer 99.0.0" "update-with flatpak"; do
  grep -qxF "$line" <<<"$text" || fail "--check-update didn't print '$line'"
done
[ -z "$(find "$dl" -mindepth 1 -print -quit)" ] || fail "--check-update wrote into the download folder inside Flatpak"

log "bundled git and the runtime's ssh"
git_version="$(flatpak run --command=git "$APP_ID" --version)"
case "$git_version" in
  "git version 2."*) ;;
  *) fail "the bundled git printed '$git_version'" ;;
esac
flatpak run --command=ssh "$APP_ID" -V > "$out/ssh.log" 2>&1 || fail "the runtime has no ssh (ssh.log)"

# --- Uninstall ------------------------------------------------------------------------------
log "uninstalling"
flatpak uninstall --user -y --noninteractive "$APP_ID" > "$out/uninstall.log" 2>&1 || fail "uninstall failed (uninstall.log)"
INSTALLED=0
[ ! -e "$exports/applications/$APP_ID.desktop" ] || fail "the desktop file is still exported after uninstalling"
log "passed"
