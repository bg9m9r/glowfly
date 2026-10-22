#!/usr/bin/env bash
# Installs, upgrades, starts, update-checks and purges the Linux .deb, and checks what each step
# leaves behind. Run by the Installers workflow (.github/workflows/installers.yml) after
# scripts/package-linux.sh built an older package (tag v0.0.1) and the current one from the same
# program. The window is started from the desktop entry's Exec on its own X server (Xvfb, never
# the caller's display) and is seen through the smoke probe (GLOWFLY_SMOKE_PROBE). The steps it
# shares with the .rpm check are in installer-lib-linux.sh.
#
# Usage: scripts/installer-check-linux.sh [--allow-local] <deb> <old deb> <tar.gz> <version>
# Environment:
#   INSTALLER_OUT      logs, program output and the probe (default: ${RUNNER_TEMP:-/tmp}/installers)
#   INSTALLER_TIMEOUT  seconds to wait for the window (default: 60)
#
# It installs and removes the system's glowfly package (with sudo), so it refuses to run outside
# GitHub Actions unless given --allow-local, and always refuses when glowfly is already installed.
set -Eeuo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR source=installer-lib-linux.sh
. "$here/installer-lib-linux.sh"

usage() {
  echo "usage: installer-check-linux.sh [--allow-local] <deb> <old deb> <tar.gz> <version>" >&2
  exit 2
}

allow_local=0
if [ "${1:-}" = --allow-local ]; then
  allow_local=1
  shift
fi
[ "$#" -eq 4 ] || usage

abs() {
  case "$1" in /*) printf '%s\n' "$1" ;; *) printf '%s\n' "$PWD/$1" ;; esac
}
deb="$(abs "$1")"
old_deb="$(abs "$2")"
tgz="$(abs "$3")"
version="$4"

if [ "$(uname -s)" != Linux ]; then
  echo "installer-check-linux.sh runs only on Linux" >&2
  exit 2
fi
if [ "${GITHUB_ACTIONS:-}" != true ] && [ "$allow_local" -ne 1 ]; then
  echo "installer-check-linux.sh installs and removes the glowfly package; run it on CI or pass --allow-local" >&2
  exit 2
fi
installed_status() {
  dpkg-query -W -f="\${Status}" glowfly 2>/dev/null || true
}
if installed_status | grep -q 'install ok installed'; then
  echo "Glowfly is already installed; remove it first (sudo dpkg --purge glowfly)" >&2
  exit 2
fi
missing=()
for tool in dpkg dpkg-query Xvfb jq pwsh desktop-file-validate sha256sum; do
  command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
done
if [ "${#missing[@]}" -gt 0 ]; then
  echo "missing tools: ${missing[*]} (Ubuntu: xvfb jq desktop-file-utils, and PowerShell)" >&2
  exit 2
fi
for f in "$deb" "$old_deb" "$tgz"; do
  [ -f "$f" ] || { echo "no file at $f" >&2; exit 2; }
done

installer_init
INSTALLED=0

cleanup() {
  local status=$?
  set +e
  installer_stop
  # Leave no install behind on a local run.
  if [ "$INSTALLED" -eq 1 ]; then
    as_root dpkg --purge glowfly >"$INSTALLER_OUT/cleanup.log" 2>&1
  fi
  rm -rf "$WORK"
  exit "$status"
}
trap cleanup EXIT

installed_version() {
  dpkg-query -W -f="\${Version}" glowfly 2>/dev/null || true
}

# The package's version is the tag's, possibly with a Debian revision.
assert_version() {
  local got
  got="$(installed_version)"
  case "$got" in
    "$1" | "$1"-*) ;;
    *) fail "installed version is '$got', expected $1" ;;
  esac
}

assert_removed() {
  assert_paths_removed
  if installed_status | grep -q 'install ok installed'; then
    fail "glowfly is still installed after the purge"
  fi
}

# --- Install the old version, then upgrade -------------------------------------------------
as_root dpkg -i "$old_deb" >"$INSTALLER_OUT/install-old.log" 2>&1 || fail "dpkg -i of the old package failed; see install-old.log"
INSTALLED=1
assert_version 0.0.1
log "ok install 0.0.1"
as_root dpkg -i "$deb" >"$INSTALLER_OUT/upgrade.log" 2>&1 || fail "dpkg -i of the current package failed; see upgrade.log"
assert_version "$version"
log "ok upgrade to $version"
assert_installed_files "$(dpkg -L glowfly)" deb
log "ok files, desktop entries, icons, docs and package-format"

# --- The command-line modes, with fresh HOME and XDG folders they must leave alone ---------------
cli_prepare
cli_version /usr/bin/glowfly "$version"
env "${CLI_ENV[@]}" pwsh -NoProfile -File "$here/update-feed-check.ps1" -Glowfly /usr/bin/glowfly -Version "$version" \
  -Installer "$deb" -Archive "$tgz" -Expect installer -Out "$INSTALLER_OUT/feed" || fail "update-feed-check.ps1 failed"
assert_cli_left_nothing

# --- The window --------------------------------------------------------------------------------
window_start /usr/bin/glowfly
log "ok the window drew a frame"

# --- Purge -------------------------------------------------------------------------------------
as_root dpkg --purge glowfly >"$INSTALLER_OUT/purge.log" 2>&1 || fail "dpkg --purge failed; see purge.log"
INSTALLED=0
assert_removed
log "ok purge removed everything"
log "all installer checks passed"
