#![cfg(windows)]

use crate::{ToastNotification as TN, WINDOWS_APP_USER_MODEL_ID};
use std::ffi::OsStr;
use std::fs;
use std::mem::{self, ManuallyDrop};
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;
use std::sync::OnceLock;
use xml::escape::escape_str_pcdata;

use windows::core::{Error as WinError, IInspectable, Interface, GUID, HSTRING, PCWSTR, PWSTR};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::Win32::Foundation::{BOOL, E_POINTER};
use windows::Win32::System::Com::StructuredStorage::{
    PropVariantClear, PROPVARIANT, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoUninitialize, IPersistFile,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};
use windows::Win32::UI::Shell::{IShellLinkW, SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastFailedEventArgs, ToastNotification, ToastNotificationManager,
};

const CLSID_SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);
const PKEY_APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};
const VT_LPWSTR: u16 = 31;

static SHORTCUT_REGISTRATION: OnceLock<Result<(), String>> = OnceLock::new();

fn unwrap_arg<T>(a: &Option<T>) -> Result<&T, WinError> {
    match a {
        Some(t) => Ok(t),
        None => Err(WinError::new(E_POINTER, HSTRING::from("option is none"))),
    }
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn app_user_model_id_prop_variant() -> Result<PROPVARIANT, String> {
    let value = wide(OsStr::new(WINDOWS_APP_USER_MODEL_ID));
    let bytes = value
        .len()
        .checked_mul(mem::size_of::<u16>())
        .ok_or_else(|| "AppUserModelID is too long".to_string())?;
    let ptr = unsafe { CoTaskMemAlloc(bytes) as *mut u16 };
    if ptr.is_null() {
        return Err("CoTaskMemAlloc failed for AppUserModelID".to_string());
    }
    unsafe {
        ptr.copy_from_nonoverlapping(value.as_ptr(), value.len());
    }

    let mut variant = PROPVARIANT::default();
    unsafe {
        let mut data: PROPVARIANT_0_0_0 = mem::zeroed();
        data.pwszVal = PWSTR(ptr);
        variant.Anonymous.Anonymous = ManuallyDrop::new(PROPVARIANT_0_0 {
            vt: VT_LPWSTR,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            Anonymous: data,
        });
    }
    Ok(variant)
}

/// Register the current executable as a desktop-toast application.
///
/// Windows requires an unpackaged Win32 application to have a Start-menu
/// shortcut carrying `System.AppUserModel.ID`. The Setup installer creates
/// one, but portable builds and stale/manual installations do not. Repairing
/// a per-user shortcut here makes both deployment modes use the same toast
/// identity without requiring elevation.
fn register_app_user_model_shortcut() -> Result<(), String> {
    let app_data = std::env::var_os("APPDATA")
        .ok_or_else(|| "APPDATA is unavailable; cannot register toast shortcut".to_string())?;
    let target = std::env::current_exe().map_err(|err| err.to_string())?;
    let shortcut = PathBuf::from(app_data)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("WezTerm.lnk");
    if let Some(parent) = shortcut.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED) }
        .map_err(|err| err.to_string())?;

    let result = (|| {
        let link: IShellLinkW =
            unsafe { CoCreateInstance(&CLSID_SHELL_LINK, None, CLSCTX_INPROC_SERVER) }
                .map_err(|err| err.to_string())?;
        let target_wide = wide(target.as_os_str());
        let working_directory = target
            .parent()
            .ok_or_else(|| "executable has no parent directory".to_string())?;
        let working_directory_wide = wide(working_directory.as_os_str());
        let shortcut_wide = wide(shortcut.as_os_str());

        if shortcut.exists() {
            let persist: IPersistFile = link.cast().map_err(|err| err.to_string())?;
            unsafe { persist.Load(PCWSTR(shortcut_wide.as_ptr()), 0) }
                .map_err(|err| err.to_string())?;
        }

        unsafe {
            link.SetPath(PCWSTR(target_wide.as_ptr()))
                .map_err(|err| err.to_string())?;
            link.SetWorkingDirectory(PCWSTR(working_directory_wide.as_ptr()))
                .map_err(|err| err.to_string())?;
            link.SetIconLocation(PCWSTR(target_wide.as_ptr()), 0)
                .map_err(|err| err.to_string())?;
        }

        let store: IPropertyStore = link.cast().map_err(|err| err.to_string())?;
        let mut variant = app_user_model_id_prop_variant()?;
        let property_result = (|| {
            unsafe { store.SetValue(&PKEY_APP_USER_MODEL_ID, &variant) }
                .map_err(|err| err.to_string())?;
            unsafe { store.Commit() }.map_err(|err| err.to_string())
        })();
        unsafe {
            let _ = PropVariantClear(&mut variant);
        }
        property_result?;

        let persist: IPersistFile = link.cast().map_err(|err| err.to_string())?;
        unsafe { persist.Save(PCWSTR(shortcut_wide.as_ptr()), BOOL(1)) }
            .map_err(|err| err.to_string())?;
        unsafe {
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, ptr::null(), ptr::null());
        }
        Ok(())
    })();

    unsafe { CoUninitialize() };
    result
}

fn ensure_app_user_model_shortcut() {
    let result = SHORTCUT_REGISTRATION.get_or_init(register_app_user_model_shortcut);
    if let Err(err) = result {
        log::warn!("Unable to register Windows toast shortcut: {err}");
    }
}

fn show_notif_impl(toast: TN) -> Result<(), Box<dyn std::error::Error>> {
    ensure_app_user_model_shortcut();

    let xml = XmlDocument::new()?;

    let url_actions = if toast.url.is_some() {
        r#"
        <actions>
           <action content="Show" arguments="show" />
        </actions>
        "#
    } else {
        ""
    };

    xml.LoadXml(HSTRING::from(format!(
        r#"<toast duration="long">
        <visual>
            <binding template="ToastGeneric">
                <text>{}</text>
                <text>{}</text>
            </binding>
        </visual>
        {}
    </toast>"#,
        escape_str_pcdata(&toast.title),
        escape_str_pcdata(&toast.message),
        url_actions
    )))?;

    let notif = ToastNotification::CreateToastNotification(xml)?;

    notif.Activated(TypedEventHandler::new(
        move |_: &Option<ToastNotification>, result: &Option<IInspectable>| {
            let result = unwrap_arg(result)?.cast::<ToastActivatedEventArgs>()?;
            let args = result.Arguments()?;

            if args == "show" {
                if let Some(url) = toast.url.as_ref() {
                    wezterm_open_url::open_url(url);
                }
            }

            Ok(())
        },
    ))?;

    notif.Failed(TypedEventHandler::new(
        |_sender: &Option<ToastNotification>, result: &Option<ToastFailedEventArgs>| {
            if let Some(result) = result {
                log::error!("Windows toast failed: {:?}", result.ErrorCode());
            } else {
                log::error!("Windows toast failed without error details");
            }
            Ok(())
        },
    ))?;

    let notifier = ToastNotificationManager::CreateToastNotifierWithId(HSTRING::from(
        WINDOWS_APP_USER_MODEL_ID,
    ))?;
    if let Ok(setting) = notifier.Setting() {
        log::debug!("Windows toast setting for {WINDOWS_APP_USER_MODEL_ID}: {setting:?}");
    }
    notifier.Show(&notif)?;

    Ok(())
}

pub fn show_notif(notif: TN) -> Result<(), Box<dyn std::error::Error>> {
    // We need to be in a different thread from the caller
    // in case we get called in the guts of a windows message
    // loop dispatch and are unable to pump messages
    std::thread::spawn(move || {
        if let Err(err) = show_notif_impl(notif) {
            log::error!("Failed to show toast notification: {:#}", err);
        }
    });

    Ok(())
}
