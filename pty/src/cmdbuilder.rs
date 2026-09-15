#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::path::Path;

/// Used to deal with Windows having case-insensitive environment variables.
#[derive(Clone, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
struct EnvEntry {
    /// Whether or not this environment variable came from the base environment,
    /// as opposed to having been explicitly set by the caller.
    is_from_base_env: bool,

    /// For case-insensitive platforms, the environment variable key in its preferred casing.
    preferred_key: OsString,

    /// The environment variable value.
    value: OsString,
}

impl EnvEntry {
    fn map_key(k: OsString) -> OsString {
        if cfg!(windows) {
            // Best-effort lowercase transformation of an os string
            match k.to_str() {
                Some(s) => s.to_lowercase().into(),
                None => k,
            }
        } else {
            k
        }
    }
}

#[allow(unused_mut)]
fn get_base_env() -> BTreeMap<OsString, EnvEntry> {
    let mut env: BTreeMap<OsString, EnvEntry> = std::env::vars_os()
        .map(|(key, value)| {
            (
                EnvEntry::map_key(key.clone()),
                EnvEntry {
                    is_from_base_env: true,
                    preferred_key: key,
                    value,
                },
            )
        })
        .collect();

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
        use winreg::enums::{RegType, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::types::FromRegValue;
        use winreg::{RegKey, RegValue};

        fn reg_value_to_string(value: &RegValue) -> anyhow::Result<OsString> {
            match value.vtype {
                RegType::REG_EXPAND_SZ => {
                    let src = unsafe {
                        std::slice::from_raw_parts(
                            value.bytes.as_ptr() as *const u16,
                            value.bytes.len() / 2,
                        )
                    };
                    let size =
                        unsafe { ExpandEnvironmentStringsW(src.as_ptr(), std::ptr::null_mut(), 0) };
                    let mut buf = vec![0u16; size as usize + 1];
                    unsafe {
                        ExpandEnvironmentStringsW(src.as_ptr(), buf.as_mut_ptr(), buf.len() as u32)
                    };

                    let mut buf = buf.as_slice();
                    while let Some(0) = buf.last() {
                        buf = &buf[0..buf.len() - 1];
                    }
                    Ok(OsString::from_wide(buf))
                }
                _ => Ok(OsString::from_reg_value(value)?),
            }
        }

        if let Ok(sys_env) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey("System\\CurrentControlSet\\Control\\Session Manager\\Environment")
        {
            for res in sys_env.enum_values() {
                if let Ok((name, value)) = res {
                    if name.to_ascii_lowercase() == "username" {
                        continue;
                    }
                    if let Ok(value) = reg_value_to_string(&value) {
                        log::trace!("adding SYS env: {:?} {:?}", name, value);
                        env.insert(
                            EnvEntry::map_key(name.clone().into()),
                            EnvEntry {
                                is_from_base_env: true,
                                preferred_key: name.into(),
                                value,
                            },
                        );
                    }
                }
            }
        }

        if let Ok(sys_env) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Environment") {
            for res in sys_env.enum_values() {
                if let Ok((name, value)) = res {
                    if let Ok(value) = reg_value_to_string(&value) {
                        // Merge the system and user paths together
                        let value = if name.to_ascii_lowercase() == "path" {
                            match env.get(&EnvEntry::map_key(name.clone().into())) {
                                Some(entry) => {
                                    let mut result = OsString::new();
                                    result.push(&entry.value);
                                    result.push(";");
                                    result.push(&value);
                                    result
                                }
                                None => value,
                            }
                        } else {
                            value
                        };

                        log::trace!("adding USER env: {:?} {:?}", name, value);
                        env.insert(
                            EnvEntry::map_key(name.clone().into()),
                            EnvEntry {
                                is_from_base_env: true,
                                preferred_key: name.into(),
                                value,
                            },
                        );
                    }
                }
            }
        }
    }

    env
}

/// `CommandBuilder` is used to prepare a command to be spawned into a pty.
/// The interface is intentionally similar to that of `std::process::Command`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct CommandBuilder {
    args: Vec<OsString>,
    envs: BTreeMap<OsString, EnvEntry>,
    cwd: Option<OsString>,
    controlling_tty: bool,
}

impl CommandBuilder {
    /// Create a new builder instance with argv\[0\] set to the specified
    /// program.
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        Self {
            args: vec![program.as_ref().to_owned()],
            envs: get_base_env(),
            cwd: None,
            controlling_tty: true,
        }
    }

    /// Create a new builder instance from a pre-built argument vector
    pub fn from_argv(args: Vec<OsString>) -> Self {
        Self {
            args,
            envs: get_base_env(),
            cwd: None,
            controlling_tty: true,
        }
    }

    /// Set whether we should set the pty as the controlling terminal.
    /// The default is true, which is usually what you want.
    pub fn set_controlling_tty(&mut self, controlling_tty: bool) {
        self.controlling_tty = controlling_tty;
    }

    pub fn get_controlling_tty(&self) -> bool {
        self.controlling_tty
    }

    /// Create a new builder instance that will run some idea of a default
    /// program.  Such a builder will panic if `arg` is called on it.
    pub fn new_default_prog() -> Self {
        Self {
            args: vec![],
            envs: get_base_env(),
            cwd: None,
            controlling_tty: true,
        }
    }

    /// Returns true if this builder was created via `new_default_prog`
    pub fn is_default_prog(&self) -> bool {
        self.args.is_empty()
    }

    /// Append an argument to the current command line.
    /// Will panic if called on a builder created via `new_default_prog`.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) {
        if self.is_default_prog() {
            panic!("attempted to add args to a default_prog builder");
        }
        self.args.push(arg.as_ref().to_owned());
    }

    /// If a builder is_default_prog, then this function can be used to
    /// set the actual prog that should be used.
    /// This is intended to facilitate plumbing through the handling
    /// of the underlying default prog when merging together supplemental
    /// env and cwd information.
    /// You will not typically use this method in your own code.
    pub fn replace_default_prog(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) {
        if !self.is_default_prog() {
            panic!("attempted to replace_default_prog on a non-default_prog builder");
        }

        for arg in args {
            self.args.push(arg.as_ref().to_owned());
        }
    }

    /// Append a sequence of arguments to the current command line
    pub fn args<I, S>(&mut self, args: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
    }

    pub fn get_argv(&self) -> &Vec<OsString> {
        &self.args
    }

    pub fn get_argv_mut(&mut self) -> &mut Vec<OsString> {
        &mut self.args
    }

    /// Override the value of an environmental variable
    pub fn env<K, V>(&mut self, key: K, value: V)
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let key: OsString = key.as_ref().into();
        let value: OsString = value.as_ref().into();
        self.envs.insert(
            EnvEntry::map_key(key.clone()),
            EnvEntry {
                is_from_base_env: false,
                preferred_key: key,
                value: value,
            },
        );
    }

    pub fn env_remove<K>(&mut self, key: K)
    where
        K: AsRef<OsStr>,
    {
        let key = key.as_ref().into();
        self.envs.remove(&EnvEntry::map_key(key));
    }

    pub fn env_clear(&mut self) {
        self.envs.clear();
    }

    pub fn get_env<K>(&self, key: K) -> Option<&OsStr>
    where
        K: AsRef<OsStr>,
    {
        let key = key.as_ref().into();
        self.envs.get(&EnvEntry::map_key(key)).map(
            |EnvEntry {
                 is_from_base_env: _,
                 preferred_key: _,
                 value,
             }| value.as_os_str(),
        )
    }

    pub fn cwd<D>(&mut self, dir: D)
    where
        D: AsRef<OsStr>,
    {
        self.cwd = Some(dir.as_ref().to_owned());
    }

    pub fn clear_cwd(&mut self) {
        self.cwd.take();
    }

    pub fn get_cwd(&self) -> Option<&OsString> {
        self.cwd.as_ref()
    }

    /// Iterate over the configured environment. Only includes environment
    /// variables set by the caller via `env`, not variables set in the base
    /// environment.
    pub fn iter_extra_env_as_str(&self) -> impl Iterator<Item = (&str, &str)> {
        self.envs.values().filter_map(
            |EnvEntry {
                 is_from_base_env,
                 preferred_key,
                 value,
             }| {
                if *is_from_base_env {
                    None
                } else {
                    let key = preferred_key.to_str()?;
                    let value = value.to_str()?;
                    Some((key, value))
                }
            },
        )
    }

    pub fn iter_full_env_as_str(&self) -> impl Iterator<Item = (&str, &str)> {
        self.envs.values().filter_map(
            |EnvEntry {
                 preferred_key,
                 value,
                 ..
             }| {
                let key = preferred_key.to_str()?;
                let value = value.to_str()?;
                Some((key, value))
            },
        )
    }

    /// Return the configured command and arguments as a single string,
    /// quoted per the unix shell conventions.
    pub fn as_unix_command_line(&self) -> anyhow::Result<String> {
        let mut strs = vec![];
        for arg in &self.args {
            let s = arg
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("argument cannot be represented as utf8"))?;
            strs.push(s);
        }
        Ok(shell_words::join(strs))
    }
}

#[cfg(windows)]
impl CommandBuilder {
    fn search_path(&self, exe: &OsStr) -> OsString {
        if let Some(path) = self.get_env("PATH") {
            let extensions = self.get_env("PATHEXT").unwrap_or(OsStr::new(".EXE"));
            for path in std::env::split_paths(&path) {
                // Check for exactly the user's string in this path dir
                let candidate = path.join(&exe);
                if candidate.exists() {
                    return candidate.into_os_string();
                }

                // otherwise try tacking on some extensions.
                // Note that this really replaces the extension in the
                // user specified path, so this is potentially wrong.
                for ext in std::env::split_paths(&extensions) {
                    // PATHEXT includes the leading `.`, but `with_extension`
                    // doesn't want that
                    let ext = ext.to_str().expect("PATHEXT entries must be utf8");
                    let path = path.join(&exe).with_extension(&ext[1..]);
                    if path.exists() {
                        return path.into_os_string();
                    }
                }
            }
        }

        exe.to_owned()
    }

    pub(crate) fn current_directory(&self) -> Option<Vec<u16>> {
        let home: Option<&OsStr> = self
            .get_env("USERPROFILE")
            .filter(|path| Path::new(path).is_dir());
        let cwd: Option<&OsStr> = self.cwd.as_deref().filter(|path| Path::new(path).is_dir());
        let dir: Option<&OsStr> = cwd.or(home);

        dir.map(|dir| {
            let mut wide = vec![];

            if Path::new(dir).is_relative() {
                if let Ok(ccwd) = std::env::current_dir() {
                    wide.extend(ccwd.join(dir).as_os_str().encode_wide());
                } else {
                    wide.extend(dir.encode_wide());
                }
            } else {
                wide.extend(dir.encode_wide());
            }

            wide.push(0);
            wide
        })
    }

    /// Constructs an environment block for this spawn attempt.
    /// Uses the current process environment as the base and then
    /// adds/replaces the environment that was specified via the
    /// `env` methods.
    pub(crate) fn environment_block(&self) -> Vec<u16> {
        // encode the environment as wide characters
        let mut block = vec![];

        for EnvEntry {
            is_from_base_env: _,
            preferred_key,
            value,
        } in self.envs.values()
        {
            block.extend(preferred_key.encode_wide());
            block.push(b'=' as u16);
            block.extend(value.encode_wide());
            block.push(0);
        }
        // and a final terminator for CreateProcessW
        block.push(0);

        block
    }

    pub fn get_shell(&self) -> String {
        let exe: OsString = self
            .get_env("ComSpec")
            .unwrap_or(OsStr::new("cmd.exe"))
            .into();
        exe.into_string()
            .unwrap_or_else(|_| "%CompSpec%".to_string())
    }

    pub(crate) fn cmdline(&self) -> anyhow::Result<(Vec<u16>, Vec<u16>)> {
        let mut cmdline = Vec::<u16>::new();

        let exe: OsString = if self.is_default_prog() {
            self.get_env("ComSpec")
                .unwrap_or(OsStr::new("cmd.exe"))
                .into()
        } else {
            self.search_path(&self.args[0])
        };

        Self::append_quoted(&exe, &mut cmdline);

        // Ensure that we nul terminate the module name, otherwise we'll
        // ask CreateProcessW to start something random!
        let mut exe: Vec<u16> = exe.encode_wide().collect();
        exe.push(0);

        for arg in self.args.iter().skip(1) {
            cmdline.push(' ' as u16);
            anyhow::ensure!(
                !arg.encode_wide().any(|c| c == 0),
                "invalid encoding for command line argument {:?}",
                arg
            );
            Self::append_quoted(arg, &mut cmdline);
        }
        // Ensure that the command line is nul terminated too!
        cmdline.push(0);
        Ok((exe, cmdline))
    }

    // Borrowed from https://github.com/hniksic/rust-subprocess/blob/873dfed165173e52907beb87118b2c0c05d8b8a1/src/popen.rs#L1117
    // which in turn was translated from ArgvQuote at http://tinyurl.com/zmgtnls
    fn append_quoted(arg: &OsStr, cmdline: &mut Vec<u16>) {
        if !arg.is_empty()
            && !arg.encode_wide().any(|c| {
                c == ' ' as u16
                    || c == '\t' as u16
                    || c == '\n' as u16
                    || c == '\x0b' as u16
                    || c == '\"' as u16
            })
        {
            cmdline.extend(arg.encode_wide());
            return;
        }
        cmdline.push('"' as u16);

        let arg: Vec<_> = arg.encode_wide().collect();
        let mut i = 0;
        while i < arg.len() {
            let mut num_backslashes = 0;
            while i < arg.len() && arg[i] == '\\' as u16 {
                i += 1;
                num_backslashes += 1;
            }

            if i == arg.len() {
                for _ in 0..num_backslashes * 2 {
                    cmdline.push('\\' as u16);
                }
                break;
            } else if arg[i] == b'"' as u16 {
                for _ in 0..num_backslashes * 2 + 1 {
                    cmdline.push('\\' as u16);
                }
                cmdline.push(arg[i]);
            } else {
                for _ in 0..num_backslashes {
                    cmdline.push('\\' as u16);
                }
                cmdline.push(arg[i]);
            }
            i += 1;
        }
        cmdline.push('"' as u16);
    }
}

#[cfg(not(windows))]
impl CommandBuilder {
    pub fn get_shell(&self) -> String {
        "unsupported".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env() {
        let mut cmd = CommandBuilder::new("dummy");
        let package_authors = cmd.get_env("CARGO_PKG_AUTHORS");
        println!("package_authors: {:?}", package_authors);
        assert!(package_authors == Some(OsStr::new("Wez Furlong")));

        cmd.env("foo key", "foo value");
        cmd.env("bar key", "bar value");

        let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
        println!("iterated_envs: {:?}", iterated_envs);
        assert!(iterated_envs == vec![("bar key", "bar value"), ("foo key", "foo value")]);

        {
            let mut cmd = cmd.clone();
            cmd.env_remove("foo key");

            let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
            println!("iterated_envs: {:?}", iterated_envs);
            assert!(iterated_envs == vec![("bar key", "bar value")]);
        }

        {
            let mut cmd = cmd.clone();
            cmd.env_remove("bar key");

            let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
            println!("iterated_envs: {:?}", iterated_envs);
            assert!(iterated_envs == vec![("foo key", "foo value")]);
        }

        {
            let mut cmd = cmd.clone();
            cmd.env_clear();

            let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
            println!("iterated_envs: {:?}", iterated_envs);
            assert!(iterated_envs.is_empty());
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_env_case_insensitive_override() {
        let mut cmd = CommandBuilder::new("dummy");
        cmd.env("Cargo_Pkg_Authors", "Not Wez");
        assert!(cmd.get_env("cargo_pkg_authors") == Some(OsStr::new("Not Wez")));

        cmd.env_remove("cARGO_pKG_aUTHORS");
        assert!(cmd.get_env("CARGO_PKG_AUTHORS").is_none());
    }
}
