#!/usr/bin/env bash
# Vega-Lite backend runner: validates a JSON spec against the vendored
# Vega-Lite v5 JSON schema (fixtures/vega/schema/vega-lite-v5.schema.json,
# fetched once from https://vega.github.io/schema/vega-lite/v5.json and
# checked in so validation runs offline; refresh by re-running that curl).
#
#   vega.sh detect
#   vega.sh run <input.json> [outdir]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
. lib/common.sh

schema="fixtures/vega/schema/vega-lite-v5.schema.json"

cmd="${1:-}"
case "$cmd" in
  detect)
    figcheck_have uv && [ -f "$schema" ]
    ;;
  run)
    in="${2:?usage: vega.sh run <input.json> [outdir]}"
    uv run --no-project --with jsonschema python3 - "$schema" "$in" <<'PY'
import json
import sys

import jsonschema

schema_path, instance_path = sys.argv[1], sys.argv[2]
with open(schema_path, encoding="utf-8") as f:
    schema = json.load(f)
with open(instance_path, encoding="utf-8") as f:
    instance = json.load(f)

jsonschema.validate(instance=instance, schema=schema)
print(f"{instance_path}: valid against Vega-Lite v5 schema")
PY
    ;;
  *)
    echo "usage: vega.sh {detect|run} ..." >&2
    exit 2
    ;;
esac
