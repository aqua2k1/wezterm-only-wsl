//! Windows-only startup for a single local WSL session.
//!
//! This intentionally does not use the GUI socket, mux listener, client
//! discovery, or configured remote domains.  The regular mux remains in use
//! as the internal pane/tab representation for the first phase of this mode.

use wezterm_gui_subcommands::StartCommand;

pub(crate) fn run(opts: StartCommand, requested_domain_name: Option<String>) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        run_windows(opts, requested_domain_name)
    }

    #[cfg(not(windows))]
    {
        let _ = opts;
        let _ = requested_domain_name;
        anyhow::bail!(
            "single-session WSL startup is Windows-only; build and run it on Windows with WSL installed"
        )
    }
}

// Keep the implementation type-checked on all hosts; `run` only calls it on
// Windows, where LocalDomain wraps the command with wsl.exe through ConPTY.
fn run_windows(opts: StartCommand, requested_domain_name: Option<String>) -> anyhow::Result<()> {
    if let Some(cls) = opts.class.as_ref() {
        crate::set_window_class(cls);
    }
    if let Some(pos) = opts.position.as_ref() {
        crate::set_window_position(pos.clone());
    }

    // Do not inherit an endpoint published by another wezterm instance.  This
    // mode deliberately has no mux listener or RPC endpoint of its own.
    std::env::remove_var("WEZTERM_UNIX_SOCKET");

    let config = config::configuration();
    let requested_domain_name = opts.domain.as_deref().or(requested_domain_name.as_deref());
    let wsl_domain = select_wsl_domain(&config, requested_domain_name)?;
    let domain: std::sync::Arc<dyn mux::domain::Domain> =
        std::sync::Arc::new(mux::domain::LocalDomain::new_wsl(wsl_domain.clone())?);

    // Build an empty/default command here rather than using the Windows
    // default program.  LocalDomain::new_wsl then applies the WSL domain's
    // default program/cwd and wraps the command with wsl.exe.
    let cmd = build_command(&opts)?;

    setup_mux(domain.clone());
    let _mux = mux::Mux::get();

    // Kitty graphics may use the blob lease store even though this mode does
    // not start a mux server, so retain the normal GUI storage setup.
    wezterm_blob_leases::register_storage(std::sync::Arc::new(
        wezterm_blob_leases::simple_tempdir::SimpleTempDir::new_in(&*config::CACHE_DIR)?,
    ))?;

    let gui = crate::frontend::try_new()?;
    let activity = mux::activity::Activity::new();
    promise::spawn::spawn(async move {
        if let Err(err) = spawn_initial_wsl_pane(domain, cmd).await {
            crate::terminate_with_error(err);
        }
        drop(activity);
    })
    .detach();

    // In particular, do not emit gui-startup here: a callback can spawn a
    // pane before the WSL pane and would violate the single-session contract.
    crate::maybe_show_configuration_error_window();
    gui.run_forever()
}

fn select_wsl_domain(
    config: &config::ConfigHandle,
    requested_name: Option<&str>,
) -> anyhow::Result<config::WslDomain> {
    select_wsl_domain_from(
        config.wsl_domains(),
        requested_name,
        config.default_domain.as_deref(),
    )
}

fn select_wsl_domain_from(
    domains: Vec<config::WslDomain>,
    requested_name: Option<&str>,
    default_name: Option<&str>,
) -> anyhow::Result<config::WslDomain> {
    anyhow::ensure!(
        !domains.is_empty(),
        "no WSL distributions are configured or installed"
    );

    if let Some(name) = requested_name {
        return domains
            .into_iter()
            .find(|domain| domain.name == name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "domain `{name}` is not a configured WSL domain; single-session WSL startup cannot start a local or remote non-WSL domain"
                )
            });
    }

    // Respect a WSL default_domain when one is configured.  If the normal
    // default is local (or another domain), use the first configured WSL
    // domain instead of silently falling back to a Windows shell.
    if let Some(name) = default_name {
        if let Some(domain) = domains.iter().find(|domain| domain.name == name) {
            return Ok(domain.clone());
        }
    }

    Ok(domains
        .into_iter()
        .next()
        .expect("checked non-empty WSL domains"))
}

fn build_command(opts: &StartCommand) -> anyhow::Result<Option<portable_pty::CommandBuilder>> {
    if opts.prog.is_empty() && opts.cwd.is_none() {
        return Ok(None);
    }

    let mut cmd = if opts.prog.is_empty() {
        portable_pty::CommandBuilder::new_default_prog()
    } else {
        portable_pty::CommandBuilder::from_argv(opts.prog.clone())
    };

    if let Some(cwd) = &opts.cwd {
        // This is passed to `wsl.exe --cd`, not a Windows child cwd. In
        // particular, do not turn `~` or `/home/...` into a host drive path.
        cmd.cwd(cwd.as_os_str());
    }

    // This is also removed from the base environment in run_windows; remove
    // it from an explicitly supplied command as a second line of defence.
    cmd.env_remove("WEZTERM_UNIX_SOCKET");
    Ok(Some(cmd))
}

fn setup_mux(domain: std::sync::Arc<dyn mux::domain::Domain>) {
    let mux = std::sync::Arc::new(mux::Mux::new(Some(domain)));
    mux::Mux::set_mux(&mux);

    let client_id = std::sync::Arc::new(mux::client::ClientId::new());
    mux.register_client(client_id.clone());
    mux.replace_identity(Some(client_id));
    // The internal mux requires one workspace wrapper, but this mode never
    // creates or switches any additional workspaces.
    mux.set_active_workspace(mux::DEFAULT_WORKSPACE);
}

async fn spawn_initial_wsl_pane(
    domain: std::sync::Arc<dyn mux::domain::Domain>,
    cmd: Option<portable_pty::CommandBuilder>,
) -> anyhow::Result<()> {
    let mux = mux::Mux::get();
    let config = config::configuration();
    config.update_ulimit()?;

    // A single empty window is the internal wrapper used by the GUI; the
    // single-session mux feature rejects subsequent pane/tab/window spawns.
    let window_id = *mux.new_empty_window(None, None);
    domain.attach(Some(window_id)).await?;

    let dpi = config.dpi.unwrap_or_else(|| ::window::default_dpi());
    let tab = domain
        .spawn(
            config.initial_size(dpi as u32, Some(crate::cell_pixel_dims(&config, dpi)?)),
            cmd,
            None,
            window_id,
        )
        .await?;

    let mut window = mux
        .get_window_mut(window_id)
        .ok_or_else(|| anyhow::anyhow!("failed to get mux window id {window_id}"))?;
    if let Some(tab_idx) = window.get_tab_idx_for_id(tab.tab_id()) {
        window.set_active_tab_idx_without_saving(tab_idx);
    }

    // Do not emit gui-attached either: Lua callbacks are not part of this
    // first-phase single-session startup path.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(name: &str, distribution: &str) -> config::WslDomain {
        config::WslDomain {
            name: name.to_string(),
            distribution: Some(distribution.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn wsl_working_directory_is_not_resolved_on_the_host() {
        for cwd in ["~", "/home/test"] {
            let command = build_command(&StartCommand {
                cwd: Some(cwd.into()),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
            assert_eq!(
                command.get_cwd().map(|dir| dir.as_os_str()),
                Some(std::ffi::OsStr::new(cwd))
            );
        }
    }

    #[test]
    fn requested_domain_must_be_wsl() {
        let domains = vec![domain("WSL:Ubuntu", "Ubuntu")];
        let selected = select_wsl_domain_from(domains, Some("WSL:Ubuntu"), None).unwrap();
        assert_eq!(selected.name, "WSL:Ubuntu");
        assert_eq!(selected.distribution.as_deref(), Some("Ubuntu"));

        assert!(
            select_wsl_domain_from(vec![domain("WSL:Ubuntu", "Ubuntu")], Some("local"), None,)
                .is_err()
        );
    }

    #[test]
    fn default_domain_is_used_before_first_wsl_domain() {
        let selected = select_wsl_domain_from(
            vec![domain("WSL:Ubuntu", "Ubuntu"), domain("WSL:Arch", "Arch")],
            None,
            Some("WSL:Arch"),
        )
        .unwrap();
        assert_eq!(selected.name, "WSL:Arch");
    }

    #[test]
    fn local_default_falls_back_to_first_wsl_domain() {
        let selected =
            select_wsl_domain_from(vec![domain("WSL:Ubuntu", "Ubuntu")], None, Some("local"))
                .unwrap();
        assert_eq!(selected.name, "WSL:Ubuntu");
    }
}
