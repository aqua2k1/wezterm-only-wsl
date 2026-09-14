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
```

Run `target/release/wezterm-gui.exe` directly. The `wezterm` CLI launcher and
standalone mux-server crates have been deleted. The default workspace build
selects `wezterm-gui`; upstream CI/deployment scripts referring to the removed
programs are not supported packaging entry points for this fork.

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
  `wezterm-mux-server-impl`, `codec`, `wezterm-ssh`, and `async_ossl` crates.
- SSH agent forwarding, serial PTY, SSH/TLS/serial configuration schemas,
  and tmux control-mode integration (ordinary tmux inside WSL still works).
- GUI update checker.
- Battery, Git plugin manager, and SSH Lua API crates and registration.
- GUI session-management actions, command palette, pane selector, secondary
  session spawning, and Lua session-creation/mutation APIs.
- Multi-session split/move implementation paths in the local mux layer.

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
SSH/TLS/serial fields or Lua APIs must be updated; they are not silently accepted.

Kitty keyboard and graphics code is retained, including shared image/cache and
GPU rendering paths. Sixel/iTerm image handlers have not yet been removed.
Windows and WSL have different filesystem/shared-memory namespaces: retaining
Kitty handlers does not implement cross-boundary file or shared-memory
transport. Test supported transfers explicitly; do not promise that all transfer
modes work across this boundary.

The Windows build script still packages ConPTY, ANGLE, and Mesa resources. Do
not remove ConPTY assets or GPU fallback DLLs before choosing and testing the
rendering strategy. Removing source for other platforms does not improve the
Windows runtime path and is not a priority.

## Validation before distribution

The changes have been checked with Linux unit tests and a Windows GNU
cross-target `cargo check`. Neither is Windows runtime or MSVC release-build
validation. No distributable Windows binary or performance result is claimed.

A Linux `cargo check` is not Windows runtime validation. On the target Windows
machine, verify:

- Default and explicitly selected WSL distro startup; failed/missing distro.
- Close, child exit, resize, DPI changes, and repeated independent launches.
- No GUI RPC socket/listener or standalone server startup.
- Tab/split/workspace keybindings, menus, and Lua cannot create extra sessions;
  copy/search overlays still work.
- IME, clipboard, mouse reporting, shell, Neovim, and ordinary tmux operation.
- Kitty keyboard negotiation, modifiers, press/repeat/release events.
- Kitty graphics transfer, placement, scrolling, deletion, and repeated updates.
- OpenGL versus WebGPU: text throughput, mixed text/image workload, input latency,
  idle CPU, memory/VRAM, and startup time. Choose a backend after comparison.

Measure WSL cold startup separately from GUI startup. Preserve glyph/shape/image
caches until profiling justifies changes. No speedup percentage is claimed.
