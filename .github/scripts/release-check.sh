#!/usr/bin/env bash
# Checks a release tag against Cargo.toml and CHANGELOG.md, then writes the
# matching CHANGELOG entry to <notes-file>. Prints version= and prerelease=
# lines for $GITHUB_OUTPUT. publish.yml runs it; run it locally the same way:
#   .github/scripts/release-check.sh v0.1.0 /tmp/notes.md
set -euo pipefail

usage='usage: release-check.sh <tag> <notes-file>'
tag=${1:?$usage}
notes=${2:?$usage}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

if [[ ! $tag =~ ^v([0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?)$ ]]; then
  echo "::error::tag $tag is not vX.Y.Z or vX.Y.Z-<prerelease>" >&2
  exit 1
fi
version=${BASH_REMATCH[1]}

crate_version=$(cargo metadata --no-deps --format-version 1 --manifest-path "$root/Cargo.toml" \
  | jq -r '.packages[] | select(.name == "blazing-agents") | .version')
if [[ $version != "$crate_version" ]]; then
  echo "::error::tag $tag does not match Cargo.toml version v$crate_version" >&2
  exit 1
fi

# The entry is everything between "## X.Y.Z" (optionally followed by a date)
# and the next "## " heading outside a fenced code block, without leading or
# trailing blank lines.
awk -v heading="## $version" '
  /^ {0,3}(```|~~~)/ { fenced = !fenced }
  !fenced && ($0 == heading || index($0, heading " ") == 1) { inside = 1; next }
  inside && !fenced && /^## / { exit }
  inside && /^[[:space:]]*$/ { if (started) blanks++; next }
  inside { for (; blanks > 0; blanks--) print ""; print; started = 1 }
' "$root/CHANGELOG.md" > "$notes"
if [[ ! -s $notes ]]; then
  echo "::error::CHANGELOG.md has no \"## $version\" entry with release notes" >&2
  exit 1
fi

echo "version=$version"
if [[ $version == *-* ]]; then echo "prerelease=true"; else echo "prerelease=false"; fi
