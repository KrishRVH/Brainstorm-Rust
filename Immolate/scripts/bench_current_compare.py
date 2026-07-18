#!/usr/bin/env python3
"""Counterbalanced current-ABI DLL regression gate."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import math
import shutil
import statistics
import subprocess
import sys
import tempfile
from collections import defaultdict
from contextlib import contextmanager
from pathlib import Path

HEADER = [
    "kind", "impl", "case", "group", "shape", "budget", "scanned",
    "scan_pct", "threads", "sample", "elapsed_ms", "seeds_per_sec",
    "ns_per_seed", "min_ms", "p50_ms", "p95_ms", "p99_ms", "max_ms",
    "stdev_ms", "cv_pct", "result",
]
METRICS = ("p50", "p95", "p99", "mean")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def hash_artifacts(paths: dict[str, Path]) -> dict[str, str]:
    return {label: sha256(path) for label, path in paths.items()}


def verify_artifacts(paths: dict[str, Path], expected: dict[str, str]) -> None:
    changed = [label for label, path in paths.items() if sha256(path) != expected[label]]
    if changed:
        raise ValueError(f"artifact changed during comparison: {', '.join(changed)}")


def windows_path(path: Path, executor: str) -> str:
    converter = "winepath" if executor == "wine" else "wslpath"
    return subprocess.check_output(
        [converter, "-w", str(path)], text=True, stderr=subprocess.DEVNULL
    ).strip()


def native_temp_root(requested) -> Path:
    if requested is None:
        windows_temp = subprocess.check_output(
            ["cmd.exe", "/d", "/c", "echo", "%TEMP%"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        requested = Path(
            subprocess.check_output(
                ["wslpath", "-u", windows_temp],
                text=True,
                stderr=subprocess.DEVNULL,
            ).strip()
        )
    root = requested.resolve(strict=True)
    if not root.is_dir():
        raise ValueError(f"native staging root is not a directory: {root}")
    converted = windows_path(root, "native")
    if converted.startswith("\\\\"):
        raise ValueError("native staging must use a Windows-mounted local directory")
    return root


def emit_artifacts(scope: str, paths: dict[str, Path], hashes: dict[str, str]) -> None:
    for label in ("baseline", "candidate", "harness", "comparator"):
        if label in paths:
            print(f"artifact\t{scope}\t{label}\t{hashes[label]}\t{paths[label]}")


@contextmanager
def prepared_execution(
    args: argparse.Namespace,
    source_paths: dict[str, Path],
    source_hashes: dict[str, str],
):
    if args.executor == "wine":
        dlls = {
            "A": windows_path(source_paths["baseline"], "wine"),
            "B": windows_path(source_paths["candidate"], "wine"),
        }
        yield str(source_paths["harness"]), dlls
        return

    stage_root = native_temp_root(args.native_stage_dir)
    with tempfile.TemporaryDirectory(prefix="brainstorm-current-", dir=stage_root) as name:
        stage = Path(name)
        staged_paths = {
            "baseline": stage / "baseline.dll",
            "candidate": stage / "candidate.dll",
            "harness": stage / "harness.exe",
        }
        for label, destination in staged_paths.items():
            shutil.copyfile(source_paths[label], destination)
        verify_artifacts(staged_paths, source_hashes)
        emit_artifacts("native-staged", staged_paths, source_hashes)
        sys.stdout.flush()
        try:
            yield str(staged_paths["harness"]), {
                "A": windows_path(staged_paths["baseline"], "native"),
                "B": windows_path(staged_paths["candidate"], "native"),
            }
        finally:
            verify_artifacts(staged_paths, source_hashes)
            print("artifact_check\tnative-staged\tok")


def parse_runs(
    output: str,
    repeat: int,
    expected_budget=None,
    expected_threads=None,
) -> dict[str, list[tuple[int, str, float]]]:
    rows = list(csv.reader(io.StringIO(output), delimiter="\t"))
    if not rows or rows[0] != HEADER or any(len(row) != len(HEADER) for row in rows):
        raise ValueError("DLL harness emitted malformed TSV")
    runs: dict[str, list[tuple[int, str, float]]] = defaultdict(list)
    sample_ids: dict[str, set[int]] = defaultdict(set)
    summaries: set[str] = set()
    for row in rows[1:]:
        if row[0] == "run":
            if expected_budget is not None and int(row[5]) != expected_budget:
                raise ValueError(f"budget mismatch for {row[2]}")
            if expected_threads is not None and int(row[8]) != expected_threads:
                raise ValueError(f"thread mismatch for {row[2]}")
            elapsed = float(row[10])
            if not math.isfinite(elapsed) or elapsed < 0.0:
                raise ValueError(f"invalid elapsed time for {row[2]}: {row[10]}")
            runs[row[2]].append((int(row[6]), row[20], elapsed))
            sample_ids[row[2]].add(int(row[9]))
        elif row[0] == "summary":
            if int(row[9]) != repeat:
                raise ValueError(f"repeat mismatch for {row[2]}")
            if row[2] in summaries:
                raise ValueError(f"duplicate summary for {row[2]}")
            summaries.add(row[2])
        else:
            raise ValueError(f"unexpected harness row: {row[0]}")
    if not runs or set(runs) != summaries:
        raise ValueError("run/summary case coverage mismatch")
    for case, samples in runs.items():
        if (
            len(samples) != repeat
            or sample_ids[case] != set(range(1, repeat + 1))
            or len({sample[:2] for sample in samples}) != 1
        ):
            raise ValueError(f"unstable result/scanned count for {case}")
    return dict(runs)


def invoke(
    args: argparse.Namespace,
    harness: str,
    dll: str,
) -> dict[str, list[tuple[int, str, float]]]:
    command = ([] if args.executor == "native" else ["wine"]) + [
        harness, "bench", "--dll", dll,
        "--case", args.case, "--budget", str(args.budget),
        "--threads", str(args.threads), "--repeat", str(args.repeat),
        "--warmup", str(args.warmup), "--format", "tsv", "--color", "never",
    ]
    completed = subprocess.run(command, text=True, capture_output=True)
    if completed.returncode:
        sys.stderr.write(completed.stdout)
        sys.stderr.write(completed.stderr)
        raise ValueError(f"DLL harness failed with exit {completed.returncode}")
    return parse_runs(completed.stdout, args.repeat, args.budget, args.threads)


def collect_samples(args, harness, dlls, invoke_fn=invoke):
    samples: dict[tuple[str, int, str], list[tuple[int, str, float]]] = {}
    for cycle in range(1, args.cycles + 1):
        order = "ABBA" if cycle % 2 else "BAAB"
        for arm in order:
            for case, runs in invoke_fn(args, harness, dlls[arm]).items():
                samples.setdefault((case, cycle, arm), []).extend(runs)
    return samples


def percentile(values: list[float], pct: float) -> float:
    ordered = sorted(values)
    index = math.ceil((len(ordered) - 1) * pct)
    return ordered[index]


def metric_value(values: list[float], metric: str) -> float:
    if metric == "mean":
        return statistics.fmean(values)
    return percentile(values, {"p50": 0.50, "p95": 0.95, "p99": 0.99}[metric])


def is_regression(
    baseline_ms: float,
    candidate_ms: float,
    min_ratio: float,
    min_regression_ms: float,
) -> bool:
    if candidate_ms - baseline_ms <= min_regression_ms:
        return False
    return candidate_ms > 0.0 and baseline_ms / candidate_ms < min_ratio


def ratio(baseline_ms: float, candidate_ms: float) -> float:
    return math.inf if candidate_ms == 0.0 else baseline_ms / candidate_ms


def analyze_samples(samples, args):
    cases = sorted({case for case, _, _ in samples})
    expected_per_arm = 2 * args.repeat
    rows = []
    failures: set[tuple[str, str]] = set()
    for case in cases:
        identity: set[tuple[int, str]] = set()
        by_cycle: dict[int, dict[str, list[float]]] = {}
        for cycle in range(1, args.cycles + 1):
            by_cycle[cycle] = {}
            for arm in "AB":
                runs = samples.get((case, cycle, arm), [])
                if len(runs) != expected_per_arm:
                    raise ValueError(f"sample coverage mismatch for {case}/cycle {cycle}/{arm}")
                identity.update((scanned, result) for scanned, result, _ in runs)
                by_cycle[cycle][arm] = [elapsed for _, _, elapsed in runs]
        if len(identity) != 1:
            raise ValueError(f"baseline/candidate result mismatch for {case}: {identity}")

        scanned, result = next(iter(identity))
        for metric in METRICS:
            cycle_values = [
                (
                    metric_value(by_cycle[cycle]["A"], metric),
                    metric_value(by_cycle[cycle]["B"], metric),
                )
                for cycle in range(1, args.cycles + 1)
            ]
            baseline_ms = statistics.median(value[0] for value in cycle_values)
            candidate_ms = statistics.median(value[1] for value in cycle_values)
            paired_ratio = statistics.median(ratio(a, b) for a, b in cycle_values)
            paired_delta = statistics.median(b - a for a, b in cycle_values)
            pooled_a = metric_value(
                [value for cycle in by_cycle.values() for value in cycle["A"]], metric
            )
            pooled_b = metric_value(
                [value for cycle in by_cycle.values() for value in cycle["B"]], metric
            )
            regression_cycles = sum(
                is_regression(a, b, args.min_ratio, args.min_regression_ms)
                for a, b in cycle_values
            )
            paired_regression = (
                paired_delta > args.min_regression_ms
                and paired_ratio < args.min_ratio
            )
            pooled_regression = is_regression(
                pooled_a, pooled_b, args.min_ratio, args.min_regression_ms
            )
            failed = regression_cycles > args.cycles / 2 or (
                paired_regression and pooled_regression
            )
            if failed:
                failures.add((case, metric))
            watch = regression_cycles > 0 or paired_regression or pooled_regression
            rows.append({
                "case": case,
                "metric": metric,
                "scanned": scanned,
                "result": result,
                "baseline_ms": baseline_ms,
                "candidate_ms": candidate_ms,
                "paired_ratio": paired_ratio,
                "paired_delta_ms": paired_delta,
                "pooled_ratio": ratio(pooled_a, pooled_b),
                "pooled_delta_ms": pooled_b - pooled_a,
                "regression_cycles": regression_cycles,
                "status": "regression" if failed else "watch" if watch else "ok",
            })
    return rows, failures


def emit_settings(args: argparse.Namespace) -> None:
    settings = (
        ("executor", args.executor), ("case", args.case), ("budget", args.budget),
        ("threads", args.threads), ("repeat", args.repeat), ("warmup", args.warmup),
        ("cycles", args.cycles), ("order", "ABBA/BAAB"),
        ("min_ratio", args.min_ratio),
        ("min_regression_ms", args.min_regression_ms),
        ("metrics", ",".join(METRICS)),
    )
    for name, value in settings:
        print(f"setting\t{name}\t{value}")


def compare(args: argparse.Namespace) -> int:
    source_paths = {}
    for label in ("harness", "baseline", "candidate"):
        path = getattr(args, label).resolve(strict=True)
        if not path.is_file():
            raise ValueError(f"{label} is not a file: {path}")
        source_paths[label] = path
    source_paths["comparator"] = Path(__file__).resolve(strict=True)
    if args.cycles < 1 or args.repeat < 1 or args.warmup < 0 or args.budget < 1:
        raise ValueError("cycles, repeat, budget, and warmup must be positive/nonnegative")
    if not 0.0 <= args.min_ratio <= 1.0:
        raise ValueError("minimum ratio must be between zero and one")
    if args.min_regression_ms < 0.0:
        raise ValueError("minimum regression time cannot be negative")

    source_hashes = hash_artifacts(source_paths)
    if source_hashes["baseline"] == source_hashes["candidate"]:
        raise ValueError("baseline and candidate DLLs have identical contents")
    emit_settings(args)
    emit_artifacts("source", source_paths, source_hashes)
    sys.stdout.flush()
    try:
        with prepared_execution(args, source_paths, source_hashes) as (harness, dlls):
            samples = collect_samples(args, harness, dlls)
    finally:
        verify_artifacts(source_paths, source_hashes)
        print("artifact_check\tsource\tok")

    rows, failures = analyze_samples(samples, args)
    print(
        "case\tmetric\tscanned\tresult\tbaseline_median_ms\t"
        "candidate_median_ms\tpaired_ratio\tpaired_delta_ms\tpooled_ratio\t"
        "pooled_delta_ms\tregression_cycles\tstatus"
    )
    for row in rows:
        print(
            f"{row['case']}\t{row['metric']}\t{row['scanned']}\t{row['result']}\t"
            f"{row['baseline_ms']:.3f}\t{row['candidate_ms']:.3f}\t"
            f"{row['paired_ratio']:.3f}\t{row['paired_delta_ms']:.3f}\t"
            f"{row['pooled_ratio']:.3f}\t{row['pooled_delta_ms']:.3f}\t"
            f"{row['regression_cycles']}/{args.cycles}\t{row['status']}"
        )
    return 1 if failures else 0


def expect_value_error(operation) -> None:
    try:
        operation()
    except ValueError:
        return
    raise AssertionError("expected ValueError")


def synthetic_tsv(times: list[float], result: str = "SEED") -> str:
    output = io.StringIO()
    writer = csv.writer(output, delimiter="\t", lineterminator="\n")
    writer.writerow(HEADER)
    for sample, elapsed in enumerate(times, 1):
        row = [""] * len(HEADER)
        row[0], row[2], row[6], row[9], row[10], row[20] = (
            "run", "synthetic", "10", str(sample), str(elapsed), result
        )
        writer.writerow(row)
    summary = [""] * len(HEADER)
    summary[0], summary[2], summary[9] = "summary", "synthetic", str(len(times))
    writer.writerow(summary)
    return output.getvalue()


def self_test() -> None:
    parsed = parse_runs(synthetic_tsv([1.0, 1.1]), 2)
    assert parsed["synthetic"] == [(10, "SEED", 1.0), (10, "SEED", 1.1)]
    expect_value_error(lambda: parse_runs("bad\n", 1))

    args = argparse.Namespace(
        cycles=2, repeat=2, min_ratio=0.99, min_regression_ms=0.005
    )
    calls = []

    def fake_invoke(_args, _harness, dll):
        calls.append(dll)
        elapsed = 1.0 if dll == "A" else 1.02
        return {"synthetic": [(10, "SEED", elapsed)] * 2}

    samples = collect_samples(args, "harness", {"A": "A", "B": "B"}, fake_invoke)
    assert calls == list("ABBABAAB")
    rows, failures = analyze_samples(samples, args)
    assert len(rows) == 4 and len(failures) == 4

    mismatched = {key: list(value) for key, value in samples.items()}
    mismatched[("synthetic", 1, "B")][0] = (10, "DIFFERENT", 1.02)
    expect_value_error(lambda: analyze_samples(mismatched, args))

    majority_args = argparse.Namespace(
        cycles=3, repeat=2, min_ratio=0.99, min_regression_ms=0.005
    )
    majority_samples = {}
    for cycle in range(1, 4):
        majority_samples[("majority", cycle, "A")] = [(10, "SEED", 1.0)] * 4
        candidate = 1.02 if cycle < 3 else 0.99
        majority_samples[("majority", cycle, "B")] = [
            (10, "SEED", candidate)
        ] * 4
    _, majority_failures = analyze_samples(majority_samples, majority_args)
    assert ("majority", "p50") in majority_failures
    majority_samples[("majority", 2, "B")] = [(10, "SEED", 0.99)] * 4
    _, minority_failures = analyze_samples(majority_samples, majority_args)
    assert ("majority", "p50") not in minority_failures

    drift_args = argparse.Namespace(
        cycles=4, repeat=1, min_ratio=0.99, min_regression_ms=0.005
    )
    drift_samples = {}
    for cycle, (baseline, candidate) in enumerate(
        ((100.0, 102.0), (100.0, 102.0), (1.0, 0.99), (1.0, 0.99)), 1
    ):
        drift_samples[("drift", cycle, "A")] = [
            (10, "SEED", baseline)
        ] * 2
        drift_samples[("drift", cycle, "B")] = [
            (10, "SEED", candidate)
        ] * 2
    drift_rows, drift_failures = analyze_samples(drift_samples, drift_args)
    assert ("drift", "p50") not in drift_failures
    assert next(row for row in drift_rows if row["metric"] == "p50")["status"] == "watch"

    tail_args = argparse.Namespace(
        cycles=3, repeat=20, min_ratio=0.99, min_regression_ms=0.005
    )
    tail_samples = {}
    for cycle in range(1, 4):
        tail_samples[("tail", cycle, "A")] = [
            (10, "SEED", value) for value in ([1.0] * 38 + [2.0] * 2)
        ]
        tail_samples[("tail", cycle, "B")] = [
            (10, "SEED", value) for value in ([1.0] * 38 + [3.0] * 2)
        ]
    tail_rows, tail_failures = analyze_samples(tail_samples, tail_args)
    assert ("tail", "p50") not in tail_failures
    assert ("tail", "p95") in tail_failures and ("tail", "p99") in tail_failures
    assert next(row for row in tail_rows if row["metric"] == "p95")["status"] == "regression"

    with tempfile.TemporaryDirectory(prefix="brainstorm-current-self-test-") as name:
        artifact = Path(name) / "artifact"
        artifact.write_bytes(b"before")
        paths = {"artifact": artifact}
        hashes = hash_artifacts(paths)
        verify_artifacts(paths, hashes)
        artifact.write_bytes(b"after")
        expect_value_error(lambda: verify_artifacts(paths, hashes))

    assert not is_regression(1.0, 1.004, 0.99, 0.005)
    assert is_regression(1.0, 1.02, 0.99, 0.005)
    print("bench-current-compare self-test: PASS")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--harness", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--executor", choices=("native", "wine"), default="native")
    parser.add_argument("--native-stage-dir", type=Path)
    parser.add_argument("--case", default="ux")
    parser.add_argument("--budget", type=int, default=100_000)
    parser.add_argument("--threads", type=int, default=0)
    parser.add_argument("--repeat", type=int, default=31)
    parser.add_argument("--warmup", type=int, default=3)
    parser.add_argument("--cycles", type=int, default=4)
    parser.add_argument("--min-ratio", type=float, default=0.99)
    parser.add_argument("--min-regression-ms", type=float, default=0.005)
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not all((args.harness, args.baseline, args.candidate)):
        parser.error("--harness, --baseline, and --candidate are required")
    return compare(args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"bench-current-compare: {error}", file=sys.stderr)
        raise SystemExit(2)
