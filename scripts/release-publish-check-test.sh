#!/usr/bin/env bash
# Tests scripts/release-publish.sh's offline parts on small fixtures: the check that a draft holds
# every file before it is published (--check), and which full releases become the latest
# (--latest). Each case that should fail must fail with its own message. Run by the Installers
# workflow; needs only bash, coreutils and awk.
#
#   bash scripts/release-publish-check-test.sh
set -Eeuo pipefail
script="$(cd "$(dirname "$0")" && pwd)/release-publish.sh"
work="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/glowfly-publish-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cd "$work"

tag=v0.3.0
bundle="glowfly-$tag.intoto.jsonl"
printf 'zip\n' > a.zip
printf 'msi\n' > b.msi
sha256sum a.zip b.msi > SHA256SUMS
ha="$(sha256sum a.zip | cut -d' ' -f1)"
hb="$(sha256sum b.msi | cut -d' ' -f1)"
failures=0

# <name> <want exit> <want text, or empty> <sums file> <assets lines...>
case_check() {
  local name="$1" want="$2" text="$3" sums="$4"
  shift 4
  printf '%s\n' "$@" > assets
  local out code=0
  out="$(bash "$script" --check "$tag" "$sums" assets 2>&1)" || code=$?
  if [ "$code" != "$want" ] || { [ -n "$text" ] && ! grep -qF -- "$text" <<< "$out"; }; then
    echo "FAIL $name: exit $code (want $want), output:"
    printf '%s\n' "$out" | sed 's/^/  /'
    failures=$((failures + 1))
  else
    echo "ok   $name"
  fi
}

attached=("a.zip sha256:$ha 1" "b.msi sha256:$hb 2" "SHA256SUMS - 3" "$bundle - 4")
case_check "every file attached with its digest" 0 "2 listed in SHA256SUMS, all attached" SHA256SUMS "${attached[@]}"
case_check "a listed file not attached" 1 "b.msi is in SHA256SUMS but not attached" SHA256SUMS \
  "a.zip sha256:$ha" "SHA256SUMS -" "$bundle -"
case_check "a digest that doesn't match" 1 "a.zip on the release has digest sha256:$hb, SHA256SUMS lists $ha" SHA256SUMS \
  "a.zip sha256:$hb" "b.msi sha256:$hb" "SHA256SUMS -" "$bundle -"
case_check "an upper-case digest matches" 0 "all attached" SHA256SUMS \
  "a.zip SHA256:${ha^^}" "b.msi sha256:$hb" "SHA256SUMS -" "$bundle -"
case_check "a - digest (none given) is only checked for presence" 0 "all attached" SHA256SUMS \
  "a.zip -" "b.msi -" "SHA256SUMS -" "$bundle -"
case_check "SHA256SUMS not attached" 1 "::error::SHA256SUMS is not attached" SHA256SUMS \
  "a.zip sha256:$ha" "b.msi sha256:$hb" "$bundle -"
case_check "the provenance bundle not attached" 1 "::error::$bundle is not attached" SHA256SUMS \
  "a.zip sha256:$ha" "b.msi sha256:$hb" "SHA256SUMS -"
case_check "another tag's bundle doesn't count" 1 "::error::$bundle is not attached" SHA256SUMS \
  "a.zip sha256:$ha" "b.msi sha256:$hb" "SHA256SUMS -" "glowfly-v0.2.0.intoto.jsonl -"
case_check "no files attached" 1 "::error::The release has no files attached" SHA256SUMS ""

printf '%s *a.zip\n%s *b.msi\n' "$ha" "$hb" > star
case_check "*name (binary mode) lines" 0 "2 listed in SHA256SUMS, all attached" star "${attached[@]}"
printf '%s  a.zip\r\n%s  b.msi\r\n' "$ha" "$hb" > crlf
case_check "CRLF lines" 0 "2 listed in SHA256SUMS, all attached" crlf "${attached[@]}"
printf 'a.zip sha256:%s\r\nb.msi sha256:%s\r\nSHA256SUMS -\r\n%s -\r\n' "$ha" "$hb" "$bundle" > crlf-assets
if out="$(bash "$script" --check "$tag" SHA256SUMS crlf-assets 2>&1)" && grep -qF "all attached" <<< "$out"; then
  echo "ok   CRLF asset lines"
else
  echo "FAIL CRLF asset lines: $out"
  failures=$((failures + 1))
fi
: > empty
case_check "an empty SHA256SUMS" 1 "::error::SHA256SUMS lists no files" empty "${attached[@]}"
printf 'not a checksum line\n' > garbage
case_check "a malformed SHA256SUMS line" 1 "SHA256SUMS line 1 is not <sha256>  <file>" garbage "${attached[@]}"

# <tag> <newest published full release, or -> <want>
case_latest() {
  local got
  got="$(bash "$script" --latest "$1" "$2")"
  if [ "$got" = "$3" ]; then echo "ok   latest $1 over $2: $got"; else
    echo "FAIL latest $1 over $2: $got (want $3)"
    failures=$((failures + 1))
  fi
}
case_latest v0.3.0 - true
case_latest v0.3.0 v0.2.9 true
case_latest v0.10.0 v0.9.0 true
case_latest v0.3.1 v0.4.0 false
case_latest v0.4.0 v0.4.0 false
case_latest v0.3.0-rc.1 - false
case_latest v0.3.0-rc.1 v0.2.0 false

code=0
bash "$script" --check "$tag" missing-sums assets > /dev/null 2>&1 || code=$?
if [ "$code" = 2 ]; then
  echo "ok   a missing file is a usage error"
else
  echo "FAIL missing file: exit $code"
  failures=$((failures + 1))
fi

if [ "$failures" -ne 0 ]; then
  echo "$failures case(s) failed"
  exit 1
fi
echo "release-publish.sh: every case passed"
