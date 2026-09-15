fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target = std::env::var("TARGET").expect("TARGET is set by Cargo");
    if !target.contains("windows") {
        return;
    }

    use anyhow::Context as _;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    let profile = std::env::var("PROFILE").unwrap();
    let repo_dir = std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.parent().map(|p| p.to_path_buf()))
        .unwrap();
    let mut target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_dir.join("target"));
    if target_dir.is_relative() {
        target_dir = repo_dir.join(target_dir);
    }
    let host = std::env::var("HOST").unwrap_or_default();
    let exe_output_dir = if target == host {
        target_dir.join(&profile)
    } else {
        target_dir.join(&target).join(&profile)
    };
    std::fs::create_dir_all(&exe_output_dir).unwrap();
    let windows_dir = repo_dir.join("assets").join("windows");

    let conhost_dir = windows_dir.join("conhost");
    for name in &["conpty.dll", "OpenConsole.exe"] {
        let dest_name = exe_output_dir.join(name);
        let src_name = conhost_dir.join(name);

        if !dest_name.exists() {
            std::fs::copy(&src_name, &dest_name)
                .context(format!(
                    "copy {} -> {}",
                    src_name.display(),
                    dest_name.display()
                ))
                .unwrap();
        }
    }

    let angle_dir = windows_dir.join("angle");
    for name in &["libEGL.dll", "libGLESv2.dll"] {
        let dest_name = exe_output_dir.join(name);
        let src_name = angle_dir.join(name);

        if !dest_name.exists() {
            std::fs::copy(&src_name, &dest_name)
                .context(format!(
                    "copy {} -> {}",
                    src_name.display(),
                    dest_name.display()
                ))
                .unwrap();
        }
    }

    let dest_mesa = exe_output_dir.join("mesa");
    std::fs::create_dir_all(&dest_mesa).unwrap();
    let dest_name = dest_mesa.join("opengl32.dll");
    let src_name = windows_dir.join("mesa").join("opengl32.dll");
    if !dest_name.exists() {
        std::fs::copy(&src_name, &dest_name)
            .context(format!(
                "copy {} -> {}",
                src_name.display(),
                dest_name.display()
            ))
            .unwrap();
    }

    // If a file named `.tag` is present, use its contents for the file version.
    let mut ci_tag = String::new();
    if let Ok(tag) = std::fs::read("../.tag") {
        if let Ok(s) = String::from_utf8(tag) {
            ci_tag = s.trim().to_string();
            println!("cargo:rerun-if-changed=../.tag");
        }
    }
    let version = if ci_tag.is_empty() {
        let mut cmd = std::process::Command::new("git");
        cmd.args([
            "-c",
            "core.abbrev=8",
            "show",
            "-s",
            "--format=%cd-%h",
            "--date=format:%Y%m%d-%H%M%S",
        ]);
        if let Ok(output) = cmd.output() {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).trim().to_owned()
            } else {
                "UNKNOWN".to_owned()
            }
        } else {
            "UNKNOWN".to_owned()
        }
    } else {
        ci_tag
    };

    let out_dir = Path::new(&std::env::var_os("OUT_DIR").unwrap()).to_path_buf();
    let rcfile_name = out_dir.join("resource.rc");
    let mut rcfile = std::fs::File::create(&rcfile_name).unwrap();
    println!("cargo:rerun-if-changed=../assets/windows/manifest.manifest");
    println!("cargo:rerun-if-changed=../assets/windows/terminal.ico");
    let resource_root = windows_dir.display().to_string().replace('\\', "\\\\");
    write!(
        rcfile,
        r#"
#include <winres.h>
// This ID is coupled with code in window/src/os/windows/window.rs
#define IDI_ICON 0x101
1 RT_MANIFEST "{root}/manifest.manifest"
IDI_ICON ICON "{root}/terminal.ico"
VS_VERSION_INFO VERSIONINFO
FILEVERSION     1,0,0,0
PRODUCTVERSION  1,0,0,0
FILEFLAGSMASK   VS_FFI_FILEFLAGSMASK
FILEFLAGS       0
FILEOS          VOS__WINDOWS32
FILETYPE        VFT_APP
FILESUBTYPE     VFT2_UNKNOWN
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904E4"
        BEGIN
            VALUE "CompanyName",      "Wez Furlong\0"
            VALUE "FileDescription",  "WezTerm - Wez's Terminal Emulator\0"
            VALUE "FileVersion",      "{version}\0"
            VALUE "LegalCopyright",   "Wez Furlong, MIT licensed\0"
            VALUE "InternalName",     "\0"
            VALUE "OriginalFilename", "\0"
            VALUE "ProductName",      "WezTerm\0"
            VALUE "ProductVersion",   "{version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1252
    END
END
"#,
        root = resource_root,
        version = version,
    )
    .unwrap();
    drop(rcfile);

    // Native MSVC builds may need the Windows SDK environment. GNU cross
    // builds use embed-resource's target-aware windres discovery instead.
    if target.ends_with("-msvc") && cfg!(target_os = "windows") {
        if let Some(tool) = cc::windows_registry::find_tool(target.as_str(), "cl.exe") {
            for (key, value) in tool.env() {
                std::env::set_var(key, value);
            }
        }
    }
    embed_resource::compile(rcfile_name);
}
