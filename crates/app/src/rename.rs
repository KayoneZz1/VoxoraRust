#![allow(unsafe_op_in_unsafe_fn)]

//! Windows ses uç noktası adlarını değiştirme (C# AudioEndpointNamingService portu).
//!
//! Yöntem: Windows Ses ayarları ekranının kullandığı belgelenmemiş
//! `IPolicyConfig::SetPropertyValue` (Windows 7+ vtable, `fxStore` argümanlı).
//! Uç nokta özellik deposu yönetici izni istediği için normal deneme başarısız
//! olursa uygulama kendini `--rename-audio-endpoints` ile yükseltilmiş çalıştırır.

use std::ffi::c_void;
use std::path::PathBuf;
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
use windows_sys::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemAlloc, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

pub const RENAME_SWITCH: &str = "--rename-audio-endpoints";
pub const OUT_NAME: &str = "Voxora Out Audio";
pub const IN_NAME: &str = "Voxora In Audio";

const E_ACCESSDENIED: i32 = 0x8007_0005u32 as i32;
const INFINITE: u32 = 0xFFFF_FFFF;
const VT_LPWSTR: u16 = 31;

const FRIENDLY_NAME_FMT: GUID = GUID {
    data1: 0xa45c_254e,
    data2: 0xdf1c,
    data3: 0x4efd,
    data4: [0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0],
};

const CLSID_POLICY_CONFIG: GUID = GUID {
    data1: 0x870a_f99c,
    data2: 0x171d,
    data3: 0x4f9e,
    data4: [0xaf, 0x0d, 0xe6, 0x3d, 0xf4, 0x0c, 0x2b, 0xc9],
};

const IID_IPOLICY_CONFIG: GUID = GUID {
    data1: 0xf867_9f50,
    data2: 0x850a,
    data3: 0x41cf,
    data4: [0x9c, 0x72, 0x43, 0x0f, 0x29, 0x02, 0x90, 0xc8],
};

#[repr(C)]
#[derive(Clone, Copy)]
struct PropertyKey {
    format_id: GUID,
    property_id: u32,
}

#[repr(C)]
struct PropVariant {
    variant_type: u16,
    r1: u16,
    r2: u16,
    r3: u16,
    pointer: *mut c_void,
    pointer2: *mut c_void,
    pointer3: *mut c_void,
}

#[repr(C)]
struct IPolicyConfigVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_mix_format: unsafe extern "system" fn(*mut c_void, *const u16, *mut *mut c_void) -> i32,
    get_device_format:
        unsafe extern "system" fn(*mut c_void, *const u16, i32, *mut *mut c_void) -> i32,
    reset_device_format: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
    set_device_format:
        unsafe extern "system" fn(*mut c_void, *const u16, *mut c_void, *mut c_void) -> i32,
    get_processing_period:
        unsafe extern "system" fn(*mut c_void, *const u16, i32, *mut i64, *mut i64) -> i32,
    set_processing_period: unsafe extern "system" fn(*mut c_void, *const u16, *mut i64) -> i32,
    get_share_mode: unsafe extern "system" fn(*mut c_void, *const u16, *mut *mut c_void) -> i32,
    set_share_mode: unsafe extern "system" fn(*mut c_void, *const u16, *mut c_void) -> i32,
    get_property_value:
        unsafe extern "system" fn(*mut c_void, *const u16, i32, *const PropertyKey, *mut PropVariant)
            -> i32,
    set_property_value:
        unsafe extern "system" fn(*mut c_void, *const u16, i32, *const PropertyKey, *const PropVariant)
            -> i32,
    set_default_endpoint: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> i32,
    set_endpoint_visibility: unsafe extern "system" fn(*mut c_void, *const u16, i32) -> i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flow {
    Render,
    Capture,
}

impl Flow {
    fn registry_root(self) -> &'static str {
        match self {
            Flow::Render => "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\MMDevices\\Audio\\Render",
            Flow::Capture => {
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\MMDevices\\Audio\\Capture"
            }
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Flow::Render => "0.0.0.00000000",
            Flow::Capture => "0.0.1.00000000",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub guid: String,
    pub id: String,
    pub name: String,
    pub flow: Flow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameOutcome {
    Renamed,
    AlreadyNamed,
    NotFound,
    AccessDenied,
    UacCancelled,
    Failed(String),
}

/// Bir uç noktanın IMMDevice kimliğini üretir.
pub fn endpoint_id(flow: Flow, guid: &str) -> String {
    let guid = guid.trim();
    if guid.starts_with("{0.0.0.") || guid.starts_with("{0.0.1.") {
        return guid.to_string();
    }
    let g = guid.trim_start_matches('{').trim_end_matches('}');
    format!("{{{}}}.{{{}}}", flow.prefix(), g)
}

fn readable(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("cable") || lower.contains("voxora")
}

fn friendly_of(props: &RegKey) -> Option<String> {
    let candidates = [
        "{a45c254e-df1c-4efd-8020-67d146a850e0},14",
        "{a45c254e-df1c-4efd-8020-67d146a850e0},2",
        "{b3f8fa53-0004-438e-9003-51a46e139bfc},6",
    ];
    for key in candidates {
        if let Ok(value) = props.get_value::<String, _>(key) {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    None
}

/// Kayıt defterinden CABLE uç noktalarını bulur.
pub fn find_cable() -> Option<(Endpoint, Endpoint)> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut render: Option<Endpoint> = None;
    let mut capture: Option<Endpoint> = None;
    for flow in [Flow::Render, Flow::Capture] {
        let Ok(root) = hklm.open_subkey(flow.registry_root()) else {
            continue;
        };
        for guid in root.enum_keys().flatten() {
            let Ok(props) = root.open_subkey(format!("{guid}\\Properties")) else {
                continue;
            };
            let Some(name) = friendly_of(&props) else {
                continue;
            };
            if !readable(&name) {
                continue;
            }
            let lower = name.to_lowercase();
            let endpoint = Endpoint {
                guid: guid.clone(),
                id: endpoint_id(flow, &guid),
                name: name.clone(),
                flow,
            };
            match flow {
                Flow::Render if lower.contains("cable input") && render.is_none() => {
                    render = Some(endpoint)
                }
                Flow::Capture if lower.contains("cable output") && capture.is_none() => {
                    capture = Some(endpoint)
                }
                _ => {}
            }
        }
    }
    match (render, capture) {
        (Some(r), Some(c)) => Some((r, c)),
        _ => None,
    }
}

pub fn is_voxora_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("voxora in audio") || lower.contains("voxora out audio")
}

pub fn names_applied() -> bool {
    match find_cable() {
        Some((render, capture)) => is_voxora_name(&render.name) && is_voxora_name(&capture.name),
        None => false,
    }
}

// ------------------------------------------------------------------ COM

struct ComGuard {
    uninit: bool,
}

impl ComGuard {
    fn new() -> Self {
        let hr = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        Self { uninit: hr >= 0 }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninit {
            unsafe { CoUninitialize() };
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn set_friendly_name(device_id: &str, name: &str) -> i32 {
    let _com = ComGuard::new();
    unsafe {
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let hr = CoCreateInstance(
            &CLSID_POLICY_CONFIG,
            std::ptr::null_mut(),
            CLSCTX_ALL,
            &IID_IPOLICY_CONFIG,
            &mut ptr,
        );
        if hr < 0 || ptr.is_null() {
            log(&format!("CoCreateInstance hr=0x{hr:08X}"));
            return hr;
        }
        let id = wide(device_id);
        let text = wide(name);
        let bytes = text.len() * std::mem::size_of::<u16>();
        let mem = CoTaskMemAlloc(bytes) as *mut u16;
        if mem.is_null() {
            let vtbl = *(ptr as *mut *const IPolicyConfigVtbl);
            ((*vtbl).release)(ptr);
            return -1;
        }
        std::ptr::copy_nonoverlapping(text.as_ptr(), mem, text.len());

        let mut pv: PropVariant = std::mem::zeroed();
        pv.variant_type = VT_LPWSTR;
        pv.pointer = mem as *mut c_void;
        let key = PropertyKey {
            format_id: FRIENDLY_NAME_FMT,
            property_id: 14,
        };
        let vtbl = *(ptr as *mut *const IPolicyConfigVtbl);
        let hr = ((*vtbl).set_property_value)(ptr, id.as_ptr(), 1, &key, &pv);
        CoTaskMemFree(mem as *const c_void);
        ((*vtbl).release)(ptr);
        log(&format!(
            "SetPropertyValue({}, '{}') hr=0x{hr:08X}",
            short(device_id),
            name
        ));
        hr
    }
}

fn try_rename_both(render: &Endpoint, capture: &Endpoint) -> (i32, i32) {
    let hr_out = set_friendly_name(&render.id, IN_NAME);
    let hr_in = set_friendly_name(&capture.id, OUT_NAME);
    (hr_out, hr_in)
}

fn try_registry(endpoint: &Endpoint, name: &str) -> bool {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let guid = endpoint
        .guid
        .trim_start_matches('{')
        .trim_end_matches('}')
        .to_string();
    let path = format!(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\MMDevices\\Audio\\{}\\{{{}}}\\Properties",
        match endpoint.flow {
            Flow::Render => "Render",
            Flow::Capture => "Capture",
        },
        guid
    );
    match hklm.open_subkey_with_flags(path, winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ)
    {
        Ok(key) => key
            .set_value("{a45c254e-df1c-4efd-8020-67d146a850e0},14", &name.to_string())
            .is_ok(),
        Err(e) => {
            log(&format!("registry open failed: {e}"));
            false
        }
    }
}

fn restart_audio_services() {
    for args in [
        ["stop", "Audiosrv"],
        ["stop", "AudioEndpointBuilder"],
    ] {
        let _ = std::process::Command::new("sc.exe")
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    std::thread::sleep(std::time::Duration::from_millis(600));
    for args in [
        ["start", "AudioEndpointBuilder"],
        ["start", "Audiosrv"],
    ] {
        let _ = std::process::Command::new("sc.exe")
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        std::thread::sleep(std::time::Duration::from_millis(800));
    }
}

fn wait_for_names(timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    loop {
        if names_applied() {
            return true;
        }
        if start.elapsed().as_millis() as u64 >= timeout_ms {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// Yükseltilmiş süreçte çalışır: 0 değişti, 2 kablo yok, 3 izin yok, 4 diğer.
pub fn rename_elevated() -> i32 {
    log("=== elevated start ===");
    let Some((render, capture)) = find_cable() else {
        log("cable endpoints not found");
        return 2;
    };
    let (hr_out, hr_in) = try_rename_both(&render, &capture);
    if hr_out == 0 && hr_in == 0 && wait_for_names(2000) {
        log("policy rename verified");
        return 0;
    }
    log("policy failed, trying registry fallback");
    let reg_out = try_registry(&render, IN_NAME);
    let reg_in = try_registry(&capture, OUT_NAME);
    if reg_out && reg_in {
        restart_audio_services();
        if wait_for_names(6000) {
            log("registry rename verified");
            return 0;
        }
    }
    if names_applied() {
        return 0;
    }
    let denied = hr_out == E_ACCESSDENIED || hr_in == E_ACCESSDENIED;
    log(&format!(
        "rename failed hr_out=0x{hr_out:08X} hr_in=0x{hr_in:08X} denied={denied}"
    ));
    if denied {
        3
    } else {
        4
    }
}

enum HelperError {
    UacCancelled,
    Failed(String),
}

fn run_elevated_helper() -> Result<i32, HelperError> {
    let exe = std::env::current_exe()
        .map_err(|e| HelperError::Failed(e.to_string()))?;
    let exe_w = wide(&exe.to_string_lossy());
    let verb = wide("runas");
    let params = wide(RENAME_SWITCH);
    let dir = exe
        .parent()
        .map(|p| wide(&p.to_string_lossy()))
        .unwrap_or_else(|| wide(""));
    unsafe {
        let mut sei: SHELLEXECUTEINFOW = std::mem::zeroed();
        sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        sei.fMask = SEE_MASK_NOCLOSEPROCESS;
        sei.lpVerb = verb.as_ptr();
        sei.lpFile = exe_w.as_ptr();
        sei.lpParameters = params.as_ptr();
        sei.lpDirectory = dir.as_ptr();
        sei.nShow = SW_HIDE;
        if ShellExecuteExW(&mut sei) == 0 {
            let err = GetLastError();
            if err == 1223 {
                return Err(HelperError::UacCancelled);
            }
            return Err(HelperError::Failed(format!("ShellExecuteEx={err}")));
        }
        if sei.hProcess.is_null() {
            return Err(HelperError::Failed("süreç tutamacı yok".into()));
        }
        WaitForSingleObject(sei.hProcess, INFINITE);
        let mut code: u32 = 0;
        GetExitCodeProcess(sei.hProcess, &mut code);
        CloseHandle(sei.hProcess);
        Ok(code as i32)
    }
}

/// Kullanıcı akışı: zaten adlıysa geç, değilse normal dene, olmazsa UAC.
pub fn rename_all() -> RenameOutcome {
    let Some((render, capture)) = find_cable() else {
        return RenameOutcome::NotFound;
    };
    if names_applied() {
        return RenameOutcome::AlreadyNamed;
    }
    let (hr_out, hr_in) = try_rename_both(&render, &capture);
    if hr_out == 0 && hr_in == 0 && wait_for_names(1500) {
        return RenameOutcome::Renamed;
    }
    match run_elevated_helper() {
        Ok(0) => {
            if wait_for_names(2500) {
                RenameOutcome::Renamed
            } else {
                RenameOutcome::Failed("adlar doğrulanamadı".into())
            }
        }
        Ok(2) => RenameOutcome::NotFound,
        Ok(3) => RenameOutcome::AccessDenied,
        Ok(code) => RenameOutcome::Failed(format!(
            "exit={code} {}",
            last_log_hint().unwrap_or_default()
        )),
        Err(HelperError::UacCancelled) => RenameOutcome::UacCancelled,
        Err(HelperError::Failed(e)) => RenameOutcome::Failed(e),
    }
}

// ------------------------------------------------------------------ log

pub fn log_path() -> PathBuf {
    std::env::temp_dir().join("voxora-rename.log")
}

pub fn log(message: &str) {
    use std::io::Write;
    let stamp = chrono_like_now();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(file, "{stamp} {message}");
    }
}

fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let day = secs % 86_400;
    format!(
        "{:02}:{:02}:{:02}.000",
        day / 3600,
        (day % 3600) / 60,
        day % 60
    )
}

fn last_log_hint() -> Option<String> {
    let text = std::fs::read_to_string(log_path()).ok()?;
    text.lines().last().map(|l| l.to_string())
}

fn short(id: &str) -> String {
    if id.len() <= 12 {
        id.to_string()
    } else {
        format!("…{}", &id[id.len() - 10..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_endpoint_ids() {
        assert_eq!(
            endpoint_id(Flow::Render, "{091e64a4-fe14-4d96-90ab-e3202d26e652}"),
            "{0.0.0.00000000}.{091e64a4-fe14-4d96-90ab-e3202d26e652}"
        );
        assert_eq!(
            endpoint_id(Flow::Capture, "091e64a4-fe14-4d96-90ab-e3202d26e652"),
            "{0.0.1.00000000}.{091e64a4-fe14-4d96-90ab-e3202d26e652}"
        );
        let full = "{0.0.0.00000000}.{091e64a4-fe14-4d96-90ab-e3202d26e652}";
        assert_eq!(endpoint_id(Flow::Render, full), full);
    }

    #[test]
    fn recognizes_voxora_names() {
        assert!(is_voxora_name("Voxora In Audio"));
        assert!(is_voxora_name("voxora out audio (VB-Audio)"));
        assert!(!is_voxora_name("CABLE Input"));
    }

    #[test]
    fn finds_cable_or_reports_none() {
        // Donanıma bağlı: yalnızca tutarlılık kontrolü (panik yok).
        if let Some((render, capture)) = find_cable() {
            assert_eq!(render.flow, Flow::Render);
            assert_eq!(capture.flow, Flow::Capture);
            assert!(render.id.contains("0.0.0"));
            assert!(capture.id.contains("0.0.1"));
        }
    }
}
