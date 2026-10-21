#!/usr/bin/env bash
# Real-input smoke tests for the Linux build. Starts its own X server (Xvfb, on a free display
# it picks with -displayfd) with the openbox window manager, never the caller's display, and
# drives Glowfly there only with real X11 input through xdotool (XTEST): a click that focuses
# the window, typing in the commit box and committing, a context menu, a keyboard shortcut, a
# drag of the sidebar's edge and a window resize. It runs one full pass per scale factor in
# SMOKE_SCALES, each on a fresh fixture repository, config and HOME. The app reports what it
# shows through its opt-in probe file (GLOWFLY_SMOKE_PROBE, crates/glowfly-app/src/ui/probe.rs).
#
# Usage: scripts/smoke-linux.sh [glowfly binary]   (default: $CARGO_TARGET_DIR/release/glowfly)
# Environment:
#   SMOKE_OUT          screenshots and, on failure, logs (default: $CARGO_TARGET_DIR/smoke)
#   SMOKE_SCALES       scale factors to run (default: "1 1.5")
#   SMOKE_TIMEOUT      seconds any one wait may take (default: 20)
#   SMOKE_EXPECT_FAIL  check ids whose failure is only a warning (default: none)
set -Eeuo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$here/.." && pwd)"

# shellcheck source=scripts/smoke/lib.sh
source "$here/smoke/lib.sh"
# shellcheck source=scripts/smoke/pass.sh
source "$here/smoke/pass.sh"

if [ "$(uname -s)" != Linux ]; then
  echo "smoke-linux.sh runs only on Linux (it starts its own X server)" >&2
  exit 2
fi

target="${CARGO_TARGET_DIR:-target}"
case "$target" in /*) ;; *) target="$ROOT/$target" ;; esac
BIN="${1:-$target/release/glowfly}"
case "$BIN" in /*) ;; *) BIN="$PWD/$BIN" ;; esac
SMOKE_OUT="${SMOKE_OUT:-$target/smoke}"
case "$SMOKE_OUT" in /*) ;; *) SMOKE_OUT="$PWD/$SMOKE_OUT" ;; esac
SMOKE_SCALES="${SMOKE_SCALES:-1 1.5}"
SMOKE_TIMEOUT="${SMOKE_TIMEOUT:-20}"
SMOKE_EXPECT_FAIL="${SMOKE_EXPECT_FAIL:-}"
export SMOKE_OUT SMOKE_TIMEOUT SMOKE_EXPECT_FAIL

missing=()
for tool in Xvfb openbox xdotool xmessage xprop import jq git timeout awk; do
  command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
done
if [ "${#missing[@]}" -gt 0 ]; then
  echo "missing tools: ${missing[*]} (Ubuntu: xvfb openbox xdotool x11-utils imagemagick jq)" >&2
  exit 2
fi
if [ ! -x "$BIN" ]; then
  echo "no executable Glowfly at $BIN (build it with cargo build --release -p glowfly)" >&2
  exit 2
fi

# The original stdout, for GitHub annotations printed from inside command substitutions.
exec 9>&1

# Only this script's own results are cleared: SMOKE_OUT may be a folder holding other files.
mkdir -p "$SMOKE_OUT"
rm -rf "$SMOKE_OUT"/scale-* "$SMOKE_OUT/work" "$SMOKE_OUT/xvfb.log" "$SMOKE_OUT/openbox.log"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/glowfly-smoke.XXXXXX")"
XVFB_PID="" WM_PID="" APP_PID="" THIEF_PID="" WID="" PASS="setup" PASS_OUT="$SMOKE_OUT" CHECK="setup"
PROBE="$WORK/none.json"

cleanup() {
  local status=$?
  set +e
  trap - ERR
  for pid in "$APP_PID" "$THIEF_PID" "$WM_PID" "$XVFB_PID"; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null
  done
  wait 2>/dev/null
  if [ "$status" -ne 0 ]; then
    # Kept for the artifact, minus the runtime folders (sockets).
    rm -rf "$WORK"/*/run
    mv "$WORK" "$SMOKE_OUT/work" 2>/dev/null
    echo "smoke: failed; screenshots and logs in $SMOKE_OUT" >&2
  else
    rm -rf "$WORK"
  fi
  exit "$status"
}
trap cleanup EXIT
arm_err_trap

# Never the caller's display or Wayland session (winit would pick WSLg's Wayland, say).
unset WAYLAND_DISPLAY XDG_SESSION_TYPE DISPLAY
export LIBGL_ALWAYS_SOFTWARE=1 XMODIFIERS=@im=none

Xvfb -displayfd 3 -screen 0 2560x1600x24 -nolisten tcp -noreset 3>"$WORK/display" 2>"$SMOKE_OUT/xvfb.log" &
XVFB_PID=$!
deadline=$((SECONDS + SMOKE_TIMEOUT))
until [ -s "$WORK/display" ]; do
  kill -0 "$XVFB_PID" 2>/dev/null || fail setup "Xvfb exited; see xvfb.log"
  [ "$SECONDS" -lt "$deadline" ] || fail setup "Xvfb did not report a display"
  sleep 0.1
done
DISPLAY=":$(head -n1 "$WORK/display" | tr -d '[:space:]')"
export DISPLAY
log "Xvfb on $DISPLAY"

openbox --sm-disable >"$SMOKE_OUT/openbox.log" 2>&1 &
WM_PID=$!
deadline=$((SECONDS + SMOKE_TIMEOUT))
until xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -q 'window id'; do
  kill -0 "$WM_PID" 2>/dev/null || fail setup "openbox exited; see openbox.log"
  [ "$SECONDS" -lt "$deadline" ] || fail setup "openbox did not start"
  sleep 0.1
done
log "openbox running"

if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]; then
  log "note: no D-Bus session bus (CI runs this under dbus-run-session); the keyring and screen-reader bridge stay off"
fi

passed=()
for scale in $SMOKE_SCALES; do
  run_pass "$scale"
  passed+=("scale $scale")
done
log "all checks passed: ${passed[*]}"
exit 0
