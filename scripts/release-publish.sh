#!/usr/bin/env bash
# Publishes a Release run's draft once every file is on it: run by the Release workflow's
# publish job, after the build, flatpak, checksums and manifests jobs. With release immutability
# on, a published release's files and tag can't change, so it is checked first: SHA256SUMS and
# the provenance bundle are attached, and so is every file SHA256SUMS lists, with the same
# SHA-256 where GitHub reports the attached file's digest.
#
#   bash scripts/release-publish.sh v0.3.0 <release id>    # needs gh, GH_TOKEN and GH_REPO
#   bash scripts/release-publish.sh --check v0.3.0 <SHA256SUMS> <assets file>
#
# --check runs only the check and changes nothing; <assets file> has one `<name> <digest>` line
# per attached file, the digest `sha256:<hex>` or `-` when GitHub gave none.
#
# A full release (vX.Y.Z) is published as the latest release. A pre-release (v0.3.0-rc.1) is
# published as a pre-release and never as the latest, so the app's update check, which reads
# the latest release, doesn't offer it. An already published release is left as it is.
set -Eeuo pipefail

usage() {
  echo "usage: release-publish.sh <tag> <release id> | --check <tag> <SHA256SUMS> <assets file>" >&2
  exit 2
}

# Fails unless the release's files (<assets>) hold SHA256SUMS (<sums>), the provenance bundle and
# every file SHA256SUMS lists, each with the listed SHA-256 when its digest is known.
check() {
  local tag="$1" sums="$2" assets="$3"
  [ -f "$sums" ] && [ -f "$assets" ] || usage
  awk -v bundle="glowfly-$tag.intoto.jsonl" '
    FNR == NR { if (NF >= 1) digest[$1] = (NF >= 2 ? tolower($2) : "-"); next }
    {
      sub(/\r$/, "")
      if ($0 == "") next
      name = $2; sub(/^\*/, "", name)
      if (length($1) != 64 || $1 !~ /^[0-9a-fA-F]+$/ || name == "") { print "::error::SHA256SUMS line " FNR " is not <sha256>  <file>: " $0; bad = 1; next }
      listed++
      if (!(name in digest)) { print "::error::" name " is in SHA256SUMS but not attached"; bad = 1; next }
      if (digest[name] != "-" && digest[name] != "sha256:" tolower($1)) {
        print "::error::" name " on the release has digest " digest[name] ", SHA256SUMS lists " tolower($1); bad = 1
      }
    }
    END {
      if (!listed) { print "::error::SHA256SUMS lists no files"; bad = 1 }
      if (!("SHA256SUMS" in digest)) { print "::error::SHA256SUMS is not attached"; bad = 1 }
      if (!(bundle in digest)) { print "::error::" bundle " is not attached"; bad = 1 }
      if (bad) exit 1
      print "release files: " listed " listed in SHA256SUMS, all attached"
    }
  ' "$assets" "$sums"
}

case "${1:-}" in
  --check)
    [ "$#" -eq 4 ] || usage
    check "$2" "$3" "$4"
    exit
    ;;
  "" | -*) usage ;;
esac
[ "$#" -eq 2 ] || usage
tag="$1"
id="$2"
[[ "$id" =~ ^[0-9]+$ ]] || usage
[[ "$tag" =~ ^v[0-9A-Za-z.+-]+$ ]] || { echo "::error::'$tag' isn't a release tag (v<version>)"; exit 2; }
: "${GH_REPO:?GH_REPO must name owner/repo}"

state="$(gh api "repos/$GH_REPO/releases/$id" --jq '"\(.draft) \(.tag_name)"')"
case "$state" in
  "true $tag") ;;
  "false $tag")
    echo "::notice::The release for $tag is already published"
    exit 0
    ;;
  *) echo "::error::Release $id is '$state', not the draft for $tag"; exit 1 ;;
esac
gh api "repos/$GH_REPO/git/ref/tags/$tag" > /dev/null || { echo "::error::Tag $tag doesn't exist"; exit 1; }

work="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/glowfly-publish.XXXXXX")"
trap 'rm -rf "$work"' EXIT
gh api --paginate "repos/$GH_REPO/releases/$id/assets?per_page=100" --jq '.[] | "\(.name) \(.digest // "-")"' > "$work/assets"
gh release download "$tag" --pattern SHA256SUMS --dir "$work/sums"
check "$tag" "$work/sums/SHA256SUMS" "$work/assets"

if [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  prerelease=false latest=true
else
  prerelease=true latest=false
fi
published="$(gh api -X PATCH "repos/$GH_REPO/releases/$id" -F draft=false -F prerelease="$prerelease" \
  -f make_latest="$latest" --jq '"\(.draft) \(.prerelease) \(.html_url)"')"
read -r draft is_pre url <<< "$published"
[ "$draft" = false ] && [ "$is_pre" = "$prerelease" ] || { echo "::error::Publishing $tag returned '$published'"; exit 1; }
echo "release: published $tag (pre-release: $prerelease, latest: $latest) at $url"
