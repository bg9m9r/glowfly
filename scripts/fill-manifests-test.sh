#!/usr/bin/env bash
# Tests scripts/fill-manifests.sh on a fixture SHA256SUMS: a release gets the Homebrew cask, the
# Scoop manifest and the three winget manifests, each with the right file's SHA-256 and no
# placeholder left; a pre-release gets none; a file missing from SHA256SUMS or a bad owner/repo
# fails. Run by the Installers workflow; needs bash, coreutils, sed, grep and awk, and checks the
# Scoop manifest is valid JSON with jq when jq is there.
#
#   bash scripts/fill-manifests-test.sh
set -Eeuo pipefail
script="$(cd "$(dirname "$0")" && pwd)/fill-manifests.sh"
work="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/glowfly-manifests-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
cd "$work"

tag=v0.3.0
repo=bg9m9r/glowfly
files=(
  "glowfly-$tag-aarch64-apple-darwin.dmg"
  "glowfly-$tag-x86_64-apple-darwin.dmg"
  "glowfly-$tag-x86_64-pc-windows-msvc.msi"
  "glowfly-$tag-aarch64-pc-windows-msvc.msi"
  "glowfly-$tag-x86_64-pc-windows-msvc.zip"
  "glowfly-$tag-aarch64-pc-windows-msvc.zip"
  "glowfly-$tag-x86_64-unknown-linux-gnu.tar.gz"
)
# A distinct, made-up SHA-256 for each file: its index repeated.
declare -A hash
: > SHA256SUMS
for i in "${!files[@]}"; do
  hash[${files[$i]}]="$(printf '%064d' 0 | tr 0 "$i")"
  printf '%s  %s\n' "${hash[${files[$i]}]}" "${files[$i]}" >> SHA256SUMS
done
failures=0
fail() {
  echo "FAIL $1"
  failures=$((failures + 1))
}

# <name> <want exit> <want text, or empty> <args...>
case_run() {
  local name="$1" want="$2" text="$3"
  shift 3
  local code=0
  out="$(bash "$script" "$@" 2>&1)" || code=$?
  if [ "$code" != "$want" ] || { [ -n "$text" ] && ! grep -qF -- "$text" <<< "$out"; }; then
    fail "$name: exit $code (want $want), output:"
    printf '%s\n' "$out" | sed 's/^/  /'
    return 1
  fi
  echo "ok   $name"
}

# <manifest> <text it must hold>
has() {
  if grep -qF -- "$2" "out/$1"; then echo "ok   $1 has $2"; else fail "$1 lacks $2"; fi
}

if case_run "a release" 0 "wrote out/glowfly.json" "$tag" SHA256SUMS out "$repo"; then
  written="$(cd out && find . -type f | sort | tr '\n' ' ')"
  want="./Glowfly.Glowfly.installer.yaml ./Glowfly.Glowfly.locale.en-US.yaml ./Glowfly.Glowfly.yaml ./glowfly.json ./glowfly.rb "
  if [ "$written" = "$want" ]; then echo "ok   five manifests"; else fail "wrote: $written"; fi
  if grep -n '@[A-Z0-9_]*@' out/*; then fail "a placeholder is left"; else echo "ok   no placeholder left"; fi
  has glowfly.rb "version \"0.3.0\""
  has glowfly.rb "arm:   \"${hash[glowfly-$tag-aarch64-apple-darwin.dmg]}\""
  has glowfly.rb "intel: \"${hash[glowfly-$tag-x86_64-apple-darwin.dmg]}\""
  has glowfly.json "\"version\": \"0.3.0\""
  for triple in x86_64-pc-windows-msvc aarch64-pc-windows-msvc; do
    zip="glowfly-$tag-$triple.zip"
    # Each url is followed by its own zip's hash, and the zip's top folder is extracted.
    if grep -A2 -F "\"url\": \"https://github.com/$repo/releases/download/$tag/$zip\"" out/glowfly.json \
      | grep -qF "\"hash\": \"${hash[$zip]}\""; then
      echo "ok   glowfly.json names $zip with its hash"
    else
      fail "glowfly.json lacks $zip with its hash"
    fi
    has glowfly.json "\"extract_dir\": \"glowfly-$tag-$triple\""
    has Glowfly.Glowfly.installer.yaml "InstallerSha256: \"${hash[glowfly-$tag-$triple.msi]}\""
  done
  has glowfly.json "\"url\": \"https://github.com/$repo/releases/download/v\$version/SHA256SUMS\""
  if command -v jq > /dev/null; then
    if jq -e '.architecture["64bit"].hash and .architecture.arm64.hash and .autoupdate.hash.url' out/glowfly.json > /dev/null; then
      echo "ok   glowfly.json is valid JSON"
    else
      fail "glowfly.json isn't valid JSON or lacks a hash"
    fi
  else
    echo "skip glowfly.json JSON check (no jq)"
  fi
fi

if case_run "a pre-release" 0 "no manifests for pre-release v0.3.0-rc.1" v0.3.0-rc.1 SHA256SUMS pre "$repo"; then
  if [ -e pre ]; then fail "a pre-release wrote manifests"; else echo "ok   a pre-release writes nothing"; fi
fi

printf '%s *%s\n' "${hash[${files[0]}]}" "${files[0]}" > star
tail -n +2 SHA256SUMS >> star
case_run "*name (binary mode) lines" 0 "wrote star-out/glowfly.rb" "$tag" star star-out "$repo" || true

for gone in "glowfly-$tag-aarch64-pc-windows-msvc.zip" "glowfly-$tag-x86_64-pc-windows-msvc.zip"; do
  grep -vF "$gone" SHA256SUMS > missing
  case_run "no checksum for $gone" 1 "::error::SHA256SUMS has no checksum for $gone" "$tag" missing missing-out "$repo" || true
done
case_run "a bad owner/repo" 2 "isn't owner/repo" "$tag" SHA256SUMS bad-out "not a repo" || true
case_run "no SHA256SUMS" 2 "::error::no file at nowhere" "$tag" nowhere none-out "$repo" || true

if [ "$failures" -ne 0 ]; then
  echo "$failures case(s) failed"
  exit 1
fi
echo "fill-manifests.sh: every case passed"
