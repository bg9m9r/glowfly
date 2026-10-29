#!/usr/bin/env bash
# Finds the GitHub release a Release run fills, or creates it as a draft: run by the Release
# workflow's prepare job, after the owner's approval. Writes `id=<release id>` and
# `draft=<true|false>` to $GITHUB_OUTPUT (to stdout when it is unset).
#
#   bash scripts/release-draft.sh v0.3.0      # needs gh, GH_TOKEN and GH_REPO
#
# The tag must exist (release-please creates it with the draft: force-tag-creation). With no
# release for it yet (a run by hand), a draft is created. A draft from an earlier run is reused,
# so a re-run uploads to it again. A published release is reused too, for rebuilding its
# downloads, unless it is immutable: then its files can't change, and this fails. Two releases
# for one tag fail as well, since every later job finds the release by its tag.
set -Eeuo pipefail

if ! { [ "$#" -eq 1 ] && [ -n "$1" ] && [ "${1#-}" = "$1" ]; }; then
  echo "usage: release-draft.sh <tag>" >&2
  exit 2
fi
tag="$1"
: "${GH_REPO:?GH_REPO must name owner/repo}"
[[ "$tag" =~ ^v[0-9A-Za-z.+-]+$ ]] || { echo "::error::'$tag' isn't a release tag (v<version>)"; exit 2; }

gh api "repos/$GH_REPO/git/ref/tags/$tag" > /dev/null || { echo "::error::Tag $tag doesn't exist"; exit 1; }

# Every release for the tag as `<id> <draft> <immutable>`, drafts included: the job's token can
# push, so the listing has them.
releases_for_tag() {
  TAG="$tag" gh api --paginate "repos/$GH_REPO/releases?per_page=100" \
    --jq '.[] | select(.tag_name == env.TAG) | "\(.id) \(.draft) \(.immutable // false)"'
}

found="$(releases_for_tag)"
if [ -z "$found" ]; then
  id="$(gh api -X POST "repos/$GH_REPO/releases" -f tag_name="$tag" -f name="Glowfly $tag" \
    -F draft=true -F generate_release_notes=true --jq .id)"
  echo "release: created a draft for $tag (id $id)"
  found="$id true false"
fi

if [ "$(printf '%s\n' "$found" | wc -l)" -ne 1 ]; then
  echo "::error::$tag has more than one release (id draft immutable):"
  printf '%s\n' "$found"
  echo "::error::Delete the extra drafts, then run the workflow again."
  exit 1
fi

read -r id draft immutable <<< "$found"
if [ "$draft" = false ] && [ "$immutable" = true ]; then
  echo "::error::The release for $tag is published and immutable: its downloads can't be rebuilt"
  exit 1
fi
if [ "$draft" = false ]; then
  echo "::notice::The release for $tag is already published: its downloads are rebuilt in place"
fi
out="${GITHUB_OUTPUT:-/dev/stdout}"
echo "id=$id" >> "$out"
echo "draft=$draft" >> "$out"
