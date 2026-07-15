#!/usr/bin/env bash
# ggplot2 backend runner: executes a generated R script that renders a PNG
# with base ggplot2 (no ggplot2 extension packages).
#
#   ggplot2.sh detect
#   ggplot2.sh run <input.R> [outdir]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
. lib/common.sh

cmd="${1:-}"
case "$cmd" in
  detect)
    figcheck_have Rscript || exit 1
    Rscript -e 'quit(status = if (requireNamespace("ggplot2", quietly = TRUE)) 0 else 1)' \
      >/dev/null 2>&1
    ;;
  run)
    in="${2:?usage: ggplot2.sh run <input.R> [outdir]}"
    outdir="${3:-$(figcheck_scratch ggplot2)}"
    mkdir -p "$outdir"
    FIGCHECK_OUTDIR="$outdir" Rscript --vanilla "$in"
    ;;
  *)
    echo "usage: ggplot2.sh {detect|run} ..." >&2
    exit 2
    ;;
esac
