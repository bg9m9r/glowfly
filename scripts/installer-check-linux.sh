#!/usr/bin/env bash
# Installs, upgrades, starts, update-checks and purges the Linux .deb, and checks what each step
# leaves behind. Run by the Installers workflow (.github/workflows/installers.yml) after
# scripts/package-linux.sh built an older package (tag v0.0.1) and the current one from the same
# program. The window is started from the desktop entry's Exec on its own X server (Xvfb, never
# the caller's display) and is seen through the smoke probe (TRELLIS_SMOKE_PROBE).
#
# Usage: scripts/installer-check-linux.sh [--allow-local] <deb> <old deb> <tar.gz> <version>
# Environment:
#   INSTALLER_OUT      logs, program output and the probe (default: ${RUNNER_TEMP:-/tmp}/installers)
#   INSTALLER_TIMEOUT  seconds to wait for the window (default: 60)
#
# It installs and removes the system's trellis package (with sudo), so it refuses to run outside
# GitHub Actions unless given --allow-local, and always refuses when trellis is already installed.
set -Eeuo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

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
  echo "installer-check-linux.sh installs and removes the trellis package; run it on CI or pass --allow-local" >&2
  exit 2
fi
installed_status() {
  dpkg-query -W -f="\${Status}" trellis 2>/dev/null || true
}
if installed_status | grep -q 'install ok installed'; then
  echo "Trellis is already installed; remove it first (sudo dpkg --purge trellis)" >&2
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

INSTALLER_OUT="${INSTALLER_OUT:-${RUNNER_TEMP:-/tmp}/installers}"
INSTALLER_TIMEOUT="${INSTALLER_TIMEOUT:-60}"
mkdir -p "$INSTALLER_OUT"
INSTALLER_OUT="$(cd "$INSTALLER_OUT" && pwd)"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/trellis-installer.XXXXXX")"
XVFB_PID="" APP_PID="" INSTALLED=0

as_root() {
  if [ "$(id -u)" -eq 0 ]; then "$@"; else sudo "$@"; fi
}

log() {
  echo "installer: $*"
}

fail() {
  echo "::error::installer check: $*"
  exit 1
}

cleanup() {
  local status=$?
  set +e
  for pid in "$APP_PID" "$XVFB_PID"; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null
  done
  wait 2>/dev/null
  # The probe as last written, for the failure's artifact.
  [ -f "$WORK/probe.json" ] && cp "$WORK/probe.json" "$INSTALLER_OUT/probe.json"
  # Leave no install behind on a local run.
  if [ "$INSTALLED" -eq 1 ]; then
    as_root dpkg --purge trellis >"$INSTALLER_OUT/cleanup.log" 2>&1
  fi
  rm -rf "$WORK"
  exit "$status"
}
trap cleanup EXIT

installed_version() {
  dpkg-query -W -f="\${Version}" trellis 2>/dev/null || true
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

PATHS=(
  /usr/bin/trellis
  /usr/share/applications/trellis.desktop
  /usr/share/applications/trellis-links.desktop
  /usr/share/doc/trellis/LICENSE-MIT
  /usr/share/doc/trellis/LICENSE-APACHE
  /usr/share/doc/trellis/README.md
  /usr/share/doc/trellis/THIRD-PARTY-NOTICES.md
)
for size in 16 24 32 48 64 128 256 512; do
  PATHS+=("/usr/share/icons/hicolor/${size}x${size}/apps/trellis.png")
done

assert_installed() {
  local listed
  listed="$(dpkg -L trellis)"
  for p in "${PATHS[@]}"; do
    [ -f "$p" ] || fail "$p isn't installed"
    grep -qxF "$p" <<<"$listed" || fail "dpkg -L trellis doesn't list $p"
  done
  [ -x /usr/bin/trellis ] || fail "/usr/bin/trellis isn't executable"
  desktop-file-validate /usr/share/applications/trellis.desktop /usr/share/applications/trellis-links.desktop ||
    fail "the installed desktop entries don't validate"
}

assert_removed() {
  for p in "${PATHS[@]}"; do
    [ ! -e "$p" ] || fail "$p is still there after the purge"
  done
  if installed_status | grep -q 'install ok installed'; then
    fail "trellis is still installed after the purge"
  fi
}

# Starts the installed window the way a launcher does, from the desktop entry's Exec, on its own
# Xvfb with fresh HOME and XDG folders, and waits until the probe shows a drawn frame.
window_start() {
  local exec_line prog display deadline probe app
  exec_line="$(sed -n 's/^Exec=//p' /usr/share/applications/trellis.desktop | head -n1)"
  # Field codes (%f, %U, ...) stand for files a launcher passes; there are none here.
  exec_line="$(sed -E 's/%[a-zA-Z]//g' <<<"$exec_line")"
  local -a cmd
  read -r -a cmd <<<"$exec_line"
  [ "${#cmd[@]}" -gt 0 ] || fail "trellis.desktop has no Exec"
  prog="$(command -v "${cmd[0]}" || true)"
  [ "$prog" = /usr/bin/trellis ] || fail "Exec=${cmd[0]} resolves to '$prog', not /usr/bin/trellis"

  # Never the caller's display or Wayland session.
  unset WAYLAND_DISPLAY XDG_SESSION_TYPE DISPLAY
  Xvfb -displayfd 3 -screen 0 1920x1080x24 -nolisten tcp 3>"$WORK/display" 2>"$INSTALLER_OUT/xvfb.log" &
  XVFB_PID=$!
  deadline=$((SECONDS + INSTALLER_TIMEOUT))
  until [ -s "$WORK/display" ]; do
    kill -0 "$XVFB_PID" 2>/dev/null || fail "Xvfb exited; see xvfb.log"
    [ "$SECONDS" -lt "$deadline" ] || fail "Xvfb did not report a display"
    sleep 0.1
  done
  display=":$(head -n1 "$WORK/display" | tr -d '[:space:]')"
  log "Xvfb on $display"

  app="$WORK/app"
  mkdir -p "$app/home" "$app/config" "$app/cache" "$app/data" "$app/run"
  chmod 0700 "$app/run"
  probe="$WORK/probe.json"
  env DISPLAY="$display" LIBGL_ALWAYS_SOFTWARE=1 HOME="$app/home" XDG_CONFIG_HOME="$app/config" \
    XDG_CACHE_HOME="$app/cache" XDG_DATA_HOME="$app/data" XDG_RUNTIME_DIR="$app/run" \
    TRELLIS_SMOKE_PROBE="$probe" "$prog" "${cmd[@]:1}" >"$INSTALLER_OUT/window.out" 2>"$INSTALLER_OUT/window.err" &
  APP_PID=$!
  deadline=$((SECONDS + INSTALLER_TIMEOUT))
  until [ -s "$probe" ] && jq -e '.v == 1 and .seq >= 1 and .screen[0] > 0' "$probe" >/dev/null 2>&1; do
    kill -0 "$APP_PID" 2>/dev/null || fail "the window's process exited; see window.err"
    [ "$SECONDS" -lt "$deadline" ] || fail "no drawn frame in the probe within ${INSTALLER_TIMEOUT}s"
    sleep 0.5
  done
  cp "$probe" "$INSTALLER_OUT/probe.json"
  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  APP_PID=""
  kill "$XVFB_PID" 2>/dev/null || true
  wait "$XVFB_PID" 2>/dev/null || true
  XVFB_PID=""
}

# --- Install the old version, then upgrade -------------------------------------------------
as_root dpkg -i "$old_deb" >"$INSTALLER_OUT/install-old.log" 2>&1 || fail "dpkg -i of the old package failed; see install-old.log"
INSTALLED=1
assert_version 0.0.1
log "ok install 0.0.1"
as_root dpkg -i "$deb" >"$INSTALLER_OUT/upgrade.log" 2>&1 || fail "dpkg -i of the current package failed; see upgrade.log"
assert_version "$version"
log "ok upgrade to $version"
assert_installed
log "ok files, desktop entries, icons and docs"

# --- The command-line modes, with fresh HOME and XDG folders they must leave alone ---------------
cli="$WORK/cli"
mkdir -p "$cli/home" "$cli/config" "$cli/cache" "$cli/data"
cli_env=("HOME=$cli/home" "XDG_CONFIG_HOME=$cli/config" "XDG_CACHE_HOME=$cli/cache" "XDG_DATA_HOME=$cli/data")
got="$(env "${cli_env[@]}" /usr/bin/trellis --version)" || fail "trellis --version failed"
[ "$got" = "trellis $version" ] || fail "trellis --version printed '$got'"
log "ok --version"
env "${cli_env[@]}" pwsh -NoProfile -File "$here/update-feed-check.ps1" -Trellis /usr/bin/trellis -Version "$version" \
  -Installer "$deb" -Archive "$tgz" -Expect installer -Out "$INSTALLER_OUT/feed" || fail "update-feed-check.ps1 failed"
for d in "$cli/config" "$cli/cache" "$cli/data" "$cli/home/.config"; do
  [ ! -e "$d/trellis" ] || fail "the command-line modes created $d/trellis"
done

# --- The window --------------------------------------------------------------------------------
window_start
log "ok the window drew a frame"

# --- Purge -------------------------------------------------------------------------------------
as_root dpkg --purge trellis >"$INSTALLER_OUT/purge.log" 2>&1 || fail "dpkg --purge failed; see purge.log"
INSTALLED=0
assert_removed
log "ok purge removed everything"
log "all installer checks passed"
