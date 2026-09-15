#!/usr/bin/env python3
"""Run native Windows GUI measurements from inside WSL only.

The workload is always a WSL child of the tested native GUI/ConPTY session.
No PowerShell, cmd.exe, Wine, or Linux GUI substitute is used.  Windows
interop is used only to execute the native GUI binary.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time
import uuid


MODES = ("idle", "ascii", "ansi", "unicode", "kitty")
BACKENDS = ("OpenGL", "WebGpu")


def now_ns() -> int:
    return time.perf_counter_ns()


def json_write(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")
    temporary.replace(path)


def linux_to_windows(path: Path) -> str:
    result = subprocess.run(
        ["wslpath", "-w", str(path)], check=True, capture_output=True, text=True
    )
    return result.stdout.strip()


def lua_string(value: str) -> str:
    return "'" + value.replace("\\", "\\\\").replace("'", "\\'") + "'"


def write_config(path: Path, distro: str, backend: str) -> str:
    config = f"""local wezterm = require 'wezterm'
wezterm.on('user-var-changed', function(window, pane, name, value)
  if name == 'PERF_FENCE' then
    pane:send_text('PERF_ACK:' .. value .. ':END')
  end
end)
return {{
  front_end = {lua_string(backend)},
  wsl_domains = {{ {{
    name = 'perf-wsl', distribution = {lua_string(distro)}, default_cwd = '~',
  }} }},
  default_domain = 'perf-wsl',
  font = wezterm.font 'Consolas',
  font_size = 11.0,
  initial_cols = 120,
  initial_rows = 40,
  scrollback_lines = 1000,
  default_cursor_style = 'SteadyBlock',
  enable_kitty_keyboard = true,
  enable_kitty_graphics = true,
  exit_behavior = 'Close',
  window_close_confirmation = 'NeverPrompt',
}}
"""
    path.write_text(config, encoding="utf-8")
    return linux_to_windows(path)


def proc_stats(pid: int, ticks_per_second: int) -> tuple[float, float] | None:
    """Return (CPU seconds, resident MiB) for a WSL worker process."""
    try:
        fields = (Path(f"/proc/{pid}/stat")).read_text().split()
        # Linux proc stat fields 14 and 15 are utime and stime; zero-indexed
        # positions 13 and 14 after split. The worker name has no whitespace.
        cpu = (int(fields[13]) + int(fields[14])) / ticks_per_second
        rss_kib = 0.0
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                rss_kib = float(line.split()[1])
                break
        return cpu, rss_kib / 1024.0
    except (FileNotFoundError, ProcessLookupError, IndexError, ValueError):
        return None


def wait_for(path: Path, process: subprocess.Popen[bytes], timeout: float, what: str) -> None:
    deadline = time.monotonic() + timeout
    while not path.exists():
        if process.poll() is not None:
            raise RuntimeError(
                f"GUI exited before {what} (exit={process.returncode}); "
                f"see {path.parent}/*.stderr.log"
            )
        if time.monotonic() >= deadline:
            raise TimeoutError(f"timeout waiting for {what}: {path}")
        time.sleep(0.01)


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def environment(results: Path, executables: dict[str, Path], distro: str) -> None:
    os_release = Path("/etc/os-release").read_text(encoding="utf-8", errors="replace") if Path("/etc/os-release").exists() else ""
    cpu_model = ""
    for line in Path("/proc/cpuinfo").read_text(errors="replace").splitlines():
        if line.startswith("model name"):
            cpu_model = line.split(":", 1)[1].strip()
            break
    mem_total = 0
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemTotal:"):
            mem_total = int(line.split()[1]) * 1024
            break
    json_write(
        results / "environment.json",
        {
            "captured_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "wsl_distro": distro,
            "wsl_env": os.environ.get("WSL_DISTRO_NAME"),
            "kernel": platform.release(),
            "machine": platform.machine(),
            "cpu": cpu_model,
            "logical_processors": os.cpu_count(),
            "memory_bytes": mem_total,
            "os_release": os_release,
            "runner": "tools/perf/run-wsl.py",
            "executables": {
                name: {
                    "path": str(path),
                    "size_bytes": path.stat().st_size,
                    "sha256": hash_file(path),
                }
                for name, path in executables.items()
            },
        },
    )


def run_one(
    executable: Path,
    worker: Path,
    results: Path,
    label: str,
    backend: str,
    mode: str,
    run: int,
    warmup: bool,
    distro: str,
    mib: int,
    settle_seconds: float,
    timeout: float,
) -> dict:
    token = f"{label}-{backend}-{mode}-{run}-{uuid.uuid4().hex}"
    result = results / f"{token}.worker.json"
    ready = Path(str(result) + ".ready")
    go = Path(str(result) + ".go")
    done = Path(str(result) + ".done")
    stderr_path = results / f"{token}.stderr.log"
    stdout_path = results / f"{token}.stdout.log"
    config_path = results / f"{token}.lua"
    config_win = write_config(config_path, distro, backend)
    ticks = os.sysconf("SC_CLK_TCK")
    command = [
        str(executable),
        "--config-file",
        config_win,
        "blocking-start",
        "--",
        "python3",
        str(worker),
        "--mode",
        mode,
        "--mib",
        str(mib),
        "--result",
        str(result),
    ]
    started_ns = now_ns()
    process: subprocess.Popen[bytes] | None = None
    worker_pid: int | None = None
    peak_rss = 0.0
    worker_cpu_start = None
    worker_cpu_end = None
    work_started_ns = None
    try:
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            process = subprocess.Popen(
                command,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                close_fds=True,
            )
            wait_for(ready, process, timeout, "worker readiness")
            startup_ready_ms = (now_ns() - started_ns) / 1_000_000.0
            ready_data = json.loads(ready.read_text(encoding="utf-8"))
            worker_pid = int(ready_data["pid"])
            time.sleep(settle_seconds)
            stats = proc_stats(worker_pid, ticks)
            if stats:
                worker_cpu_start, rss = stats
                peak_rss = max(peak_rss, rss)
            work_started_ns = now_ns()
            go.write_text("go", encoding="ascii")
            while not result.exists():
                stats = proc_stats(worker_pid, ticks)
                if stats:
                    worker_cpu_end, rss = stats
                    peak_rss = max(peak_rss, rss)
                if process.poll() is not None:
                    raise RuntimeError(
                        f"GUI exited during {mode} (exit={process.returncode}); "
                        f"see {stderr_path}"
                    )
                if (now_ns() - work_started_ns) / 1_000_000_000 > timeout:
                    raise TimeoutError(f"timeout waiting for workload: {result}")
                time.sleep(0.02)
            work_finished_ns = now_ns()
            worker_result = json.loads(result.read_text(encoding="utf-8"))
            if "error" in worker_result:
                raise RuntimeError(f"worker failed: {worker_result['error']}")
            stats = proc_stats(worker_pid, ticks)
            if stats:
                worker_cpu_end, rss = stats
                peak_rss = max(peak_rss, rss)
            done.write_text("done", encoding="ascii")
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired as error:
                raise TimeoutError(f"GUI did not exit after {mode}: {stderr_path}") from error
            if process.returncode != 0:
                raise RuntimeError(f"GUI exit code {process.returncode}; see {stderr_path}")
    except Exception:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait()
        raise
    finally:
        config_path.unlink(missing_ok=True)

    worker_cpu_seconds = None
    if worker_cpu_start is not None and worker_cpu_end is not None:
        worker_cpu_seconds = max(0.0, worker_cpu_end - worker_cpu_start)
    controller_seconds = (work_finished_ns - work_started_ns) / 1_000_000_000
    record = {
        "label": label,
        "backend_requested": backend,
        "mode": mode,
        "run": run,
        "warmup": warmup,
        "interop_pid": process.pid if process else None,
        "gui_exit_code": process.returncode if process else None,
        "worker_pid": worker_pid,
        "startup_ready_ms": startup_ready_ms,
        "controller_work_seconds": controller_seconds,
        "worker_cpu_seconds": worker_cpu_seconds,
        "worker_cpu_one_core_percent": (
            100.0 * worker_cpu_seconds / controller_seconds
            if worker_cpu_seconds is not None and controller_seconds > 0
            else None
        ),
        "worker_peak_rss_mib": peak_rss,
        "worker": worker_result,
        "stderr": str(stderr_path),
        "stdout": str(stdout_path),
    }
    json_write(results / f"{token}.json", record)
    with (results / "manifest.jsonl").open("a", encoding="utf-8") as manifest:
        manifest.write(json.dumps(record, ensure_ascii=False) + "\n")
    print(
        f"{label:9} {backend:6} {mode:7} run={run} warmup={warmup} "
        f"startup={startup_ready_ms:7.1f}ms "
        f"rate={worker_result.get('mib_per_second', 0):6.2f}MiB/s "
        f"worker-rss={peak_rss:6.1f}MiB",
        flush=True,
    )
    return record


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--worker", type=Path, default=Path(__file__).with_name("wsl-workload.py"))
    parser.add_argument("--results", type=Path, default=Path("target/perf-results-wsl"))
    parser.add_argument("--distro", default=os.environ.get("WSL_DISTRO_NAME", ""))
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--mib", type=int, default=16)
    parser.add_argument("--settle-seconds", type=float, default=1.0)
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument("--modes", nargs="+", choices=MODES, default=list(MODES))
    parser.add_argument("--backends", nargs="+", choices=BACKENDS, default=list(BACKENDS))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.distro:
        raise SystemExit("--distro is required when WSL_DISTRO_NAME is not set")
    if args.runs < 1 or args.warmups < 0 or args.mib < 1:
        raise SystemExit("--runs must be >=1, --warmups >=0 and --mib >=1")
    args.results = args.results.resolve()
    executables = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    for name, executable in executables.items():
        if not executable.is_file():
            raise SystemExit(f"{name} executable does not exist: {executable}")
    worker = args.worker.resolve()
    if not worker.is_file():
        raise SystemExit(f"worker does not exist: {worker}")
    if args.results.exists() and any(args.results.iterdir()):
        raise SystemExit(f"results directory is not empty; use a new path: {args.results}")
    args.results.mkdir(parents=True, exist_ok=True)
    environment(args.results, executables, args.distro)
    order = ("baseline", "candidate")
    for backend in args.backends:
        for mode in args.modes:
            for round_number in range(args.warmups + args.runs):
                if round_number % 2:
                    order = ("candidate", "baseline")
                else:
                    order = ("baseline", "candidate")
                warmup = round_number < args.warmups
                run = round_number if not warmup else round_number
                for label in order:
                    label_for_file = f"warmup-{label}" if warmup else label
                    run_one(
                        executables[label], worker, args.results, label_for_file,
                        backend, mode, run, warmup, args.distro, args.mib,
                        args.settle_seconds, args.timeout,
                    )
    print(f"RESULTS={args.results}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(130)
