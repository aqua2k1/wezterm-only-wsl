# WSL-native Windows GUI performance measurements

Medians and observed ranges; three repeats are diagnostic, not statistical significance.
The tested terminal is the native Windows GUI; the workload and sampling controller run in WSL.

## Startup readiness

The endpoint is the WSL worker's GUI Lua fence acknowledgment. It includes native GUI/WSL startup,
but is not time-to-first-present or shell-ready latency.

| Backend | Build | n | Median ms | Min–max ms |
|---|---|---:|---:|---:|
| OpenGL | baseline | 15 | 810.4 | 763.3–996.1 |
| OpenGL | candidate | 15 | 841.1 | 781.1–931.3 |
| WebGpu | baseline | 15 | 1388.3 | 1300.7–1691.8 |
| WebGpu | candidate | 15 | 1414.7 | 1333.3–1493.3 |

## WSL/ConPTY output throughput

The final fence is a GUI-parsed OSC user-variable callback. Kitty images/s measures PNG/parser work and
is not displayed FPS or GPU presentation throughput.

| Backend | Workload | Unit | Baseline | Candidate | Change | Baseline range | Candidate range |
|---|---|---|---:|---:|---:|---:|---:|
| OpenGL | ascii | MiB/s | 22.69 | 22.04 | -2.9% | 22.34–24.17 | 21.94–22.07 |
| OpenGL | ansi | MiB/s | 22.78 | 22.50 | -1.2% | 21.47–23.68 | 21.58–23.02 |
| OpenGL | unicode | MiB/s | 23.78 | 23.37 | -1.7% | 23.00–24.18 | 22.24–24.76 |
| OpenGL | kitty | images/s (PNG/parser ACK) | 1909.38 | 913.81 | -52.1% | 1322.33–2012.33 | 896.60–1874.66 |
| WebGpu | ascii | MiB/s | 23.05 | 22.89 | -0.7% | 22.59–23.22 | 21.24–23.24 |
| WebGpu | ansi | MiB/s | 23.81 | 22.98 | -3.5% | 23.18–24.62 | 22.92–23.61 |
| WebGpu | unicode | MiB/s | 26.30 | 26.81 | +2.0% | 24.49–29.12 | 26.76–28.11 |
| WebGpu | kitty | images/s (PNG/parser ACK) | 1878.65 | 2008.91 | +6.9% | 1789.64–1942.43 | 1911.32–2106.39 |

## WSL worker resources

CPU excludes startup/settling and is for the WSL producer only. RSS is sampled approximately every 20 ms;
it excludes the Windows GUI process and GPU VRAM.

| Backend | Workload | Build | Worker CPU s | One-core CPU % | Peak RSS MiB |
|---|---|---|---:|---:|---:|
| OpenGL | idle | baseline | 0.0000 | 0.00 | 16.1 |
| OpenGL | idle | candidate | 0.0000 | 0.00 | 15.9 |
| OpenGL | ascii | baseline | 0.0800 | 11.56 | 16.1 |
| OpenGL | ascii | candidate | 0.0700 | 9.28 | 16.1 |
| OpenGL | ansi | baseline | 0.1100 | 14.19 | 15.8 |
| OpenGL | ansi | candidate | 0.1000 | 14.02 | 15.9 |
| OpenGL | unicode | baseline | 0.0900 | 12.99 | 15.9 |
| OpenGL | unicode | candidate | 0.1000 | 13.62 | 15.9 |
| OpenGL | kitty | baseline | 0.0900 | 55.24 | 16.4 |
| OpenGL | kitty | candidate | 0.1000 | 40.77 | 16.4 |
| WebGpu | idle | baseline | 0.0000 | 0.00 | 16.1 |
| WebGpu | idle | candidate | 0.0000 | 0.00 | 16.1 |
| WebGpu | ascii | baseline | 0.0900 | 12.60 | 16.0 |
| WebGpu | ascii | candidate | 0.0900 | 12.27 | 15.8 |
| WebGpu | ansi | baseline | 0.1000 | 14.44 | 15.9 |
| WebGpu | ansi | candidate | 0.1100 | 15.40 | 16.1 |
| WebGpu | unicode | baseline | 0.0900 | 15.76 | 16.1 |
| WebGpu | unicode | candidate | 0.1000 | 16.35 | 15.9 |
| WebGpu | kitty | baseline | 0.0900 | 55.18 | 16.3 |
| WebGpu | kitty | candidate | 0.0900 | 55.24 | 16.3 |
