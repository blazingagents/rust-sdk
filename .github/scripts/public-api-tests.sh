#!/usr/bin/env bash
# Fails when a file under src/ or any file the library compiles holds a test,
# printing each file and line. Tests live in tests/ and use the public API.
# ci.yml runs it; run it locally the same way:
#   .github/scripts/public-api-tests.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"

message='Tests must use the public API. Either rewrite this test against the public client, or, if the test really needs this value, the SDK is missing a public way to get it: add one to the public API (with docs) and use that.'

deps=$(mktemp)
trap 'rm -f "$deps"' EXIT
cargo rustc --quiet -p blazing-agents --lib --all-features -- --emit=dep-info="$deps"
# Bash 3.2 and BSD sed (macOS) lack mapfile and GNU sed's \+ and \n, so read with while and split with tr.
compiled=()
while IFS= read -r file; do
  compiled+=("$file")
done < <(sed -n '1{s/^[^:]*: *//;p;}' "$deps" | tr -s ' ' '\n' | grep '\.rs$')
if [[ ! " ${compiled[*]-} " =~ " src/lib.rs " ]]; then
  echo "::error::the library's dep-info does not list src/lib.rs: ${compiled[*]-}" >&2
  exit 1
fi
# src/ files that this platform's build skips (a cfg(windows) module) count too.
sources=()
while IFS= read -r file; do
  sources+=("$file")
done < <({ printf '%s\n' "${compiled[@]}"; find src -name '*.rs'; } | sort -u)

# Matches #[cfg(..test..)], #[cfg_attr(..test..)] and cfg!(..test..) even when
# rustfmt wraps them, and test attributes such as #[test], #[tokio::test] and
# #[::core::prelude::v1::test]. Line comments are blanked first.
if perl -0777 -ne '
  s{//[^\n]*}{}g;
  while (/#!?\[\s*cfg(?:_attr)?\s*(\((?:[^()]++|(?1))*\))|\bcfg!\s*(\((?:[^()]++|(?2))*\))|#!?\[\s*(?:::)?(?:\w+\s*::\s*)*test\s*[\](]/g) {
    my ($at, $args) = ($-[0], $1 // $2);
    next if defined $args && $args !~ /\btest\b/;
    my $line = 1 + (substr($_, 0, $at) =~ tr/\n//);
    my ($text) = substr($_, $at) =~ /^([^\n]*)/;
    print "$ARGV:$line:$text\n";
    $found = 1;
  }
  END { exit($found ? 0 : 1) }
' "${sources[@]}"; then
  echo "$message"
  exit 1
fi

# A test the grep cannot see, such as a macro that emits #[$meta] or an
# aliased test attribute, still shows up in the library's test harness.
listed=$(cargo test --quiet -p blazing-agents --lib --all-features -- --list --format terse)
if grep -E ': (test|bench)$' <<<"$listed"; then
  echo "$message"
  exit 1
fi
