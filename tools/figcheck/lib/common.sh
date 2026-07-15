# Shared shell helpers for tools/figcheck backend runners.
#
# A backend script under backends/ sources this file and implements two
# entry points invoked by check.sh and selftest.sh:
#   detect            -- exit 0 if the toolchain is usable, 1 otherwise
#   run <in> [outdir]  -- compile/execute <in>; exit code is the toolchain's
#
# Availability is a runtime property of the machine, never assumed from a
# package being listed anywhere -- detect() always calls the real binary.

set -u

figcheck_notice() {
  echo "::notice::$1"
}

figcheck_warning() {
  echo "::warning::$1"
}

figcheck_have() {
  command -v "$1" >/dev/null 2>&1
}

# figcheck_scratch <label> -- fresh temp dir for one compile/execute attempt.
figcheck_scratch() {
  mktemp -d "${TMPDIR:-/tmp}/figcheck-$1.XXXXXX"
}
