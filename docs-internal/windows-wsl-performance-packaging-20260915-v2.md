# WSL-native Windows GUI performance measurements

Medians and observed ranges; three repeats are diagnostic, not statistical significance.
The tested terminal is the native Windows GUI; the workload and sampling controller run in WSL.

## Startup readiness

The endpoint is the WSL worker's GUI Lua fence acknowledgment. It includes native GUI/WSL startup,
but is not time-to-first-present or shell-ready latency.

| Backend | Build | n | Median ms | Min–max ms |
|---|---|---:|---:|---:|
| OpenGL | baseline | 15 | 851.3 | 800.7–1006.7 |
| OpenGL | candidate | 15 | 793.0 | 739.3–839.6 |
| WebGpu | baseline | 15 | 1442.2 | 1381.3–1548.0 |
| WebGpu | candidate | 15 | 1374.8 | 1339.7–1445.2 |

## Portable ZIP runtime footprint

The portable package contains one GUI executable plus the fixed native runtime DLLs.
Sizes below are uncompressed; ZIP compression is not a runtime performance metric.

| Build | GUI executable bytes | GUI executable MiB |
|---|---:|---:|
| baseline | 72177497 | 68.83 |
| candidate | 71795894 | 68.47 |
| shared runtime DLLs | 44280920 | 42.23 |

## WSL/ConPTY output throughput

The final fence is a GUI-parsed OSC user-variable callback. Kitty images/s measures PNG/parser work and
is not displayed FPS or GPU presentation throughput.

| Backend | Workload | Unit | Baseline | Candidate | Change | Baseline range | Candidate range |
|---|---|---|---:|---:|---:|---:|---:|
| OpenGL | ascii | MiB/s | 22.80 | 22.98 | +0.8% | 22.62–23.12 | 22.62–24.48 |
| OpenGL | ansi | MiB/s | 22.40 | 23.47 | +4.8% | 22.22–23.74 | 22.92–24.19 |
| OpenGL | unicode | MiB/s | 22.62 | 25.05 | +10.8% | 22.16–24.36 | 23.29–25.87 |
| OpenGL | kitty | images/s (PNG/parser ACK) | 1955.46 | 1970.56 | +0.8% | 1940.01–2048.81 | 1959.46–2051.83 |
| WebGpu | ascii | MiB/s | 23.17 | 23.41 | +1.0% | 22.69–24.10 | 22.78–24.51 |
| WebGpu | ansi | MiB/s | 23.34 | 23.28 | -0.2% | 23.24–23.35 | 23.08–24.53 |
| WebGpu | unicode | MiB/s | 27.25 | 28.41 | +4.2% | 25.56–27.47 | 25.05–28.90 |
| WebGpu | kitty | images/s (PNG/parser ACK) | 2092.02 | 2038.99 | -2.5% | 2013.32–2120.06 | 1927.76–2055.63 |

## WSL worker resources

CPU excludes startup/settling and is for the WSL producer only. RSS is sampled approximately every 20 ms;
it excludes the Windows GUI process and GPU VRAM.

| Backend | Workload | Build | Worker CPU s | One-core CPU % | Peak RSS MiB |
|---|---|---|---:|---:|---:|
| OpenGL | idle | baseline | 0.0000 | 0.00 | 15.9 |
| OpenGL | idle | candidate | 0.0000 | 0.00 | 15.8 |
| OpenGL | ascii | baseline | 0.0800 | 10.90 | 15.9 |
| OpenGL | ascii | candidate | 0.0800 | 11.22 | 15.9 |
| OpenGL | ansi | baseline | 0.1100 | 14.98 | 15.9 |
| OpenGL | ansi | candidate | 0.1000 | 14.26 | 15.9 |
| OpenGL | unicode | baseline | 0.1000 | 14.01 | 15.8 |
| OpenGL | unicode | candidate | 0.1000 | 14.02 | 16.1 |
| OpenGL | kitty | baseline | 0.1000 | 61.32 | 16.3 |
| OpenGL | kitty | candidate | 0.0900 | 55.20 | 16.3 |
| WebGpu | idle | baseline | 0.0000 | 0.00 | 15.9 |
| WebGpu | idle | candidate | 0.0000 | 0.00 | 15.9 |
| WebGpu | ascii | baseline | 0.0900 | 13.38 | 15.9 |
| WebGpu | ascii | candidate | 0.0800 | 10.90 | 15.8 |
| WebGpu | ansi | baseline | 0.1000 | 14.29 | 15.9 |
| WebGpu | ansi | candidate | 0.1000 | 14.88 | 15.9 |
| WebGpu | unicode | baseline | 0.1000 | 15.26 | 15.8 |
| WebGpu | unicode | candidate | 0.0900 | 15.78 | 15.9 |
| WebGpu | kitty | baseline | 0.0900 | 55.26 | 16.2 |
| WebGpu | kitty | candidate | 0.1000 | 61.48 | 16.3 |
