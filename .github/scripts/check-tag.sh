#!/usr/bin/env bash
# Fails unless the release tag (e.g. v0.9.0) names the ad-app crate version.
set -euo pipefail
tag="${1:?usage: check-tag.sh <tag>}"
pkgid="$(cargo pkgid -p ad-app)"
version="${pkgid##*[#@]}"
if [ "$tag" != "v$version" ]; then
  echo "Tag $tag does not match the crate version $version (expected v$version)." >&2
  exit 1
fi
echo "Tag $tag matches the crate version."
