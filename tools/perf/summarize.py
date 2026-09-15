#!/usr/bin/env python3
"""Summarize measurements produced by run-wsl.py.

The input is deliberately WSL-native: worker CPU/RSS and the WSL/ConPTY
throughput are reported. Windows GUI CPU, GPU VRAM and present timing are not
pretended to be available from /proc.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import statistics


BACKENDS = ("OpenGL", "WebGpu")
MODES = ("idle", "ascii", "ansi", "unicode", "kitty")
LABELS = ("baseline", "candidate")


def median(values):
    return statistics.median(values)


def records_from(path: Path):
    manifest = path / "manifest.jsonl"
    if not manifest.is_file():
        raise ValueError(f"missing WSL manifest: {manifest}")
    records = []
    for line in manifest.read_text(encoding="utf-8").splitlines():
        if line.strip():
            record = json.loads(line)
            if not record.get("warmup") and record.get("label") in LABELS:
                records.append(record)
    return records


def group_records(records):
    groups = {}
    for record in records:
        key = (record["backend_requested"], record["mode"], record["label"])
        groups.setdefault(key, []).append(record)
    for backend in BACKENDS:
        for mode in MODES:
            for label in LABELS:
                group = groups.get((backend, mode, label), [])
                if len(group) < 3:
                    raise ValueError(
                        f"need at least three valid runs for {backend}/{mode}/{label}; "
                        f"got {len(group)}"
                    )
                for record in group:
                    worker = record.get("worker", {})
                    if "error" in worker or record.get("worker_cpu_seconds") is None:
                        raise ValueError(f"incomplete run: {record}")
    return groups


def range_text(values):
    return f"{min(values):.2f}–{max(values):.2f}"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--output-prefix", type=Path, required=True)
    args = parser.parse_args()
    records = records_from(args.results)
    groups = group_records(records)
    environment = json.loads((args.results / "environment.json").read_text(encoding="utf-8"))

    lines = [
        "# WSL-native Windows GUI performance measurements",
        "",
        "Medians and observed ranges; three repeats are diagnostic, not statistical significance.",
        "The tested terminal is the native Windows GUI; the workload and sampling controller run in WSL.",
        "",
        "## Startup readiness",
        "",
        "The endpoint is the WSL worker's GUI Lua fence acknowledgment. It includes native GUI/WSL startup,",
        "but is not time-to-first-present or shell-ready latency.",
        "",
        "| Backend | Build | n | Median ms | Min–max ms |",
        "|---|---|---:|---:|---:|",
    ]
    for backend in BACKENDS:
        for label in LABELS:
            values = [
                record["startup_ready_ms"]
                for record in records
                if record["backend_requested"] == backend and record["label"] == label
            ]
            lines.append(
                f"| {backend} | {label} | {len(values)} | {median(values):.1f} | "
                f"{min(values):.1f}–{max(values):.1f} |"
            )

    lines += [
        "",
        "## WSL/ConPTY output throughput",
        "",
        "The final fence is a GUI-parsed OSC user-variable callback. Kitty images/s measures PNG/parser work and",
        "is not displayed FPS or GPU presentation throughput.",
        "",
        "| Backend | Workload | Unit | Baseline | Candidate | Change | Baseline range | Candidate range |",
        "|---|---|---|---:|---:|---:|---:|---:|",
    ]
    for backend in BACKENDS:
        for mode in ("ascii", "ansi", "unicode", "kitty"):
            values = []
            for label in LABELS:
                value = [
                    (record["worker"]["images"] / record["worker"]["seconds"])
                    if mode == "kitty"
                    else record["worker"]["mib_per_second"]
                    for record in groups[(backend, mode, label)]
                ]
                values.append(value)
            baseline, candidate = values
            unit = "images/s (PNG/parser ACK)" if mode == "kitty" else "MiB/s"
            lines.append(
                f"| {backend} | {mode} | {unit} | {median(baseline):.2f} | "
                f"{median(candidate):.2f} | {(median(candidate) / median(baseline) - 1) * 100:+.1f}% | "
                f"{range_text(baseline)} | {range_text(candidate)} |"
            )

    lines += [
        "",
        "## WSL worker resources",
        "",
        "CPU excludes startup/settling and is for the WSL producer only. RSS is sampled approximately every 20 ms;",
        "it excludes the Windows GUI process and GPU VRAM.",
        "",
        "| Backend | Workload | Build | Worker CPU s | One-core CPU % | Peak RSS MiB |",
        "|---|---|---|---:|---:|---:|",
    ]
    for backend in BACKENDS:
        for mode in MODES:
            for label in LABELS:
                group = groups[(backend, mode, label)]
                cpu = median([record["worker_cpu_seconds"] for record in group])
                percent = median([record["worker_cpu_one_core_percent"] for record in group])
                rss = median([record["worker_peak_rss_mib"] for record in group])
                lines.append(f"| {backend} | {mode} | {label} | {cpu:.4f} | {percent:.2f} | {rss:.1f} |")

    output = {"environment": environment, "samples": records}
    args.output_prefix.parent.mkdir(parents=True, exist_ok=True)
    args.output_prefix.with_suffix(".json").write_text(
        json.dumps(output, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    args.output_prefix.with_suffix(".md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
