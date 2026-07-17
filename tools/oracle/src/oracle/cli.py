"""`oracle` command-line entry point: run, generate, diff.

See `docs/plan/validation.md` for the mechanics this mirrors:

    oracle run --case pitch-defaults --audio tests/fixtures/audio/...
    oracle diff --reference REF.json --measured MEASURED.json
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from oracle import jsonio
from oracle import tolerances as tol
from oracle.cases import CASES
from oracle.diff import DiffError, diff
from oracle.paths import references_dir, repo_root
from oracle.runner import OracleUnavailable, load_parselmouth, reference_filename, resolve_case, run_case

SKIP_EXIT_CODE = 0  # graceful degradation: a clear message, not a failure


def _cmd_run(args: argparse.Namespace) -> int:
    try:
        case = resolve_case(args.case)
    except KeyError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    audio_filename = Path(args.audio).name
    try:
        payload = run_case(case, audio_filename)
    except OracleUnavailable as exc:
        print(f"SKIP: {exc}")
        return SKIP_EXIT_CODE
    except FileNotFoundError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    text = jsonio.dumps(payload)
    if args.out:
        Path(args.out).write_text(text, encoding="utf-8")
        print(f"wrote {args.out}")
    else:
        print(text, end="")
    return 0


def _cmd_generate(args: argparse.Namespace) -> int:
    try:
        parselmouth_module = load_parselmouth()
    except OracleUnavailable as exc:
        print(f"SKIP: {exc}")
        return SKIP_EXIT_CODE

    case_names = args.case or sorted(CASES)
    out_dir = Path(args.out_dir) if args.out_dir else references_dir()
    written = []
    for case_name in case_names:
        case = resolve_case(case_name)
        audio_files = args.audio or case.default_audio
        for audio_filename in audio_files:
            payload = run_case(case, audio_filename, parselmouth_module)
            out_path = out_dir / reference_filename(case.name, audio_filename)
            jsonio.write_json(out_path, payload)
            written.append(out_path)
            try:
                shown = out_path.relative_to(repo_root())
            except ValueError:
                shown = out_path
            print(f"wrote {shown}")

    print(f"generated {len(written)} reference file(s) in {out_dir}")
    return 0


def _print_report(report) -> None:
    status = "PASS" if report.passed else "FAIL"
    print(f"[{status}] measure={report.measure}")
    for key, value in report.summary.items():
        print(f"  {key}: {value}")
    if report.notes:
        print("  notes:")
        for note in report.notes[:20]:
            print(f"    - {note}")
        if len(report.notes) > 20:
            print(f"    ... and {len(report.notes) - 20} more")
    if report.violations:
        print(f"  violations ({len(report.violations)}, showing up to 20):")
        for v in report.violations[:20]:
            print(f"    - {v}")
        if len(report.violations) > 20:
            print(f"    ... and {len(report.violations) - 20} more")


def _cmd_diff(args: argparse.Namespace) -> int:
    ref_path = Path(args.reference)
    meas_path = Path(args.measured)
    if not ref_path.is_file():
        print(f"error: reference file not found: {ref_path}", file=sys.stderr)
        return 2
    if not meas_path.is_file():
        print(f"error: measured file not found: {meas_path}", file=sys.stderr)
        return 2

    reference = jsonio.read_json(ref_path)
    measured = jsonio.read_json(meas_path)

    try:
        report = diff(reference, measured)
    except DiffError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    _print_report(report)

    if args.json:
        jsonio.write_json(Path(args.json), report.as_dict())

    return 0 if report.passed else 1


def _cmd_diff_all(args: argparse.Namespace) -> int:
    """Diff every reference file in `references/` against a measured directory
    holding same-named files (the Rust CLI's dump directory).

    Formant is gated on a corpus-wide aggregate rather than each fixture's
    own `passed`: docs/plan/gates.md T2.6 records the accepted residual as
    487/6717 (7.3%) violations summed over the whole formant corpus, and
    individual fixtures range from well under that rate to well over it.
    Every other measure's `passed` already reflects gates.md's per-fixture
    acceptance record (see `oracle.diff`), so it gates the run directly.
    """
    ref_dir = references_dir()
    measured_dir = Path(args.measured_dir)
    ref_files = sorted(ref_dir.glob("*.json"))
    if not ref_files:
        print(f"SKIP: no reference files found in {ref_dir}")
        return SKIP_EXIT_CODE

    overall_pass = True
    formant_checked = 0
    formant_violations = 0
    formant_missing = 0
    for ref_path in ref_files:
        reference = jsonio.read_json(ref_path)
        if reference.get("measure") == "spectrogram":
            print(f"[SKIP] {ref_path.name}: spectrogram is not oracle-diffed (see validation.md)")
            continue
        measured_path = measured_dir / ref_path.name
        if not measured_path.is_file():
            print(f"[MISSING] no measured file for {ref_path.name} (expected {measured_path})")
            overall_pass = False
            continue
        measured = jsonio.read_json(measured_path)
        try:
            report = diff(reference, measured)
        except DiffError as exc:
            print(f"[ERROR] {ref_path.name}: {exc}")
            overall_pass = False
            continue
        _print_report(report)

        if report.measure == "formant":
            formant_checked += report.summary["checked_points"]
            formant_violations += report.summary["violations"]
            formant_missing += report.summary["missing_points"]
        else:
            overall_pass = overall_pass and report.passed

    if formant_checked:
        formant_violation_rate = formant_violations / formant_checked
        formant_ok = (
            formant_missing <= tol.FORMANT_MISSING_MAX
            and formant_violation_rate <= tol.FORMANT_CORPUS_VIOLATION_RATE_MAX
        )
        print(
            f"[{'PASS' if formant_ok else 'FAIL'}] measure=formant (corpus aggregate)\n"
            f"  checked_points: {formant_checked}\n"
            f"  violations: {formant_violations}\n"
            f"  violation_rate: {formant_violation_rate:.4f}\n"
            f"  violation_rate_max: {tol.FORMANT_CORPUS_VIOLATION_RATE_MAX}\n"
            f"  missing_points: {formant_missing}\n"
            f"  missing_points_max: {tol.FORMANT_MISSING_MAX}"
        )
        overall_pass = overall_pass and formant_ok

    return 0 if overall_pass else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="oracle", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    p_run = sub.add_parser("run", help="run one case against one fixture audio file")
    p_run.add_argument("--case", required=True, choices=sorted(CASES))
    p_run.add_argument("--audio", required=True, help="path or filename under tests/fixtures/audio/")
    p_run.add_argument("--out", help="write JSON here instead of stdout")
    p_run.set_defaults(func=_cmd_run)

    p_gen = sub.add_parser("generate", help="regenerate reference JSON files")
    p_gen.add_argument("--case", action="append", choices=sorted(CASES), help="repeatable; default: all cases")
    p_gen.add_argument("--audio", action="append", help="repeatable filename; default: each case's default corpus")
    p_gen.add_argument("--out-dir", help="default: tools/oracle/references/")
    p_gen.set_defaults(func=_cmd_generate)

    p_diff = sub.add_parser("diff", help="compare a measured JSON dump against a reference JSON")
    p_diff.add_argument("--reference", required=True)
    p_diff.add_argument("--measured", required=True)
    p_diff.add_argument("--json", help="also write the structured report here")
    p_diff.set_defaults(func=_cmd_diff)

    p_diff_all = sub.add_parser("diff-all", help="diff every committed reference against a directory of measured dumps")
    p_diff_all.add_argument("--measured-dir", required=True, help="directory of Rust-side JSON dumps, same filenames as references/")
    p_diff_all.set_defaults(func=_cmd_diff_all)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
