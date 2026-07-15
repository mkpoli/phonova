#!/usr/bin/env bash
# TikZ/PGFPlots backend runner: compiles a .tex file with latexmk -pdf.
#
#   tikz.sh detect
#   tikz.sh run <input.tex> [outdir]
#
# Detection requires latexmk itself plus the pgfplots.sty package, since a
# TeX Live install missing pgfplots would otherwise "detect" as available
# and then fail every real check.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
. lib/common.sh

cmd="${1:-}"
case "$cmd" in
  detect)
    figcheck_have latexmk && figcheck_have kpsewhich || exit 1
    kpsewhich pgfplots.sty >/dev/null 2>&1
    ;;
  run)
    in="${2:?usage: tikz.sh run <input.tex> [outdir]}"
    outdir="${3:-$(figcheck_scratch tikz)}"
    mkdir -p "$outdir"
    latexmk -pdf -interaction=nonstopmode -halt-on-error \
      -output-directory="$outdir" "$in"
    ;;
  *)
    echo "usage: tikz.sh {detect|run} ..." >&2
    exit 2
    ;;
esac
