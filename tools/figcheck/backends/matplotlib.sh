#!/usr/bin/env bash
# matplotlib backend runner: executes a generated Python script that renders
# a PNG. Runs in an ephemeral uv environment (no persistent install).
#
#   matplotlib.sh detect
#   matplotlib.sh run <input.py> [outdir]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
. lib/common.sh

cmd="${1:-}"
case "$cmd" in
  detect)
    figcheck_have uv
    ;;
  run)
    in="${2:?usage: matplotlib.sh run <input.py> [outdir]}"
    outdir="${3:-$(figcheck_scratch matplotlib)}"
    mkdir -p "$outdir"
    FIGCHECK_OUTDIR="$outdir" uv run --no-project --with matplotlib python3 "$in"
    ;;
  *)
    echo "usage: matplotlib.sh {detect|run} ..." >&2
    exit 2
    ;;
esac
