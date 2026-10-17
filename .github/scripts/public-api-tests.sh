#!/usr/bin/env bash
# Fails when a file the library compiles holds a test, printing each file and
# line. Tests live in tests/ and use the public API. ci.yml runs it; run it
# locally the same way:
#   .github/scripts/public-api-tests.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"

deps=$(mktemp)
trap 'rm -f "$deps"' EXIT
cargo rustc --quiet -p blazing-agents --lib --all-features -- --emit=dep-info="$deps"
mapfile -t sources < <(sed -n '1{s/^[^:]*: *//;s/ \+/\n/g;p}' "$deps" | grep '\.rs$')
if [[ ! " ${sources[*]} " =~ " src/lib.rs " ]]; then
  echo "::error::the library's dep-info does not list src/lib.rs: ${sources[*]}" >&2
  exit 1
fi

test_attribute='#!?\[[[:space:]]*(cfg(_attr)?[[:space:]]*\(.*\btest\b|([A-Za-z_][A-Za-z0-9_]*::)*test[[:space:]]*[](])'
if grep -nHE "$test_attribute" "${sources[@]}" | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//'; then
  echo 'Tests must use the public API. Either rewrite this test against the public client, or, if the test really needs this value, the SDK is missing a public way to get it: add one to the public API (with docs) and use that.'
  exit 1
fi
