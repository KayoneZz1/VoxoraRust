#![allow(unsafe_op_in_unsafe_fn)]

//! Tepsi simgesi (Shell_NotifyIcon) + sağ tık menüsü.
//!
//! Mesaj penceresi Win32 tarafında oluşturulur; menü seçimleri kanal ile
//! uygulamaya iletilir (ui thread'i bloklanmaz).

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    ExtractIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, HMENU, LoadIconW, MF_SEPARATOR, MF_STRING,
    PostQuitMessage, RegisterClassW, SetForegroundWindow, TPM_RIGHTBUTTON, TPM_RETURNCMD,
    TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW, MSG,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{IDI_APPLICATION, WM_NULL};

pub const WM_TRAY: u32 = WM_APP + 1;
const IDI_TRAY: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    Show,
    StopAll,
    Radial,
    ToggleMic,
    Exit,
}

static TX: OnceLock<Sender<TrayCommand>> = OnceLock::new();

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct Tray {
    pub rx: Receiver<TrayCommand>,
}

impl Tray {
    /// Tepsiyi başlatır; çağrı thread'i Win32 mesaj döngüsünü çalıştırır.
    pub fn start(tooltip: &str) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = TX.set(tx);
        let tip = tooltip.to_string();
        std::thread::Builder::new()
            .name("voxora-tray".into())
            .spawn(move || unsafe { run(tip) })
            .ok();
        Self { rx }
    }
}

unsafe fn run(tooltip: String) {
    let module = GetModuleHandleW(std::ptr::null());
    let class_name = wide("VoxoraTrayWindow");
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wndproc),
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
        WS_EX_TOOLWINDOW,
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
        return;
    }

    let exe: Vec<u16> = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut icon = ExtractIconW(module, exe.as_ptr(), 0);
    if icon.is_null() || icon as isize == 1 {
        icon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
    }

    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = IDI_TRAY as u32;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon;
    let tip_w = wide(&tooltip);
    for (i, c) in tip_w.iter().take(127).enumerate() {
        nid.szTip[i] = *c;
    }
    Shell_NotifyIconW(NIM_ADD, &nid);

    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    Shell_NotifyIconW(NIM_DELETE, &nid);
    DestroyWindow(hwnd);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let event = lparam as u32;
            if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
                show_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK {
                send(TrayCommand::Show);
            }
            0
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            match id {
                1 => send(TrayCommand::Show),
                2 => send(TrayCommand::StopAll),
                3 => send(TrayCommand::Radial),
                4 => send(TrayCommand::ToggleMic),
                5 => send(TrayCommand::Exit),
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let menu: HMENU = CreatePopupMenu();
    AppendMenuW(menu, MF_STRING, 1, wide("Voxora'yı aç").as_ptr());
    AppendMenuW(menu, MF_STRING, 2, wide("Tüm sesleri durdur").as_ptr());
    AppendMenuW(menu, MF_STRING, 3, wide("Radial menü (F1)").as_ptr());
    AppendMenuW(menu, MF_STRING, 4, wide("Mikrofonu aç / kapat").as_ptr());
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(menu, MF_STRING, 5, wide("Çıkış").as_ptr());
    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(
        menu,
        TPM_RIGHTBUTTON | TPM_RETURNCMD,
        pt.x,
        pt.y,
        0,
        hwnd,
        std::ptr::null(),
    ) as usize;
    DestroyMenu(menu);
    match cmd {
        1 => send(TrayCommand::Show),
        2 => send(TrayCommand::StopAll),
        3 => send(TrayCommand::Radial),
        4 => send(TrayCommand::ToggleMic),
        5 => send(TrayCommand::Exit),
        _ => {}
    }
    // Menü kapanışında boş mesajla kuyruğu temizle (Win32 kuralı).
    let _ = windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(hwnd, WM_NULL, 0, 0);
}

fn send(command: TrayCommand) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(command);
    }
    crate::platform::wake();
}

#[allow(dead_code)]
fn _assert_send(_: *mut c_void) {}
