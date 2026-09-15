//! A Domain represents an instance of a multiplexer.
//! For example, the gui frontend has its own domain,
//! and we can connect to a domain hosted by a mux server
//! that may be local, running "remotely" inside a WSL
//! container or actually remote, running on the other end
//! of an ssh session somewhere.

use crate::localpane::LocalPane;
use crate::pane::{alloc_pane_id, Pane, PaneId};
use crate::tab::Tab;
use crate::window::WindowId;
use crate::Mux;
use anyhow::{bail, Context, Error};
use async_trait::async_trait;
use config::{configuration, WslDomain};
use downcast_rs::{impl_downcast, Downcast};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, ExitStatus, MasterPty, PtySize, PtySystem};
use std::ffi::OsString;
use std::io::Write;
use std::sync::Arc;
use wezterm_term::TerminalSize;

static DOMAIN_ID: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(0);
pub type DomainId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainState {
    Detached,
    Attached,
}

pub fn alloc_domain_id() -> DomainId {
    DOMAIN_ID.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed)
}

#[async_trait(?Send)]
pub trait Domain: Downcast + Send + Sync {
    /// Spawn a new command within this domain
    async fn spawn(
        &self,
        size: TerminalSize,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
        window: WindowId,
    ) -> anyhow::Result<Arc<Tab>> {
        let pane = self
            .spawn_pane(size, command, command_dir)
            .await
            .context("spawn")?;

        let tab = Arc::new(Tab::new(&size));
        tab.assign_pane(&pane);

        let mux = Mux::get();
        mux.add_tab_and_active_pane(&tab)?;
        mux.add_tab_to_window(&tab, window)?;

        Ok(tab)
    }

    async fn spawn_pane(
        &self,
        size: TerminalSize,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
    ) -> anyhow::Result<Arc<dyn Pane>>;

    /// Returns false if the `spawn` method will never succeed.
    /// There are some internal placeholder domains that are
    /// pre-created with local UI that we do not want to allow
    /// to show in the launcher/menu as launchable items.
    fn spawnable(&self) -> bool {
        true
    }

    /// Returns true if the `detach` method can be used
    /// to detach the domain, preserving the associated
    /// panes, or false if the `detach` method will never
    /// succeed
    fn detachable(&self) -> bool;

    /// Returns the domain id, which is useful for obtaining
    /// a handle on the domain later.
    fn domain_id(&self) -> DomainId;

    /// Returns the name of the domain.
    /// Should be a short identifier.
    fn domain_name(&self) -> &str;

    /// Returns a label describing the domain.
    async fn domain_label(&self) -> String {
        self.domain_name().to_string()
    }

    /// Re-attach to any tabs that might be pre-existing in this domain
    async fn attach(&self, window_id: Option<WindowId>) -> anyhow::Result<()>;

    /// Detach all tabs
    fn detach(&self) -> anyhow::Result<()>;

    /// Indicates the state of the domain
    fn state(&self) -> DomainState;
}
impl_downcast!(Domain);

#[cfg(test)]
mod single_session_tests {
    use super::*;

    #[test]
    fn wsl_definition_is_pinned_without_configuration_lookup() {
        let domain = LocalDomain::new_wsl(WslDomain {
            name: "WSL:Test".to_string(),
            distribution: Some("TestDistro".to_string()),
            default_prog: Some(vec!["/bin/bash".to_string()]),
            ..Default::default()
        })
        .unwrap();
        let selected = domain.resolve_wsl_domain();
        assert_eq!(selected.name, "WSL:Test");
        assert_eq!(selected.distribution.as_deref(), Some("TestDistro"));
        assert_eq!(selected.default_prog, Some(vec!["/bin/bash".to_string()]));
    }
}

pub struct LocalDomain {
    pty_system: Mutex<Box<dyn PtySystem + Send>>,
    id: DomainId,
    name: String,
    pinned_wsl: WslDomain,
}

impl LocalDomain {
    pub fn new_wsl(wsl: WslDomain) -> Result<Self, Error> {
        let id = alloc_domain_id();
        let name = wsl.name.clone();
        Ok(Self {
            pty_system: Mutex::new(native_pty_system()),
            id,
            name,
            pinned_wsl: wsl,
        })
    }

    fn resolve_wsl_domain(&self) -> &WslDomain {
        &self.pinned_wsl
    }

    #[cfg(unix)]
    fn is_conpty(&self) -> bool {
        false
    }

    #[cfg(windows)]
    fn is_conpty(&self) -> bool {
        let pty_system = self.pty_system.lock();
        let pty_system: &dyn PtySystem = &**pty_system;
        pty_system
            .downcast_ref::<portable_pty::win::conpty::ConPtySystem>()
            .is_some()
    }

    async fn fixup_command(&self, cmd: &mut CommandBuilder) -> anyhow::Result<()> {
        let wsl = self.resolve_wsl_domain();
        let mut args: Vec<OsString> = cmd.get_argv().clone();

        if args.is_empty() {
            if let Some(def_prog) = &wsl.default_prog {
                for arg in def_prog {
                    args.push(arg.into());
                }
            }
        }

        let mut argv: Vec<OsString> = vec![
            "wsl.exe".into(),
            "--distribution".into(),
            wsl.distribution
                .as_deref()
                .unwrap_or(wsl.name.as_str())
                .into(),
        ];

        if let Some(cwd) = cmd.get_cwd() {
            argv.push("--cd".into());
            argv.push(cwd.into());
        }

        if let Some(user) = &wsl.username {
            argv.push("--user".into());
            argv.push(user.into());
        }

        if !args.is_empty() {
            argv.push("--exec".into());
            argv.extend(args);
        }

        cmd.clear_cwd();
        *cmd.get_argv_mut() = argv;
        Ok(())
    }

    async fn build_command(
        &self,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
        pane_id: PaneId,
    ) -> anyhow::Result<CommandBuilder> {
        let config = configuration();

        let wsl = self.resolve_wsl_domain();
        let default_prog = wsl.default_prog.as_ref().or(config.default_prog.as_ref());

        let mut cmd = match command {
            Some(mut cmd) => {
                config.apply_cmd_defaults(&mut cmd, default_prog, config.default_cwd.as_ref());
                cmd
            }
            None => config.build_prog(
                None,
                default_prog,
                wsl.default_cwd.as_ref().or(config.default_cwd.as_ref()),
            )?,
        };
        if let Some(dir) = command_dir {
            cmd.cwd(dir);
        }
        cmd.env_remove("WEZTERM_UNIX_SOCKET");
        cmd.env("WEZTERM_PANE", pane_id.to_string());
        self.fixup_command(&mut cmd).await?;
        Ok(cmd)
    }
}

/// Allows sharing the writer between the Pane and the Terminal.
/// This could potentially be eliminated in the future if we can
/// teach the Pane impl to reference the writer in the Termninal,
/// but the Pane trait returns a RefMut and that makes it a bit
/// awkward at the moment.
#[derive(Clone)]
pub(crate) struct WriterWrapper {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl WriterWrapper {
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}

impl std::io::Write for WriterWrapper {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.lock().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.lock().flush()
    }
}

/// Wraps the underlying pty; we use this as a marker for when
/// the spawn attempt failed in order to hold the pane open
pub(crate) struct FailedSpawnPty {
    inner: Mutex<Box<dyn MasterPty>>,
}

impl portable_pty::MasterPty for FailedSpawnPty {
    fn resize(&self, new_size: PtySize) -> anyhow::Result<()> {
        self.inner.lock().resize(new_size)
    }
    fn get_size(&self) -> anyhow::Result<PtySize> {
        self.inner.lock().get_size()
    }
    fn try_clone_reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send + 'static>> {
        self.inner.lock().try_clone_reader()
    }
    fn take_writer(&self) -> anyhow::Result<Box<dyn std::io::Write + Send + 'static>> {
        self.inner.lock().take_writer()
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<i32> {
        None
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<std::os::fd::RawFd> {
        None
    }

    #[cfg(unix)]
    fn tty_name(&self) -> Option<std::path::PathBuf> {
        None
    }
}

/// A fake child process for the case where the spawn attempt
/// failed. It reports as immediately terminated.
#[derive(Debug)]
pub(crate) struct FailedProcessSpawn {}

impl portable_pty::Child for FailedProcessSpawn {
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        Ok(Some(ExitStatus::with_exit_code(1)))
    }

    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        Ok(ExitStatus::with_exit_code(1))
    }

    fn process_id(&self) -> Option<u32> {
        None
    }

    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        None
    }
}

impl portable_pty::ChildKiller for FailedProcessSpawn {
    fn kill(&mut self) -> std::io::Result<()> {
        Ok(())
    }
    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        Box::new(FailedProcessSpawn {})
    }
}

#[async_trait(?Send)]
impl Domain for LocalDomain {
    async fn spawn_pane(
        &self,
        size: TerminalSize,
        command: Option<CommandBuilder>,
        command_dir: Option<String>,
    ) -> anyhow::Result<Arc<dyn Pane>> {
        // Serialize the check through registration, including the async command
        // builder, so concurrent requests cannot launch a second child.
        let session_mux = Mux::get();
        let _spawn_guard = session_mux.acquire_single_session_spawn_lock().await;
        session_mux.ensure_single_session_spawn_allowed()?;

        let pane_id = alloc_pane_id();
        let cmd = self
            .build_command(command, command_dir, pane_id)
            .await
            .context("build_command")?;
        let pair = self
            .pty_system
            .lock()
            .openpty(crate::terminal_size_to_pty_size(size)?)?;

        let command_line = cmd
            .as_unix_command_line()
            .unwrap_or_else(|err| format!("error rendering command line: {:?}", err));
        let command_description = format!(
            "\"{}\" in domain \"{}\"",
            if command_line.is_empty() {
                cmd.get_shell()
            } else {
                command_line
            },
            self.name
        );
        let child_result = pair.slave.spawn_command(cmd);
        let mut writer = WriterWrapper::new(pair.master.take_writer()?);

        let mut terminal = wezterm_term::Terminal::new(
            size,
            std::sync::Arc::new(config::TermConfig::new()),
            "WezTerm",
            config::wezterm_version(),
            Box::new(writer.clone()),
        );
        if self.is_conpty() {
            terminal.enable_conpty_quirks();
        }

        let pane: Arc<dyn Pane> = match child_result {
            Ok(child) => Arc::new(LocalPane::new(
                pane_id,
                terminal,
                child,
                pair.master,
                Box::new(writer),
                self.id,
                command_description,
            )),
            Err(err) => {
                // Show the error to the user in the new pane
                write!(writer, "{err:#}").ok();

                // and return a dummy pane that has exited
                Arc::new(LocalPane::new(
                    pane_id,
                    terminal,
                    Box::new(FailedProcessSpawn {}),
                    Box::new(FailedSpawnPty {
                        inner: Mutex::new(pair.master),
                    }),
                    Box::new(writer),
                    self.id,
                    command_description,
                ))
            }
        };

        let mux = Mux::get();
        mux.add_pane(&pane)?;

        Ok(pane)
    }

    fn domain_id(&self) -> DomainId {
        self.id
    }

    fn domain_name(&self) -> &str {
        &self.name
    }

    async fn domain_label(&self) -> String {
        self.pinned_wsl
            .distribution
            .clone()
            .unwrap_or_else(|| self.name.clone())
    }

    async fn attach(&self, _window_id: Option<WindowId>) -> anyhow::Result<()> {
        Ok(())
    }

    fn detachable(&self) -> bool {
        false
    }

    fn detach(&self) -> anyhow::Result<()> {
        bail!("detach not implemented for LocalDomain");
    }

    fn state(&self) -> DomainState {
        DomainState::Attached
    }
}
