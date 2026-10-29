#!/usr/bin/env bash
# Puts the fixed header (packaging/release-notes-header.md: unsigned builds, checking a download
# against SHA256SUMS, no warranty and the licences) at the top of a GitHub release's notes, above
# what release-please or GitHub wrote. Run by the Release workflow's prepare job once the release
# exists. A header already there, between the file's start and end markers, is replaced, so a
# re-run leaves one.
#
#   bash scripts/release-notes-header.sh v0.3.0      # needs gh, GH_TOKEN and GH_REPO
#   bash scripts/release-notes-header.sh --print <notes file>   # prints the result, changes nothing
#
# The app's release notes dialog (changelog::parse_body) skips the header: it has no `## `
# heading and no list items (crates/glowfly-core/src/changelog_tests.rs checks).
set -Eeuo pipefail
header="$(cd "$(dirname "$0")/.." && pwd)/packaging/release-notes-header.md"

usage() {
  echo "usage: release-notes-header.sh <tag> | --print <notes file>" >&2
  exit 2
}

start="$(head -n1 "$header" | tr -d '[:cntrl:]')"
end="$(tail -n1 "$header" | tr -d '[:cntrl:]')"
case "$start$end" in
  "<!-- glowfly-release-header:start --><!-- glowfly-release-header:end -->") ;;
  *) echo "$header must start and end with its markers" >&2; exit 1 ;;
esac

# The header, a blank line, then <notes> without an earlier header or the blank lines after it.
compose() {
  cat "$header"
  echo
  awk -v start="$start" -v end="$end" '
    { line = $0; sub(/[[:cntrl:]]+$/, "", line) }
    line == start { skip = 1; next }
    skip && line == end { skip = 0; next }
    skip { next }
    !seen && line == "" { next }
    { seen = 1; print }
  ' "$1"
}

case "${1:-}" in
  --print)
    [ "$#" -eq 2 ] || usage
    compose "$2"
    ;;
  "" | -*) usage ;;
  *)
    [ "$#" -eq 1 ] || usage
    tag="$1"
    work="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/glowfly-notes.XXXXXX")"
    trap 'rm -rf "$work"' EXIT
    gh release view "$tag" --json body --jq .body > "$work/notes.md"
    compose "$work/notes.md" > "$work/new.md"
    gh release edit "$tag" --notes-file "$work/new.md" > /dev/null
    echo "release notes header: put on $tag"
    ;;
esac
