# Windows / WSL single-session fork

This tree is specialized for a native Windows GUI running one local WSL
session through ConPTY. It is not a GUI running inside WSL, an SSH connection,
or an upstream-compatible opt-in feature. Removed behavior is not retained
behind a Cargo feature switch.

## Build

Use a Windows Rust/MSVC build environment with the project's native build
prerequisites and initialized submodules:

```powershell
git submodule update --init --recursive
cargo build --release -p wezterm-gui --no-default-features
bash ci/windows-zip.sh
```

The only supported artifact is the portable Windows ZIP containing
`wezterm-gui.exe`, ConPTY, ANGLE and Mesa fallback resources. There is no
Linux/Nix/macOS package, Inno installer, winget manifest, CLI launcher, or
standalone mux-server artifact. Run `target/release/wezterm-gui.exe` directly
when testing an unpacked build.

`--no-default-features` omits bundled fonts. Install suitable fonts on Windows,
or add `--features vendored-fonts` to the build command. Windows font fallback,
IME, and Kitty rendering infrastructure remain available.

Example user configuration (replace the distro and font names with installed
ones):

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

config.wsl_domains = {
  {
    name = 'WSL:Ubuntu',
    distribution = 'Ubuntu',
    default_cwd = '~',
  },
}
config.default_domain = 'WSL:Ubuntu'
config.font = wezterm.font 'Consolas'
config.enable_kitty_keyboard = true
config.enable_kitty_graphics = true

return config
```

Explicit domains avoid automatic WSL distro enumeration. The selected WSL
definition is pinned for the session, so config reload cannot replace it with
a Windows shell or exec domain during startup. Kitty keyboard support still
observes application protocol negotiation; it does not force enhanced encoding
on applications that do not request it.

## Removed code

- GUI SSH/serial/connect and normal remote startup/discovery/listener paths.
- `wezterm`, `wezterm-client`, `wezterm-mux-server`,
  `wezterm-mux-server-impl`, `codec`, `wezterm-ssh`, `async_ossl`, and
  `wezterm-uds` crates.
- SSH agent forwarding, serial PTY, SSH/TLS/serial configuration schemas,
  ExecDomain/Lua exec-domain callbacks, and tmux control-mode integration
  (ordinary tmux inside WSL still works).
- Unix-socket domain configuration, daemon/server settings, Windows context-menu
  integration, and obsolete
  `unix_domains`, `default_mux_server_domain`,
  `ratelimit_mux_line_prefetches_per_second`, and `mux_env_remove` fields.
- GUI update checker, iTerm file-download delivery, and its Downloads-folder
  bridge. Kitty graphics remains available; this is not Kitty image storage.
- Battery, Git plugin manager, and SSH Lua API crates and registration.
- GUI session-management actions, command palette, pane selector, secondary
  session spawning, and Lua session-creation/mutation APIs.
- The launcher overlay, tab navigator and tab-move helpers, workspace switcher,
  native-window reassignment notifications, and dead command metadata. Each window now captures its
  fixed session ID directly, without locking an ID mutex on every mux event.
- Multi-session split/move implementation paths in the local mux layer.
- X11, Wayland, macOS window backends; Unix terminal and Unix PTY backends.
  Windows builds now use only the native window and ConPTY implementations.

One internal pane/tab/window wrapper remains for terminal state, I/O lifecycle,
resizing, and rendering. Tabs, splits, workspaces, and extra GUI windows within
the process are not supported. Start another process for a second session.
There is no background session server, GUI RPC listener, or reconnect/reattach.
The process exits when its only session is gone, regardless of the legacy
`quit_when_all_windows_are_closed` setting. Removed default session-management
shortcuts are not installed; explicit user bindings to removed actions are
rejected. `--cwd` is forwarded as a WSL directory, without host-path expansion.
Lua `gui-startup` and `gui-attached` callbacks are not emitted by this startup
path. Application file-open commands are rejected, not injected into the
current shell or foreground program.

## Remaining work and compatibility

This is **not** yet a complete binary-size reduction. Lua remains the config
engine, and both renderers remain until Windows performance measurements select
one. Removing SSH and the plugin manager does not remove the `git2` build
dependency used by version generation or its possible HTTPS/OpenSSL dependencies.
Some internal multi-session data structures and unused UI modules/configuration
fields remain and need further extraction. Old configurations using removed
SSH/TLS/serial/Unix-domain and server-only fields or Lua APIs must be updated;
these interfaces are no longer supported.

Kitty keyboard and graphics code is retained, including shared image/cache and
GPU rendering paths. Sixel/iTerm image handlers have not yet been removed.
Windows and WSL have different filesystem/shared-memory namespaces: retaining
Kitty handlers does not implement cross-boundary file or shared-memory
transport. Test supported transfers explicitly; do not promise that all transfer
modes work across this boundary.

The Windows build/package path retains ConPTY, ANGLE (`prefer_egl`) and Mesa
fallback resources. Do not remove those DLLs before Windows runtime testing.
Other platform package definitions have been deleted; removing their source
code is separate from this packaging cleanup.

## Validation before distribution

The Windows-only backend crop has been checked with platform-neutral library
checks and a Windows GNU cross-target `cargo check`; the Linux GUI target is no
longer supported. The WSL-native harness in `tools/perf/` can launch
both native Windows renderers through WSL interop; it does not invoke
PowerShell/cmd.exe. It measures WSL/ConPTY throughput and WSL worker CPU/RSS,
not Windows GUI CPU, GPU VRAM, or present timing. Neither this harness nor the
cross-check is MSVC release-build validation.

The Windows GNU cross-check is not Windows runtime or MSVC validation. On the
target Windows machine, verify:

- Default and explicitly selected WSL distro startup; failed/missing distro.
- Close, child exit, resize, DPI changes, and repeated independent launches.
- No GUI RPC socket/listener or standalone server startup.
- Tab/split/workspace keybindings, menus, and Lua cannot create extra sessions;
  copy/search overlays still work.
- IME, clipboard, mouse reporting, shell, Neovim, and ordinary tmux operation.
- Kitty keyboard negotiation, modifiers, press/repeat/release events.
- Kitty graphics transfer, placement, scrolling, deletion, and repeated updates.
- OpenGL versus WebGPU: text throughput, mixed text/image workload, input latency,
  idle CPU, memory/VRAM, and startup time. The WSL harness covers throughput,
  startup acknowledgment, and WSL worker CPU/RSS; native Windows tooling is
  still required for GUI CPU, VRAM, present timing, and input-to-photon latency.
  Choose a backend only after those measurements.

Measure WSL cold startup separately from GUI startup. The latest Windows-only backend comparison is recorded in
`docs-internal/windows-wsl-performance-platform-20260915.md` (and its JSON
companion). It compares the platform-crop baseline with the latest build: the
executable changes from 68.47 MiB to 68.46 MiB, and Kitty throughput changes by
+1.2% on OpenGL and +1.9% on WebGPU. Text throughput remains within roughly
±6.5%. This is WSL/ConPTY data, not a Windows GUI or GPU benchmark, so it does
not select OpenGL over WebGPU. Preserve glyph/shape/image caches until native
profiling justifies changes. No speedup percentage is claimed beyond the stated
harness measurements.
