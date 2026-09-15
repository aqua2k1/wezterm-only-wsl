use config::keyassignment::*;
use config::{ConfigHandle, DeferredKeyCode};
use ordered_float::NotNan;
use std::convert::TryFrom;
use window::{KeyCode, Modifiers};
use KeyAssignment::*;

/// The small part of the old command registry that is still needed by the
/// single-session GUI: installing built-in key bindings.  Menus, launchers,
/// command metadata and dynamic session commands were removed.
pub struct CommandDef;

impl CommandDef {
    pub fn default_key_assignments(
        config: &ConfigHandle,
    ) -> Vec<(Modifiers, KeyCode, KeyAssignment)> {
        let mut result = vec![];
        for action in compute_default_actions() {
            for (mods, label) in default_key_specs(&action) {
                let code = DeferredKeyCode::try_from(label.as_str())
                    .unwrap()
                    .resolve(config.key_map_preference)
                    .clone();
                let shifted = DeferredKeyCode::try_from(us_layout_shift(&label))
                    .unwrap()
                    .resolve(config.key_map_preference)
                    .clone();

                result.push((mods, code.clone(), action.clone()));
                if mods == Modifiers::SUPER {
                    result.push((Modifiers::CTRL | Modifiers::SHIFT, code, action.clone()));
                    if shifted != result.last().unwrap().1 {
                        result.push((
                            Modifiers::CTRL | Modifiers::SHIFT,
                            shifted.clone(),
                            action.clone(),
                        ));
                        result.push((Modifiers::CTRL, shifted, action.clone()));
                    }
                } else if mods.contains(Modifiers::SHIFT) && shifted != code {
                    result.push((mods, shifted.clone(), action.clone()));
                    result.push((mods - Modifiers::SHIFT, shifted, action.clone()));
                }
            }
        }
        result
    }
}

/// Synthesize the shifted forms needed by keyboard layouts and X11/Windows
/// key normalization.  The built-in labels intentionally use US-layout names.
fn us_layout_shift(s: &str) -> String {
    match s {
        "1" => "!".to_string(),
        "2" => "@".to_string(),
        "3" => "#".to_string(),
        "4" => "$".to_string(),
        "5" => "%".to_string(),
        "6" => "^".to_string(),
        "7" => "&".to_string(),
        "8" => "*".to_string(),
        "9" => "(".to_string(),
        "0" => ")".to_string(),
        "[" => "{".to_string(),
        "]" => "}".to_string(),
        "=" => "+".to_string(),
        "-" => "_".to_string(),
        "'" => "\"".to_string(),
        s if s.len() == 1 => s.to_ascii_uppercase(),
        s => s.to_string(),
    }
}

fn default_key_specs(action: &KeyAssignment) -> Vec<(Modifiers, String)> {
    match action {
        PasteFrom(ClipboardPasteSource::PrimarySelection) => {
            vec![(Modifiers::SHIFT, "Insert".into())]
        }
        CopyTo(ClipboardCopyDestination::PrimarySelection) => {
            vec![(Modifiers::CTRL, "Insert".into())]
        }
        CopyTo(ClipboardCopyDestination::Clipboard) => vec![
            (Modifiers::SUPER, "c".into()),
            (Modifiers::NONE, "Copy".into()),
        ],
        PasteFrom(ClipboardPasteSource::Clipboard) => vec![
            (Modifiers::SUPER, "v".into()),
            (Modifiers::NONE, "Paste".into()),
        ],
        ToggleFullScreen => vec![(Modifiers::ALT, "Return".into())],
        ClearScrollback(ScrollbackEraseMode::ScrollbackOnly) => {
            vec![(Modifiers::SUPER, "k".into())]
        }
        Search(Pattern::CurrentSelectionOrEmptyString) => {
            vec![(Modifiers::SUPER, "f".into())]
        }
        ShowDebugOverlay => vec![(Modifiers::CTRL | Modifiers::SHIFT, "l".into())],
        QuickSelect => vec![(Modifiers::CTRL | Modifiers::SHIFT, "Space".into())],
        CharSelect(_) => vec![(Modifiers::CTRL | Modifiers::SHIFT, "u".into())],
        DecreaseFontSize => vec![
            (Modifiers::SUPER, "-".into()),
            (Modifiers::CTRL, "-".into()),
        ],
        IncreaseFontSize => vec![
            (Modifiers::SUPER, "=".into()),
            (Modifiers::CTRL, "=".into()),
        ],
        ResetFontSize => vec![
            (Modifiers::SUPER, "0".into()),
            (Modifiers::CTRL, "0".into()),
        ],
        CloseCurrentTab { confirm: true } => vec![(Modifiers::SUPER, "w".into())],
        ReloadConfiguration => vec![(Modifiers::SUPER, "r".into())],
        ScrollByPage(amount) if amount.into_inner() == -1.0 => {
            vec![(Modifiers::SHIFT, "PageUp".into())]
        }
        ScrollByPage(amount) if amount.into_inner() == 1.0 => {
            vec![(Modifiers::SHIFT, "PageDown".into())]
        }
        ActivateCopyMode => vec![(Modifiers::CTRL | Modifiers::SHIFT, "x".into())],
        #[cfg(target_os = "macos")]
        QuitApplication => vec![(Modifiers::SUPER, "q".into())],
        _ => vec![],
    }
}

fn compute_default_actions() -> Vec<KeyAssignment> {
    vec![
        ReloadConfiguration,
        #[cfg(target_os = "macos")]
        QuitApplication,
        CloseCurrentTab { confirm: true },
        CloseCurrentPane { confirm: true },
        ResetTerminal,
        #[cfg(not(target_os = "macos"))]
        PasteFrom(ClipboardPasteSource::PrimarySelection),
        #[cfg(not(target_os = "macos"))]
        CopyTo(ClipboardCopyDestination::PrimarySelection),
        CopyTo(ClipboardCopyDestination::Clipboard),
        PasteFrom(ClipboardPasteSource::Clipboard),
        ClearScrollback(ScrollbackEraseMode::ScrollbackOnly),
        ClearScrollback(ScrollbackEraseMode::ScrollbackAndViewport),
        QuickSelect,
        CharSelect(CharSelectArguments::default()),
        ActivateCopyMode,
        DecreaseFontSize,
        IncreaseFontSize,
        ResetFontSize,
        ResetFontAndWindowSize,
        ScrollByPage(NotNan::new(-1.0).unwrap()),
        ScrollByPage(NotNan::new(1.0).unwrap()),
        ScrollToTop,
        ScrollToBottom,
        ToggleFullScreen,
        Search(Pattern::CurrentSelectionOrEmptyString),
        ShowDebugOverlay,
    ]
}
