#!/usr/bin/env bash
# figcheck self-test: for every backend, detect the toolchain and, when
# present, compile/execute a tiny known-good fixture (must succeed) and a
# tiny known-bad fixture (must fail). Proves the harness itself -- not any
# real figure export -- correctly recognizes both outcomes.
#
# Exit status: 0 if every available backend behaved as expected (skips do
# not count as failure); non-zero if any backend mis-detected, the good
# fixture failed to compile, or the bad fixture unexpectedly succeeded.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

declare -a rows=()
failed=0

# backend name : fixture extension
backends="typst:typ tikz:tex matplotlib:py ggplot2:R makie:jl vega:vega.json"

for entry in $backends; do
  backend="${entry%%:*}"
  ext="${entry#*:}"
  good="fixtures/$backend/good.$ext"
  bad="fixtures/$backend/bad.$ext"

  if ! ./check.sh "$backend" detect >/dev/null 2>&1; then
    rows+=("SKIP  $backend  (toolchain not available on this machine)")
    continue
  fi

  outdir="$(mktemp -d "${TMPDIR:-/tmp}/figcheck-selftest-$backend.XXXXXX")"

  if ./check.sh "$backend" run "$good" "$outdir/good" >"$outdir/good.log" 2>&1; then
    good_ok=1
  else
    good_ok=0
  fi

  if ./check.sh "$backend" run "$bad" "$outdir/bad" >"$outdir/bad.log" 2>&1; then
    bad_ok=1
  else
    bad_ok=0
  fi

  if [ "$good_ok" -eq 1 ] && [ "$bad_ok" -eq 0 ]; then
    rows+=("PASS  $backend  (good compiled, bad failed as expected)")
  elif [ "$good_ok" -eq 0 ]; then
    rows+=("FAIL  $backend  (known-good fixture did not compile -- see $outdir/good.log)")
    failed=1
  else
    rows+=("FAIL  $backend  (known-bad fixture compiled cleanly -- harness would not catch a broken figure; see $outdir/bad.log)")
    failed=1
  fi
done

echo "figcheck self-test results:"
for row in "${rows[@]}"; do
  echo "  $row"
done

if [ "$failed" -ne 0 ]; then
  echo "figcheck self-test: FAILED" >&2
  exit 1
fi

echo "figcheck self-test: OK (skips are expected on machines missing a toolchain)"
