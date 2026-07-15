#!/usr/bin/env bash
# Typst backend runner: compiles a .typ file with the typst CLI.
#
#   typst.sh detect
#   typst.sh run <input.typ> [outdir]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
. lib/common.sh

cmd="${1:-}"
case "$cmd" in
  detect)
    figcheck_have typst
    ;;
  run)
    in="${2:?usage: typst.sh run <input.typ> [outdir]}"
    outdir="${3:-$(figcheck_scratch typst)}"
    mkdir -p "$outdir"
    typst compile "$in" "$outdir/$(basename "${in%.typ}").pdf"
    ;;
  *)
    echo "usage: typst.sh {detect|run} ..." >&2
    exit 2
    ;;
esac
