use clap::{Parser, ValueHint};
use config::GuiPosition;
use std::ffi::OsString;
use std::path::PathBuf;

pub const DEFAULT_WINDOW_CLASS: &str = "org.wezfurlong.wezterm";

/// Helper for parsing config overrides.
pub fn name_equals_value(arg: &str) -> Result<(String, String), String> {
    if let Some(eq) = arg.find('=') {
        let (left, right) = arg.split_at(eq);
        let left = left.trim();
        let right = right[1..].trim();
        if left.is_empty() || right.is_empty() {
            return Err(format!(
                "Got empty name/value `{}`; expected name=value",
                arg
            ));
        }
        Ok((left.to_string(), right.to_string()))
    } else {
        Err(format!("Expected name=value, but got {}", arg))
    }
}

/// Start one independent local WSL session.
#[derive(Debug, Parser, Default, Clone)]
#[command(trailing_var_arg = true)]
pub struct StartCommand {
    /// Working directory passed to the WSL session.
    #[arg(long = "cwd", value_parser, value_hint=ValueHint::DirPath)]
    pub cwd: Option<PathBuf>,

    /// Compatibility alias for a trailing program invocation.
    #[arg(short = 'e', hide = true)]
    pub _cmd: bool,

    /// Override the Windows window class.
    #[arg(long = "class")]
    pub class: Option<String>,

    /// Initial window position, for example --position 10,20.
    #[arg(long)]
    pub position: Option<GuiPosition>,

    /// Name of a configured WSL domain; defaults to the selected WSL domain.
    #[arg(long)]
    pub domain: Option<String>,

    /// Run PROG inside WSL instead of its default shell.
    #[arg(value_parser, value_hint=ValueHint::CommandWithArguments, num_args=1..)]
    pub prog: Vec<OsString>,
}

#[derive(Debug, Parser, Clone)]
pub struct LsFontsCommand {
    /// List all fonts available to the system.
    #[arg(long)]
    pub list_system: bool,

    /// Explain which fonts render this text.
    #[arg(long = "text", conflicts_with_all = &["list_system", "codepoints"])]
    pub text: Option<String>,

    /// Comma-separated hexadecimal Unicode code points to inspect.
    #[arg(long, conflicts_with = "list_system")]
    pub codepoints: Option<String>,

    /// Show rasterized glyphs using ASCII blocks.
    #[arg(long, requires = "text")]
    pub rasterize_ascii: bool,
}

#[derive(Debug, Parser, Clone)]
pub struct ShowKeysCommand {
    /// Show keys as Lua configuration statements.
    #[arg(long)]
    pub lua: bool,
    /// In Lua mode, show only the named key table.
    #[arg(long)]
    pub key_table: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_wsl_command() {
        let opts =
            StartCommand::try_parse_from(["start", "--domain", "WSL:Ubuntu", "--", "bash", "-l"])
                .unwrap();
        assert_eq!(opts.domain.as_deref(), Some("WSL:Ubuntu"));
        assert_eq!(
            opts.prog,
            vec![OsString::from("bash"), OsString::from("-l")]
        );
    }

    #[test]
    fn rejects_removed_session_options() {
        for flag in ["--new-tab", "--attach", "--workspace", "--no-auto-connect"] {
            assert!(StartCommand::try_parse_from(["start", flag]).is_err());
        }
    }
}
