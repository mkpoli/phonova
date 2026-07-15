#!/usr/bin/env bash
# figcheck: run one toolchain's compile/execute check against a generated
# figure export.
#
#   check.sh BACKEND detect
#   check.sh BACKEND run INPUT-FILE [OUTDIR]
#
# BACKEND is one of: typst tikz matplotlib ggplot2 makie vega
#
# Intended for both interactive use against real crates/phx-figure exports
# once T5.3 lands, and as the primitive selftest.sh drives against the
# fixtures/ good/bad pairs.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

backend="${1:?usage: check.sh BACKEND detect|run ...}"
shift

backend_script="backends/$backend.sh"
if [ ! -f "$backend_script" ]; then
  echo "figcheck: unknown backend '$backend' (no $backend_script)" >&2
  exit 2
fi

exec "$backend_script" "$@"
