#!/usr/bin/env bash
# Installs, upgrades, starts, update-checks and removes the Linux .rpm, and checks what each step
# leaves behind. Run by the Installers workflow (.github/workflows/installers.yml) in a Fedora
# container after scripts/package-linux.sh built an older package (tag v0.0.1) and the current
# one from the same program. The window is started from the desktop entry's Exec on its own X
# server (Xvfb) and is seen through the smoke probe (GLOWFLY_SMOKE_PROBE). The steps it shares
# with the .deb check are in installer-lib-linux.sh.
#
# Usage: scripts/installer-check-rpm.sh [--allow-local] <rpm> <old rpm> <version>
# Environment:
#   INSTALLER_OUT      logs, program output and the probe (default: ${RUNNER_TEMP:-/tmp}/installers)
#   INSTALLER_TIMEOUT  seconds to wait for the window (default: 60)
#
# It installs and removes the system's glowfly package, so it refuses to run outside GitHub
# Actions unless given --allow-local, and always refuses when glowfly is already installed.
set -Eeuo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source-path=SCRIPTDIR source=installer-lib-linux.sh
. "$here/installer-lib-linux.sh"

usage() {
  echo "usage: installer-check-rpm.sh [--allow-local] <rpm> <old rpm> <version>" >&2
  exit 2
}

allow_local=0
if [ "${1:-}" = --allow-local ]; then
  allow_local=1
  shift
fi
[ "$#" -eq 3 ] || usage

abs() {
  case "$1" in /*) printf '%s\n' "$1" ;; *) printf '%s\n' "$PWD/$1" ;; esac
}
rpm_file="$(abs "$1")"
old_rpm="$(abs "$2")"
version="$3"
# The package's version: a pre-release's '-' is '~' (scripts/package-linux.sh).
rpm_version="$(printf '%s' "$version" | tr -- '-' '~')"

if [ "$(uname -s)" != Linux ]; then
  echo "installer-check-rpm.sh runs only on Linux" >&2
  exit 2
fi
if [ "${GITHUB_ACTIONS:-}" != true ] && [ "$allow_local" -ne 1 ]; then
  echo "installer-check-rpm.sh installs and removes the glowfly package; run it on CI or pass --allow-local" >&2
  exit 2
fi
missing=()
for tool in rpm dnf Xvfb jq desktop-file-validate; do
  command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
done
if [ "${#missing[@]}" -gt 0 ]; then
  echo "missing tools: ${missing[*]} (Fedora: xorg-x11-server-Xvfb jq desktop-file-utils)" >&2
  exit 2
fi
if rpm -q glowfly >/dev/null 2>&1; then
  echo "Glowfly is already installed; remove it first (sudo dnf remove glowfly)" >&2
  exit 2
fi
for f in "$rpm_file" "$old_rpm"; do
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
    as_root dnf remove -y glowfly >"$INSTALLER_OUT/cleanup.log" 2>&1
  fi
  rm -rf "$WORK"
  exit "$status"
}
trap cleanup EXIT

assert_version() {
  local got
  got="$(rpm -q --qf '%{VERSION}' glowfly 2>/dev/null || true)"
  [ "$got" = "$1" ] || fail "installed version is '$got', expected $1"
}

# --- Install the old version, then upgrade -------------------------------------------------
as_root dnf install -y "$old_rpm" >"$INSTALLER_OUT/install-old.log" 2>&1 || fail "dnf install of the old package failed; see install-old.log"
INSTALLED=1
assert_version 0.0.1
log "ok install 0.0.1"
as_root dnf install -y "$rpm_file" >"$INSTALLER_OUT/upgrade.log" 2>&1 || fail "dnf install of the current package failed; see upgrade.log"
assert_version "$rpm_version"
log "ok upgrade to $rpm_version"
rpm -qp --requires "$rpm_file" >"$INSTALLER_OUT/requires.txt"
grep -q '^git' "$INSTALLER_OUT/requires.txt" || fail "the package doesn't require git; see requires.txt"
assert_installed_files "$(rpm -ql glowfly)" rpm
log "ok files, desktop entries, icons, docs and package-format"

# --- The command-line modes, with fresh HOME and XDG folders they must leave alone ---------------
cli_prepare
cli_version /usr/bin/glowfly "$version"
cli_install_kind /usr/bin/glowfly rpm
assert_cli_left_nothing

# --- The window --------------------------------------------------------------------------------
window_start /usr/bin/glowfly
log "ok the window drew a frame"

# --- Remove ------------------------------------------------------------------------------------
as_root dnf remove -y glowfly >"$INSTALLER_OUT/remove.log" 2>&1 || fail "dnf remove failed; see remove.log"
INSTALLED=0
assert_paths_removed
if rpm -q glowfly >/dev/null 2>&1; then
  fail "glowfly is still installed after the removal"
fi
log "ok removal took everything"
log "all installer checks passed"
