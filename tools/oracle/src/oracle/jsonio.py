"""Deterministic JSON I/O for reference/measured files.

Reference JSON is a committed data fixture: two runs of the same case must
byte-for-byte agree, so nondeterminism (dict key order, float repr jitter,
NaN handling) is squeezed out before writing.
"""

from __future__ import annotations

import json
import math
from typing import Any

FLOAT_DECIMALS = 6


def _round(value: float) -> float:
    if math.isnan(value) or math.isinf(value):
        # json has no literal for these; callers must not pass them through
        # to write_json without first converting to None.
        raise ValueError(f"non-finite float cannot be serialized: {value!r}")
    return round(value, FLOAT_DECIMALS)


def _normalize(obj: Any) -> Any:
    if isinstance(obj, float):
        return _round(obj)
    if isinstance(obj, dict):
        return {k: _normalize(v) for k, v in sorted(obj.items())}
    if isinstance(obj, (list, tuple)):
        return [_normalize(v) for v in obj]
    return obj


def dumps(obj: Any) -> str:
    """Serialize with sorted keys, fixed float precision, trailing newline."""
    normalized = _normalize(obj)
    return json.dumps(normalized, indent=2, sort_keys=True, ensure_ascii=True) + "\n"


def write_json(path, obj: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(dumps(obj), encoding="utf-8")


def read_json(path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))
