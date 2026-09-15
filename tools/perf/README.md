# Native Windows / WSL performance checks

These scripts launch **real Windows GUI processes**, not Wine or Linux GUI
substitutes. Each process owns one WSL worker and exits after its workload.
Only benchmark-owned GUI processes are killed on timeout. No user config or
persistent power setting is changed. Windows will open repeatedly during a run.

## Prerequisites

- Two optimized Windows executables, with identical build flags and native DLLs.
  Do not compare `cargo check` artifacts or debug binaries.
- An existing WSL distro with Python 3, and this worker accessible inside it.
- An interactive Windows desktop; avoid other CPU/GPU workloads during the run.
- Keep resolution, power mode, fonts, driver, ConPTY DLL and scrollback constant.

Place each executable on the Windows filesystem alongside `conpty.dll`,
`OpenConsole.exe`, `libEGL.dll`, `libGLESv2.dll` and `mesa/opengl32.dll` from
`assets/windows`. Cross-building on Linux does not automatically run the
Windows-host-only resource packaging code in `wezterm-gui/build.rs`.
For final shipping validation, use the normal native MSVC build as well.

## Run from WSL

Run the controller in WSL. Give it Windows-mounted paths to two native GUI
binaries; WSL interop starts the `.exe` directly. The controller never invokes
PowerShell, `cmd.exe`, Wine, or a Linux GUI:

```sh
python3 tools/perf/run-wsl.py \
  --baseline /mnt/c/perf/baseline/wezterm-gui.exe \
  --candidate /mnt/c/perf/candidate/wezterm-gui.exe \
  --distro "${WSL_DISTRO_NAME:-archlinux}" \
  --results "target/perf-results-wsl-$(date +%Y%m%d-%H%M%S)" \
  --runs 3 --warmups 1 --mib 16
```

The tested Windows GUI then starts `python3 tools/perf/wsl-workload.py` inside
that same WSL distro through its WSL/ConPTY session. All workload generation,
run coordination, result writing, and `/proc` worker CPU/RSS sampling happen in
WSL. Use a **fresh results directory per experiment**. Select a subset with
`--modes ascii unicode` or `--backends OpenGL`; use `--runs 1 --warmups 0` for
a smoke test.

This runs OpenGL and WebGpu with idle, ASCII, ANSI-color, mixed CJK/Unicode,
and Kitty PNG workloads. It warms each combination once, then alternates
baseline/candidate order. It requires an interactive Windows desktop because
the binary under test is a native GUI.

The tested configuration uses 120×40 cells, Consolas 11 pt, 1,000 scrollback
lines, a steady cursor, and both Kitty protocols enabled. Idle runs last five
seconds. Text workloads send at least the requested MiB in ~64 KiB chunks.
Kitty sends 120 distinct 256×256 PNGs with the same image ID and placement;
this bounds storage and avoids a cache-hit-only test. Its PNG query must
receive `OK` before that run is accepted.

## What is measured

- **Startup readiness:** WSL controller launch until the WSL worker receives
  its first GUI Lua acknowledgment. This includes native GUI/WSL startup, and
  is **not** time-to-first-present or shell-ready latency. The WSL controller
  polls every ~10 ms.
- **Output completion:** producer start through an OSC `SetUserVar` marker that
  the GUI parses and acknowledges via its Lua `user-var-changed` handler and
  `pane:send_text`. DSR alone is deliberately not used: ConPTY may answer it
  before the GUI consumes output. This includes the WSL/ConPTY pipeline and
  one final GUI/Lua round trip, not just a parser microbenchmark.
- **WSL worker CPU/RSS:** `/proc/<worker-pid>` CPU time and resident memory
  after a one-second settling interval, sampled approximately every 20 ms.
  This intentionally reports the WSL producer, not fabricated Windows GUI
  counters; it excludes the Windows GUI process and GPU VRAM.

Kitty images/s is parser/PNG-processing throughput, **not displayed FPS**.
No GPU present fence, frame-time trace, input-to-photon latency, or GPU VRAM
measurement is made. Intermediate frames can be skipped/coalesced. Renderer
selection must also consider representative interactive applications.

Keep warmups and first-launch observations separate. Three repeats provide an
initial diagnostic, not statistical significance; report ranges and investigate
large run-to-run variation rather than claiming every median delta is a gain.

## Data and harness checks

The runner writes per-run JSON, a WSL `manifest.jsonl`, and stderr logs;
`.ready`, `.go`, and `.done` files coordinate only the worker and controller.
Completed samples include worker completion and native GUI exit status.
Summarization rejects incomplete groups.

The runner also records WSL/kernel/CPU/memory and SHA-256 hashes of both
binaries in UTF-8 `environment.json`:

```sh
python3 tools/perf/summarize.py --results target/perf-results-wsl-YYYYMMDD-HHMMSS \
  --output-prefix docs-internal/performance/windows-wsl-YYYYMMDD
python3 -m unittest discover -s tools/perf -p 'test_*.py'
```

The PTY harness test validates coordination, byte counts and positive protocol
responses for all five workloads; it is not a substitute for native GUI runs.
