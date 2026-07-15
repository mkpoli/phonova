#!/usr/bin/env bash
# Makie backend runner: executes a generated Julia script that renders a PNG
# with CairoMakie (the software-rasterizing Makie backend -- headless CI has
# no GPU/display for GLMakie).
#
#   makie.sh detect
#   makie.sh run <input.jl> [outdir]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
. lib/common.sh

cmd="${1:-}"
case "$cmd" in
  detect)
    figcheck_have julia || exit 1
    julia -e 'using CairoMakie' >/dev/null 2>&1
    ;;
  run)
    in="${2:?usage: makie.sh run <input.jl> [outdir]}"
    outdir="${3:-$(figcheck_scratch makie)}"
    mkdir -p "$outdir"
    FIGCHECK_OUTDIR="$outdir" julia "$in"
    ;;
  *)
    echo "usage: makie.sh {detect|run} ..." >&2
    exit 2
    ;;
esac
