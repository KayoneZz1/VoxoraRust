#![allow(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use voxora_core::models::{HotkeyGesture, HotkeyModifiers};

use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentThreadId};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG,
    PostThreadMessageW, RegisterClassW, TranslateMessage, WM_APP, WM_HOTKEY, WNDCLASSW,
};

#[derive(Clone, Debug)]
pub enum HotkeyAction {
    Play(String),
    StopAll,
    Radial,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum HotkeyEvent {
    Pressed(HotkeyAction),
    Released(HotkeyAction),
}

const WM_MAP_CHANGED: u32 = WM_APP + 10;

static HOTKEY_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static HOTKEY_TID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static HOTKEY_TX: OnceLock<mpsc::Sender<HotkeyEvent>> = OnceLock::new();
static HOTKEY_ACTIONS: OnceLock<Arc<Mutex<Vec<(u32, HotkeyAction)>>>> = OnceLock::new();
static SUPPRESS: AtomicBool = AtomicBool::new(true);

pub struct Hotkeys {
    pub rx: mpsc::Receiver<HotkeyEvent>,
    map_tx: mpsc::Sender<Vec<(u32, HotkeyGesture, HotkeyAction)>>,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

impl Hotkeys {
    pub fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let _ = HOTKEY_TX.set(tx);
        let (map_tx, map_rx) = mpsc::channel();
        let actions = HOTKEY_ACTIONS
            .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
            .clone();
        std::thread::Builder::new()
            .name("voxora-hotkeys".into())
            .spawn(move || unsafe {
                let module = GetModuleHandleW(std::ptr::null());
                let class_name = wide("VoxoraHotkeyWindow");
                let wc = WNDCLASSW {
                    style: 0,
                    lpfnWndProc: Some(hotkey_wndproc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: module,
                    hIcon: std::ptr::null_mut(),
                    hCursor: std::ptr::null_mut(),
                    hbrBackground: std::ptr::null_mut(),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                };
                RegisterClassW(&wc);
                let hwnd = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    class_name.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    module,
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    eprintln!("voxora: hotkey penceresi oluşturulamadı");
                    return;
                }
                HOTKEY_HWND.store(hwnd as isize, Ordering::Relaxed);
                HOTKEY_TID.store(GetCurrentThreadId(), Ordering::Relaxed);

                while let Ok(list) = map_rx.try_recv() {
                    apply_map(hwnd, &actions, list);
                }
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    if msg.message == WM_MAP_CHANGED {
                        while let Ok(list) = map_rx.try_recv() {
                            apply_map(hwnd, &actions, list);
                        }
                        continue;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .ok();
        Self { rx, map_tx }
    }

    pub fn update(&self, map: HashMap<HotkeyGesture, HotkeyAction>, suppress: bool) {
        SUPPRESS.store(suppress, Ordering::Relaxed);
        let mut list: Vec<(u32, HotkeyGesture, HotkeyAction)> = Vec::new();
        let mut id = 1u32;
        for (gesture, action) in map {
            if gesture.is_valid() {
                list.push((id, gesture, action));
                id += 1;
            }
        }
        let _ = self.map_tx.send(list);
        let tid = HOTKEY_TID.load(Ordering::Relaxed);
        if tid != 0 {
            unsafe {
                PostThreadMessageW(tid, WM_MAP_CHANGED, 0, 0);
            }
        }
    }

    pub fn poll(&self) -> Vec<HotkeyEvent> {
        self.rx.try_iter().collect()
    }
}

unsafe fn apply_map(
    hwnd: HWND,
    actions: &Arc<Mutex<Vec<(u32, HotkeyAction)>>>,
    list: Vec<(u32, HotkeyGesture, HotkeyAction)>,
) {
    if let Ok(mut current) = actions.lock() {
        for (id, _) in current.iter() {
            UnregisterHotKey(hwnd, *id as i32);
        }
        current.clear();
        for (id, gesture, action) in list {
            let mods = (if gesture.has(HotkeyModifiers::Alt) {
                MOD_ALT
            } else {
                0
            }) | (if gesture.has(HotkeyModifiers::Ctrl) {
                MOD_CONTROL
            } else {
                0
            }) | (if gesture.has(HotkeyModifiers::Shift) {
                MOD_SHIFT
            } else {
                0
            }) | (if gesture.has(HotkeyModifiers::Win) {
                MOD_WIN
            } else {
                0
            }) | MOD_NOREPEAT;
            if RegisterHotKey(hwnd, id as i32, mods, gesture.virtual_key as u32) != 0 {
                current.push((id, action));
            }
        }
    }
}

unsafe extern "system" fn hotkey_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            let id = wparam as u32;
            if let Some(action) = HOTKEY_ACTIONS
                .get()
                .and_then(|a| a.lock().ok())
                .and_then(|a| {
                    a.iter()
                        .find(|(hid, _)| *hid == id)
                        .map(|(_, act)| act.clone())
                })
            {
                if let HotkeyAction::Radial = action {
                    if let Some(handler) = RADIAL_HANDLER.get() {
                        handler();
                    } else if let Some(tx) = HOTKEY_TX.get() {
                        let _ = tx.send(HotkeyEvent::Pressed(action));
                    }
                } else if let Some(tx) = HOTKEY_TX.get() {
                    let _ = tx.send(HotkeyEvent::Pressed(action));
                }
                wake();
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Kısayol tuşu hâlâ basılı mı? (basılı tutarak çalma için)
#[allow(dead_code)]
pub fn gesture_down(gesture: &HotkeyGesture) -> bool {
    unsafe {
        let key = GetAsyncKeyState(gesture.virtual_key) as u16 & 0x8000 != 0;
        if !key {
            return false;
        }
        let ctrl = GetAsyncKeyState(0x11) as u16 & 0x8000 != 0;
        let alt = GetAsyncKeyState(0x12) as u16 & 0x8000 != 0;
        let shift = GetAsyncKeyState(0x10) as u16 & 0x8000 != 0;
        let win = (GetAsyncKeyState(0x5B) as u16 & 0x8000 != 0)
            || (GetAsyncKeyState(0x5C) as u16 & 0x8000 != 0);
        let mods = (if ctrl {
            HotkeyModifiers::Ctrl as u32
        } else {
            0
        }) | (if alt {
            HotkeyModifiers::Alt as u32
        } else {
            0
        }) | (if shift {
            HotkeyModifiers::Shift as u32
        } else {
            0
        }) | (if win {
            HotkeyModifiers::Win as u32
        } else {
            0
        });
        mods == gesture.modifiers
    }
}

pub struct SingleInstance {
    handle: isize,
}

impl SingleInstance {
    pub fn acquire(name: &str) -> Option<Self> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let handle = CreateMutexW(std::ptr::null(), 1, wide.as_ptr());
            if handle.is_null() {
                return None;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                return None;
            }
            Some(Self {
                handle: handle as isize,
            })
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        let _ = self.handle;
    }
}

#[derive(Clone)]
pub struct UpdateInfo {
    pub latest: String,
    pub url: String,
}

pub fn check_update(current: &str) -> Option<UpdateInfo> {
    let response = ureq::get("https://api.github.com/repos/ObsQrr/VoxoraRust/releases/latest")
        .set("User-Agent", "VoxoraRust/1.0.0")
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .ok()?;
    let value: serde_json::Value = serde_json::from_reader(response.into_reader()).ok()?;
    let tag = value.get("tag_name")?.as_str()?;
    let url = value.get("html_url")?.as_str()?.to_string();
    let latest = tag.trim_start_matches('v').to_string();
    if version_tuple(&latest) > version_tuple(current) {
        Some(UpdateInfo { latest, url })
    } else {
        None
    }
}

fn version_tuple(value: &str) -> (u64, u64, u64, u64) {
    let mut parts = value.split('.').map(|p| {
        p.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u64>()
            .unwrap_or(0)
    });
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

pub fn set_autostart(enabled: bool) {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok((key, _)) = hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run") {
        if enabled {
            if let Ok(exe) = std::env::current_exe() {
                let _ = key.set_value("Voxora", &format!("\"{}\"", exe.display()));
            }
        } else {
            let _ = key.delete_value("Voxora");
        }
    }
}

static TITLE_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Windows 11 başlık çubuğunu temaya boyar (DWM). Pencere bulunana kadar kısa süre dener.
pub fn apply_titlebar(dark: bool, caption: u32, text: u32, border: u32) {
    std::thread::spawn(move || unsafe {
        use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
        use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
        let mut hwnd =
            TITLE_HWND.load(std::sync::atomic::Ordering::Relaxed) as *mut std::ffi::c_void;
        let title = wide("Voxora");
        if hwnd.is_null() {
            for _ in 0..40 {
                hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
                if !hwnd.is_null() {
                    TITLE_HWND.store(hwnd as isize, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        if hwnd.is_null() {
            return;
        }
        let dark_flag: i32 = if dark { 1 } else { 0 };
        let _ = DwmSetWindowAttribute(hwnd, 20, &dark_flag as *const _ as *const _, 4);
        let _ = DwmSetWindowAttribute(hwnd, 35, &caption as *const _ as *const _, 4);
        let _ = DwmSetWindowAttribute(hwnd, 36, &text as *const _ as *const _, 4);
        let _ = DwmSetWindowAttribute(hwnd, 34, &border as *const _ as *const _, 4);
    });
}

/// Tema paletini DWM renklerine çevirir.
pub fn titlebar_colors(dark: bool) -> (u32, u32, u32) {
    if dark {
        (0x000E_1114, 0x00E3_EDF4, 0x001E_252C)
    } else {
        (0x00ED_F4F8, 0x0012_1519, 0x00C8_D6DF)
    }
}

/// HTTPS dosyasını hedefe indirir; ilerlemeyi kanaldan bildirir.
#[allow(dead_code)]
pub fn download_file(
    url: &str,
    target: &std::path::Path,
    progress: std::sync::mpsc::Sender<(u64, u64)>,
) -> Result<(), String> {
    let response = ureq::get(url)
        .set("User-Agent", "VoxoraRust/1.0.0")
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| e.to_string())?;
    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let mut reader = response.into_reader();
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut file = std::fs::File::create(target).map_err(|e| e.to_string())?;
    use std::io::{Read, Write};
    let mut buf = vec![0u8; 64 * 1024];
    let mut received = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        received += n as u64;
        let _ = progress.send((received, total));
    }
    Ok(())
}

// ------------------------------------------------------------------ radyal katman

static REPAINT_CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Olay geldiğinde arayüzü hemen uyandır (boşta düşük FPS için).
pub fn set_repaint_context(ctx: egui::Context) {
    let _ = REPAINT_CTX.set(ctx);
}

pub fn wake() {
    if let Some(ctx) = REPAINT_CTX.get() {
        ctx.request_repaint();
    }
}

static RADIAL_HANDLER: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> =
    std::sync::OnceLock::new();

/// F1 gibi radyal kısayol, UI karesini beklemeden doğrudan bu işleyiciyi çağırır.
pub fn set_radial_handler(handler: Box<dyn Fn() + Send + Sync>) {
    let _ = RADIAL_HANDLER.set(handler);
}

static RADIAL_ACTIVE: AtomicBool = AtomicBool::new(false);
static WHEEL_DELTA: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static WHEEL_HOOK: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static RADIAL_CX: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static RADIAL_CY: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static RADIAL_R: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(260);
static RADIAL_CLOSE: AtomicBool = AtomicBool::new(false);

/// Radyal menü açıkken tekerlek olaylarını oyuna göndermeden yakalar.
pub fn set_radial_geometry(cx: i32, cy: i32, radius: i32) {
    RADIAL_CX.store(cx, Ordering::SeqCst);
    RADIAL_CY.store(cy, Ordering::SeqCst);
    RADIAL_R.store(radius, Ordering::SeqCst);
}

pub fn take_radial_close() -> bool {
    RADIAL_CLOSE.swap(false, Ordering::SeqCst)
}

pub fn set_radial_active(active: bool) {
    RADIAL_ACTIVE.store(active, Ordering::SeqCst);
}

#[allow(dead_code)]
pub fn radial_active() -> bool {
    RADIAL_ACTIVE.load(Ordering::SeqCst)
}

/// Biriken tekerlek deltasını alır (pozitif = yukarı).
pub fn take_wheel_delta() -> i32 {
    WHEEL_DELTA.swap(0, Ordering::SeqCst)
}

#[allow(dead_code)]
pub fn virtual_screen() -> (i32, i32, i32, i32) {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

#[allow(dead_code)]
pub fn screen_size() -> (i32, i32) {
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        (
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN),
        )
    }
}

pub fn cursor_pos() -> (i32, i32) {
    unsafe {
        let mut pt = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
        windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
        (pt.x, pt.y)
    }
}

/// Düşük seviyeli fare kancası: yalnızca radyal menü açıkken tekerleği yer.
pub fn install_wheel_interceptor() {
    if WHEEL_HOOK.load(Ordering::SeqCst) != 0 {
        return;
    }
    std::thread::Builder::new()
        .name("voxora-wheel-hook".into())
        .spawn(|| unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
                MSLLHOOKSTRUCT, MSG, WH_MOUSE_LL, WM_MOUSEWHEEL,
            };
            unsafe extern "system" fn hook(code: i32, wparam: usize, lparam: isize) -> isize {
                if code >= 0 {
                    let msg = wparam as u32;
                    let info = &*(lparam as *const MSLLHOOKSTRUCT);
                    if msg == WM_MOUSEWHEEL && RADIAL_ACTIVE.load(Ordering::SeqCst) {
                        let delta = ((info.mouseData >> 16) & 0xFFFF) as i16 as i32;
                        WHEEL_DELTA.fetch_add(delta, Ordering::SeqCst);
                        return 1;
                    }
                    let button = msg == 0x0201 || msg == 0x0204;
                    if button && RADIAL_ACTIVE.load(Ordering::SeqCst) {
                        let dx = info.pt.x - RADIAL_CX.load(Ordering::SeqCst);
                        let dy = info.pt.y - RADIAL_CY.load(Ordering::SeqCst);
                        let inside = dx * dx + dy * dy
                            <= RADIAL_R.load(Ordering::SeqCst) * RADIAL_R.load(Ordering::SeqCst);
                        if msg == 0x0204 || !inside {
                            RADIAL_CLOSE.store(true, Ordering::SeqCst);
                            return 1;
                        }
                    }
                }
                CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
            }
            let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(hook), std::ptr::null_mut(), 0);
            if hook.is_null() {
                return;
            }
            WHEEL_HOOK.store(hook as isize, Ordering::SeqCst);
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .ok();
}

/// Radyal pencereyi odak çalmayacak şekilde işaretler (NOACTIVATE + TOOLWINDOW).
#[allow(dead_code)]
pub fn mark_radial_window() {
    std::thread::spawn(|| unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            FindWindowW, GetWindowLongPtrW, SetWindowLongPtrW,
            GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };
        let title: Vec<u16> = "Voxora Radial"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        for _ in 0..40 {
            let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
            if !hwnd.is_null() {
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let style = style
                    | (WS_EX_NOACTIVATE as isize)
                    | (WS_EX_TOOLWINDOW as isize);
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
    });
}
