# shellcheck shell=bash
# Shared by the Linux package checks, installer-check-linux.sh (.deb) and installer-check-rpm.sh
# (.rpm): logging, the files a package installs, the command-line checks and starting the
# installed window on its own X server. Sourced, never run.
#
# installer_init sets INSTALLER_OUT, INSTALLER_TIMEOUT, WORK, XVFB_PID and APP_PID; the caller's
# cleanup runs installer_stop and removes $WORK.

installer_init() {
  INSTALLER_OUT="${INSTALLER_OUT:-${RUNNER_TEMP:-/tmp}/installers}"
  INSTALLER_TIMEOUT="${INSTALLER_TIMEOUT:-60}"
  mkdir -p "$INSTALLER_OUT"
  INSTALLER_OUT="$(cd "$INSTALLER_OUT" && pwd)"
  WORK="$(mktemp -d "${TMPDIR:-/tmp}/glowfly-installer.XXXXXX")"
  XVFB_PID=""
  APP_PID=""
}

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

# Stops the window and Xvfb if they still run, and keeps the probe as last written for the
# failure's artifact.
installer_stop() {
  for pid in "$APP_PID" "$XVFB_PID"; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null
  done
  wait 2>/dev/null
  [ -f "$WORK/probe.json" ] && cp "$WORK/probe.json" "$INSTALLER_OUT/probe.json"
  return 0
}

# Every file a package installs, and which none may leave behind.
PATHS=(
  /usr/bin/glowfly
  /usr/share/applications/glowfly.desktop
  /usr/share/applications/glowfly-links.desktop
  /usr/share/doc/glowfly/LICENSE-MIT
  /usr/share/doc/glowfly/LICENSE-APACHE
  /usr/share/doc/glowfly/README.md
  /usr/share/doc/glowfly/THIRD-PARTY-NOTICES.md
  /usr/share/glowfly/package-format
)
for size in 16 24 32 48 64 128 256 512; do
  PATHS+=("/usr/share/icons/hicolor/${size}x${size}/apps/glowfly.png")
done

# Fails unless every path is installed and in <listing> (the package manager's file list), the
# program runs, the desktop entries validate and package-format reads <format>.
assert_installed_files() {
  local listing="$1" format="$2" p
  for p in "${PATHS[@]}"; do
    [ -f "$p" ] || fail "$p isn't installed"
    grep -qxF "$p" <<<"$listing" || fail "the package's file list doesn't have $p"
  done
  [ -x /usr/bin/glowfly ] || fail "/usr/bin/glowfly isn't executable"
  desktop-file-validate /usr/share/applications/glowfly.desktop /usr/share/applications/glowfly-links.desktop ||
    fail "the installed desktop entries don't validate"
  [ "$(cat /usr/share/glowfly/package-format)" = "$format" ] || fail "package-format doesn't read $format"
}

assert_paths_removed() {
  local p
  for p in "${PATHS[@]}"; do
    [ ! -e "$p" ] || fail "$p is still there after the removal"
  done
}

# Fresh HOME and XDG folders for the command-line modes, which must leave them alone: CLI_ENV.
cli_prepare() {
  local cli="$WORK/cli"
  mkdir -p "$cli/home" "$cli/config" "$cli/cache" "$cli/data"
  CLI_ENV=("HOME=$cli/home" "XDG_CONFIG_HOME=$cli/config" "XDG_CACHE_HOME=$cli/cache" "XDG_DATA_HOME=$cli/data")
}

# `<program> --version` prints `glowfly <version>`.
cli_version() {
  local got
  got="$(env "${CLI_ENV[@]}" "$1" --version)" || fail "glowfly --version failed"
  [ "$got" = "glowfly $2" ] || fail "glowfly --version printed '$got'"
  log "ok --version"
}

# `<program> --check-update` against an empty feed folder exits 3 (nothing newer) and names the
# install kind <kind>.
cli_install_kind() {
  local feed dl out status=0
  feed="$(mktemp -d "$WORK/feed.XXXXXX")"
  dl="$(mktemp -d "$WORK/dl.XXXXXX")"
  out="$(env "${CLI_ENV[@]}" GLOWFLY_UPDATE_FEED="$feed" "$1" --check-update "$dl")" || status=$?
  printf '%s\n' "$out" >"$INSTALLER_OUT/check-update.out"
  [ "$status" -eq 3 ] || fail "--check-update exited $status, expected 3; see check-update.out"
  grep -qxF "install $2" <<<"$out" || fail "--check-update didn't print 'install $2'; see check-update.out"
  log "ok --check-update finds an $2 copy"
}

assert_cli_left_nothing() {
  local cli="$WORK/cli" d
  for d in "$cli/config" "$cli/cache" "$cli/data" "$cli/home/.config"; do
    [ ! -e "$d/glowfly" ] || fail "the command-line modes created $d/glowfly"
  done
}

# Starts the installed window the way a launcher does, from the desktop entry's Exec, on its own
# Xvfb with fresh HOME and XDG folders, and waits until the probe shows a drawn frame. The Exec
# must resolve to <program>.
window_start() {
  local want="$1" exec_line prog display deadline probe app
  exec_line="$(sed -n 's/^Exec=//p' /usr/share/applications/glowfly.desktop | head -n1)"
  # Field codes (%f, %U, ...) stand for files a launcher passes; there are none here.
  exec_line="$(sed -E 's/%[a-zA-Z]//g' <<<"$exec_line")"
  local -a cmd
  read -r -a cmd <<<"$exec_line"
  [ "${#cmd[@]}" -gt 0 ] || fail "glowfly.desktop has no Exec"
  prog="$(command -v "${cmd[0]}" || true)"
  # A merged /usr (Fedora) finds the same file through /usr/sbin too: compare the real paths.
  if [ -z "$prog" ] || [ "$(realpath -e -- "$prog")" != "$(realpath -e -- "$want")" ]; then
    fail "Exec=${cmd[0]} resolves to '$prog', not $want"
  fi

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
    GLOWFLY_SMOKE_PROBE="$probe" "$prog" "${cmd[@]:1}" >"$INSTALLER_OUT/window.out" 2>"$INSTALLER_OUT/window.err" &
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
