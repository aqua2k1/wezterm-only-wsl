pub mod ringlog;
pub use ringlog::setup_logger;

pub fn set_wezterm_executable() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            std::env::set_var("WEZTERM_EXECUTABLE_DIR", dir);
        }
        std::env::set_var("WEZTERM_EXECUTABLE", exe);
    }
}

/// If LANG isn't set in the environment, make an attempt at setting
/// it to a UTF-8 version of the current locale known to NSLocale.
#[cfg(target_os = "macos")]
pub fn set_lang_from_locale() {
    use objc2_foundation::{NSLocale, NSString};

    fn lang_is_set() -> bool {
        match std::env::var_os("LANG") {
            None => false,
            Some(lang) => !lang.is_empty(),
        }
    }

    if !lang_is_set() {
        unsafe fn nsstring_to_str<'a>(ns: &NSString) -> &'a str {
            let data = ns.UTF8String() as *const u8;
            let len = ns.len();
            let bytes = std::slice::from_raw_parts(data, len);
            std::str::from_utf8_unchecked(bytes)
        }

        unsafe {
            let locale = NSLocale::autoupdatingCurrentLocale();
            let lang_code_obj = locale.languageCode();
            let lang_code = nsstring_to_str(&lang_code_obj);

            #[allow(deprecated)]
            let candidate = if let Some(country_code_obj) = locale.countryCode() {
                let country_code = nsstring_to_str(&country_code_obj);
                format!("{}_{}.UTF-8", lang_code, country_code)
            } else {
                format!("{}.UTF-8", lang_code)
            };

            let candidate_cstr =
                std::ffi::CString::new(candidate.as_bytes()).expect("make cstr from str");

            // If this looks like a working locale then export it to
            // the environment so that our child processes inherit it.
            let old = libc::setlocale(libc::LC_CTYPE, std::ptr::null());
            if !libc::setlocale(libc::LC_CTYPE, candidate_cstr.as_ptr()).is_null() {
                std::env::set_var("LANG", &candidate);
            } else {
                log::debug!("setlocale({}) failed, fall back to en_US.UTF-8", candidate);
                std::env::set_var("LANG", "en_US.UTF-8");
            }
            libc::setlocale(libc::LC_CTYPE, old);
        }
    }
}

fn register_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let payload = payload.downcast_ref::<&str>().unwrap_or(&"!?");
        let bt = backtrace::Backtrace::new();
        if let Some(loc) = info.location() {
            log::error!(
                "panic at {}:{}:{} - {}\n{:?}",
                loc.file(),
                loc.line(),
                loc.column(),
                payload,
                bt
            );
        } else {
            log::error!("panic - {}\n{:?}", payload, bt);
        }
        default_hook(info);
    }));
}

fn register_lua_modules() {
    for func in [
        color_funcs::register,
        termwiz_funcs::register,
        logging::register,
        mux_lua::register,
        procinfo_funcs::register,
        filesystem::register,
        serde_funcs::register,
        spawn_funcs::register,
        share_data::register,
        time_funcs::register,
        url_funcs::register,
    ] {
        config::lua::add_context_setup_func(func);
    }
}

pub fn bootstrap() {
    config::assign_version_info(
        wezterm_version::wezterm_version(),
        wezterm_version::wezterm_target_triple(),
    );
    setup_logger();
    register_panic_hook();

    set_wezterm_executable();

    #[cfg(target_os = "macos")]
    set_lang_from_locale();

    register_lua_modules();

    // Remove this env var to avoid weirdness with some vim configurations.
    // wezterm never sets WINDOWID and we don't want to inherit it from a
    // parent process.
    std::env::remove_var("WINDOWID");
    // Avoid vte shell integration kicking in if someone started
    // wezterm or the mux server from inside gnome terminal.
    // <https://github.com/wezterm/wezterm/issues/2237>
    std::env::remove_var("VTE_VERSION");

    // Sice folks don't like to reboot or sign out if they `chsh`,
    // SHELL may be stale. Rather than using a stale value, unset
    // it so that pty::CommandBuilder::get_shell will resolve the
    // shell from the password database instead.
    std::env::remove_var("SHELL");
}
