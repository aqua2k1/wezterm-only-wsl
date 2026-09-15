# WSL-native Windows GUI performance measurements

Medians and observed ranges; three repeats are diagnostic, not statistical significance.
The tested terminal is the native Windows GUI; the workload and sampling controller run in WSL.

## Startup readiness

The endpoint is the WSL worker's GUI Lua fence acknowledgment. It includes native GUI/WSL startup,
but is not time-to-first-present or shell-ready latency.

| Backend | Build | n | Median ms | Min–max ms |
|---|---|---:|---:|---:|
| OpenGL | baseline | 15 | 740.5 | 680.1–813.4 |
| OpenGL | candidate | 15 | 732.6 | 681.8–868.8 |
| WebGpu | baseline | 15 | 1382.6 | 1300.3–1560.2 |
| WebGpu | candidate | 15 | 1353.7 | 1309.4–1485.7 |

## Portable ZIP runtime footprint

The portable package contains one GUI executable plus the fixed native runtime DLLs.
Sizes below are uncompressed; ZIP compression is not a runtime performance metric.

| Build | GUI executable bytes | GUI executable MiB |
|---|---:|---:|
| baseline | 71795894 | 68.47 |
| candidate | 71784423 | 68.46 |
| shared runtime DLLs | 44280920 | 42.23 |

## WSL/ConPTY output throughput

The final fence is a GUI-parsed OSC user-variable callback. Kitty images/s measures PNG/parser work and
is not displayed FPS or GPU presentation throughput.

| Backend | Workload | Unit | Baseline | Candidate | Change | Baseline range | Candidate range |
|---|---|---|---:|---:|---:|---:|---:|
| OpenGL | ascii | MiB/s | 20.46 | 20.38 | -0.4% | 20.35–20.82 | 19.80–21.06 |
| OpenGL | ansi | MiB/s | 21.09 | 22.47 | +6.5% | 20.57–21.24 | 21.86–23.35 |
| OpenGL | unicode | MiB/s | 23.29 | 22.87 | -1.8% | 22.18–23.81 | 22.03–24.34 |
| OpenGL | kitty | images/s (PNG/parser ACK) | 1887.51 | 1909.36 | +1.2% | 1883.28–1925.60 | 1852.09–1981.82 |
| WebGpu | ascii | MiB/s | 22.09 | 22.23 | +0.6% | 21.20–22.38 | 22.14–22.31 |
| WebGpu | ansi | MiB/s | 21.50 | 21.38 | -0.6% | 20.08–22.19 | 20.77–21.39 |
| WebGpu | unicode | MiB/s | 24.85 | 24.11 | -3.0% | 21.55–25.85 | 23.90–25.23 |
| WebGpu | kitty | images/s (PNG/parser ACK) | 1851.93 | 1888.03 | +1.9% | 1774.94–2023.70 | 1882.70–1896.58 |

## WSL worker resources

CPU excludes startup/settling and is for the WSL producer only. RSS is sampled approximately every 20 ms;
it excludes the Windows GUI process and GPU VRAM.

| Backend | Workload | Build | Worker CPU s | One-core CPU % | Peak RSS MiB |
|---|---|---|---:|---:|---:|
| OpenGL | idle | baseline | 0.0000 | 0.00 | 15.9 |
| OpenGL | idle | candidate | 0.0000 | 0.00 | 15.9 |
| OpenGL | ascii | baseline | 0.0900 | 10.96 | 15.9 |
| OpenGL | ascii | candidate | 0.0800 | 10.31 | 15.9 |
| OpenGL | ansi | baseline | 0.1100 | 13.83 | 15.9 |
| OpenGL | ansi | candidate | 0.1100 | 15.87 | 15.9 |
| OpenGL | unicode | baseline | 0.0900 | 12.60 | 15.9 |
| OpenGL | unicode | candidate | 0.1000 | 13.25 | 15.8 |
| OpenGL | kitty | baseline | 0.1000 | 54.44 | 16.2 |
| OpenGL | kitty | candidate | 0.1000 | 61.27 | 16.3 |
| WebGpu | idle | baseline | 0.0000 | 0.00 | 15.9 |
| WebGpu | idle | candidate | 0.0000 | 0.00 | 15.9 |
| WebGpu | ascii | baseline | 0.1000 | 12.90 | 15.9 |
| WebGpu | ascii | candidate | 0.0800 | 10.60 | 15.9 |
| WebGpu | ansi | baseline | 0.1100 | 13.47 | 15.9 |
| WebGpu | ansi | candidate | 0.1100 | 13.82 | 15.9 |
| WebGpu | unicode | baseline | 0.1000 | 15.30 | 15.8 |
| WebGpu | unicode | candidate | 0.1100 | 15.78 | 15.8 |
| WebGpu | kitty | baseline | 0.1000 | 54.48 | 16.3 |
| WebGpu | kitty | candidate | 0.1000 | 54.54 | 16.3 |
