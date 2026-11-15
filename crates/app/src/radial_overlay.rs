//! Şeffaf (piksel başına alfa) native radyal menü katmanı.
//!
//! C# RadialMenuWindow ile aynı davranış: arka plan tamamen şeffaf, yalnızca
//! daire görünür; `WS_EX_NOACTIVATE` ile odak çalınmaz. Pencere açılışta bir kez
//! oluşturulup gizlenir, F1'de anında gösterilir.

#![allow(unsafe_op_in_unsafe_fn)]

use std::sync::mpsc::{Receiver, Sender, channel};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HGDIOBJ,
    ReleaseDC, SelectObject,
};
use windows_sys::Win32::Graphics::GdiPlus::{
    FillModeAlternate, GdiplusStartup, GdiplusStartupInput, GdipCreateBitmapFromScan0,
    GdipCreateFont, GdipCreateFontFamilyFromName, GdipCreateSolidFill, GdipCreateStringFormat,
    GdipDeleteBrush, GdipDeleteGraphics,
    GdipDisposeImage, GdipDrawString, GdipFillEllipse, GdipFillPolygon,
    GdipGetImageGraphicsContext, GdipSetSmoothingMode, GdipSetStringFormatAlign,
    GdipSetStringFormatLineAlign, GdipSetTextRenderingHint, GpBitmap, GpBrush, GpFont,
    GpFontFamily, GpGraphics, GpImage, GpStringFormat, PointF, RectF, SmoothingModeAntiAlias,
    StringAlignmentCenter, TextRenderingHintAntiAlias, UnitPixel,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GWLP_USERDATA, HWND_TOPMOST,
    KillTimer, MSG, PostMessageW, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SetTimer,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA, UpdateLayeredWindow,
    WM_APP, WM_CLOSE, WM_LBUTTONUP, WM_MOUSEMOVE, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const PIXEL_FORMAT_PARGB: i32 = 0x000E_200B;
const WM_COMMAND_MSG: u32 = WM_APP + 1;

const SIZE_PX: i32 = 640;
const CX: f32 = 320.0;
const CY: f32 = 320.0;
const INNER: f32 = 92.0;
const OUTER: f32 = 238.0;
const PAGE: usize = 8;

const C_BG: u32 = 0xF0_0E0C0A;
const C_WEDGE: u32 = 0xEB_241F19;
const C_ACCENT: u32 = 0xF0_FF5C35;
const C_CENTER: u32 = 0xFA_14110E;
const C_INK: u32 = 0xFF_F4EDE3;
const C_ON_ACCENT: u32 = 0xFF_FFFFFF;
const C_MUTED: u32 = 0xFF_8C8073;
const C_ACCENT_TEXT: u32 = 0xFF_FF5C35;

#[derive(Clone, Debug)]
pub enum RadialEvent {
    Selected(String),
    Closed,
}

enum Command {
    Show(Vec<(String, String)>, i32, i32),
    Hide,
}

pub struct RadialOverlay {
    tx: Sender<Command>,
    events: std::sync::Mutex<Receiver<RadialEvent>>,
    open: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl RadialOverlay {
    pub fn start() -> Self {
        let (tx, rx) = channel();
        let (evt_tx, events) = channel();
        let open = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = open.clone();
        std::thread::Builder::new()
            .name("voxora-radial".into())
            .spawn(move || unsafe { run(rx, evt_tx, flag) })
            .ok();
        Self {
            tx,
            events: std::sync::Mutex::new(events),
            open,
        }
    }

    pub fn try_recv(&self) -> Result<RadialEvent, std::sync::mpsc::TryRecvError> {
        match self.events.lock() {
            Ok(rx) => rx.try_recv(),
            Err(_) => Err(std::sync::mpsc::TryRecvError::Disconnected),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn show(&self, items: Vec<(String, String)>, x: i32, y: i32) {
        let _ = self.tx.send(Command::Show(items, x, y));
        let hwnd = WINDOW.load(std::sync::atomic::Ordering::Relaxed);
        if hwnd != 0 {
            unsafe {
                PostMessageW(hwnd as HWND, WM_COMMAND_MSG, 0, 0);
            }
        }
    }

    pub fn hide(&self) {
        let _ = self.tx.send(Command::Hide);
        let hwnd = WINDOW.load(std::sync::atomic::Ordering::Relaxed);
        if hwnd != 0 {
            unsafe {
                PostMessageW(hwnd as HWND, WM_COMMAND_MSG, 0, 0);
            }
        }
    }
}

static WINDOW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct State {
    hwnd: HWND,
    items: Vec<(String, String)>,
    page: usize,
    hover: Option<usize>,
    evt: Sender<RadialEvent>,
    open_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _family: *mut GpFontFamily,
    font_small: *mut GpFont,
    font_name: *mut GpFont,
    font_bold: *mut GpFont,
    format: *mut GpStringFormat,
}

unsafe fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn run(
    rx: Receiver<Command>,
    evt: Sender<RadialEvent>,
    open_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut token: usize = 0;
    let input = GdiplusStartupInput {
        GdiplusVersion: 1,
        DebugEventCallback: 0,
        SuppressBackgroundThread: 0,
        SuppressExternalCodecs: 0,
    };
    if GdiplusStartup(&mut token, &input, std::ptr::null_mut()) != 0 {
        return;
    }
    let module = GetModuleHandleW(std::ptr::null());
    let class_name = wide("VoxoraRadialWindow");
    let title = wide("Voxora Radial");
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
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_POPUP,
        0,
        0,
        SIZE_PX,
        SIZE_PX,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        module,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return;
    }
    WINDOW.store(hwnd as usize, std::sync::atomic::Ordering::Relaxed);

    let family_name = wide("Bahnschrift");
    let mut family: *mut GpFontFamily = std::ptr::null_mut();
    if GdipCreateFontFamilyFromName(family_name.as_ptr(), std::ptr::null_mut(), &mut family) != 0 {
        let fallback = wide("Segoe UI");
        GdipCreateFontFamilyFromName(fallback.as_ptr(), std::ptr::null_mut(), &mut family);
    }
    let mut font_small: *mut GpFont = std::ptr::null_mut();
    let mut font_name: *mut GpFont = std::ptr::null_mut();
    let mut font_bold: *mut GpFont = std::ptr::null_mut();
    if !family.is_null() {
        GdipCreateFont(family, 12.0, 0, UnitPixel, &mut font_small);
        GdipCreateFont(family, 13.0, 0, UnitPixel, &mut font_name);
        GdipCreateFont(family, 15.0, 1, UnitPixel, &mut font_bold);
    }
    let mut format: *mut GpStringFormat = std::ptr::null_mut();
    GdipCreateStringFormat(0, 0, &mut format);
    GdipSetStringFormatAlign(format, StringAlignmentCenter);
    GdipSetStringFormatLineAlign(format, StringAlignmentCenter);

    let state = Box::new(State {
        hwnd,
        items: Vec::new(),
        page: 0,
        hover: None,
        evt,
        open_flag,
        _family: family,
        font_small,
        font_name,
        font_bold,
        format,
    });
    let ptr = Box::into_raw(state);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);

    let mut msg: MSG = std::mem::zeroed();
    loop {
        let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
        if ret <= 0 {
            break;
        }
        if msg.message == WM_COMMAND_MSG {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    Command::Show(items, x, y) => {
                        let st = &mut *ptr;
                        st.items = items;
                        st.page = 0;
                        st.hover = None;
                        SetWindowPos(
                            hwnd,
                            HWND_TOPMOST,
                            x - SIZE_PX / 2,
                            y - SIZE_PX / 2,
                            SIZE_PX,
                            SIZE_PX,
                            0,
                        );
                        crate::platform::set_radial_geometry(x, y, 250);
                        crate::platform::set_radial_active(true);
                        st.open_flag
                            .store(true, std::sync::atomic::Ordering::SeqCst);
                        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                        SetTimer(hwnd, 1, 12_000, None);
                        SetTimer(hwnd, 2, 30, None);
                        st.render();
                    }
                    Command::Hide => hide(&mut *ptr),
                }
            }
            continue;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    let _ = token;
}

unsafe fn hide(state: &mut State) {
    state
        .open_flag
        .store(false, std::sync::atomic::Ordering::SeqCst);
    KillTimer(state.hwnd, 1);
    KillTimer(state.hwnd, 2);
    ShowWindow(state.hwnd, SW_HIDE);
    crate::platform::set_radial_active(false);
    let _ = state.evt.send(RadialEvent::Closed);
}

impl State {
    unsafe fn render(&self) {
        let size = SIZE_PX;
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let screen = GetDC(std::ptr::null_mut());
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let hbmp = CreateDIBSection(
            screen,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        if hbmp.is_null() || bits.is_null() {
            ReleaseDC(std::ptr::null_mut(), screen);
            return;
        }
        std::ptr::write_bytes(bits as *mut u8, 0, (size * size * 4) as usize);
        let memdc = CreateCompatibleDC(screen);
        let old = SelectObject(memdc, hbmp as HGDIOBJ);

        let mut bitmap: *mut GpBitmap = std::ptr::null_mut();
        GdipCreateBitmapFromScan0(
            size,
            size,
            size * 4,
            PIXEL_FORMAT_PARGB,
            bits as *const u8,
            &mut bitmap,
        );
        let mut g: *mut GpGraphics = std::ptr::null_mut();
        GdipGetImageGraphicsContext(bitmap as *mut GpImage, &mut g);
        GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
        GdipSetTextRenderingHint(g, TextRenderingHintAntiAlias);

        // Daire zemini
        fill_ellipse(g, C_BG, CX - OUTER, CY - OUTER, OUTER * 2.0, OUTER * 2.0);

        let pages = self.items.len().div_ceil(PAGE).max(1);
        let slice_start = self.page * PAGE;
        let slice_end = (slice_start + PAGE).min(self.items.len());
        let count = slice_end - slice_start;
        let step = if count > 0 { 360.0 / count as f32 } else { 360.0 };

        for i in 0..count {
            let start = -90.0 + i as f32 * step + 1.2;
            let end = start + step - 2.4;
            let hovered = self.hover == Some(i);
            let color = if hovered { C_ACCENT } else { C_WEDGE };
            let mut pts: Vec<PointF> = Vec::with_capacity(26);
            let steps = 12;
            for k in 0..=steps {
                let a = (start + (end - start) * k as f32 / steps as f32).to_radians();
                pts.push(PointF {
                    X: CX + a.cos() * OUTER,
                    Y: CY + a.sin() * OUTER,
                });
            }
            for k in (0..=steps).rev() {
                let a = (start + (end - start) * k as f32 / steps as f32).to_radians();
                pts.push(PointF {
                    X: CX + a.cos() * INNER * 0.92,
                    Y: CY + a.sin() * INNER * 0.92,
                });
            }
            fill_polygon(g, color, &pts);

            if let Some((_, name)) = self.items.get(slice_start + i) {
                let mid = ((start + end) / 2.0).to_radians();
                let r = (INNER + OUTER) / 2.0;
                let tx = CX + mid.cos() * r;
                let ty = CY + mid.sin() * r;
                let label = truncate(name, 16);
                self.text(g, self.font_name, &label, tx, ty, 168.0, 36.0,
                    if hovered { C_ON_ACCENT } else { C_INK });
            }
        }

        // Merkez
        fill_ellipse(g, C_CENTER, CX - 84.0, CY - 84.0, 168.0, 168.0);
        self.text(g, self.font_bold, "VOXORA", CX, CY - 16.0, 160.0, 30.0, C_ACCENT_TEXT);
        if pages > 1 {
            self.text(
                g,
                self.font_small,
                &format!("TEKERLEK · {}/{}", self.page + 1, pages),
                CX,
                CY + 6.0,
                160.0,
                18.0,
                C_MUTED,
            );
        }
        self.text(
            g,
            self.font_small,
            "F1 / sağ tık: kapat",
            CX,
            CY + 30.0,
            180.0,
            18.0,
            C_MUTED,
        );

        let mut dst = POINT {
            x: 0,
            y: 0,
        };
        // Pencere konumu SetWindowPos ile verildi; UpdateLayeredWindow konumu korur.
        let _ = &mut dst;
        let src = POINT { x: 0, y: 0 };
        let sz = SIZE {
            cx: size,
            cy: size,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // GetWindowRect ile mevcut konumu al.
        let mut rect: windows_sys::Win32::Foundation::RECT = std::mem::zeroed();
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(self.hwnd, &mut rect);
        let dst = POINT {
            x: rect.left,
            y: rect.top,
        };
        UpdateLayeredWindow(
            self.hwnd,
            screen,
            &dst,
            &sz,
            memdc,
            &src,
            0,
            &blend,
            ULW_ALPHA,
        );

        GdipDeleteGraphics(g);
        GdipDisposeImage(bitmap as *mut GpImage);
        SelectObject(memdc, old);
        DeleteDC(memdc);
        DeleteObject(hbmp as HGDIOBJ);
        ReleaseDC(std::ptr::null_mut(), screen);
    }

    unsafe fn text(
        &self,
        g: *mut GpGraphics,
        font: *mut GpFont,
        text: &str,
        cx: f32,
        cy: f32,
        w: f32,
        h: f32,
        color: u32,
    ) {
        if g.is_null() || font.is_null() {
            return;
        }
        let mut brush: *mut windows_sys::Win32::Graphics::GdiPlus::GpSolidFill =
            std::ptr::null_mut();
        if GdipCreateSolidFill(color, &mut brush) != 0 {
            return;
        }
        let rect = RectF {
            X: cx - w / 2.0,
            Y: cy - h / 2.0,
            Width: w,
            Height: h,
        };
        let mut utf: Vec<u16> = text.encode_utf16().collect();
        if utf.is_empty() {
            utf.push(' ' as u16);
        }
        GdipDrawString(
            g,
            utf.as_ptr(),
            utf.len() as i32,
            font,
            &rect,
            self.format,
            brush as *mut GpBrush,
        );
        GdipDeleteBrush(brush as *mut GpBrush);
    }
}

unsafe fn fill_ellipse(g: *mut GpGraphics, color: u32, x: f32, y: f32, w: f32, h: f32) {
    let mut brush: *mut windows_sys::Win32::Graphics::GdiPlus::GpSolidFill = std::ptr::null_mut();
    if GdipCreateSolidFill(color, &mut brush) != 0 {
        return;
    }
    GdipFillEllipse(g, brush as *mut GpBrush, x, y, w, h);
    GdipDeleteBrush(brush as *mut GpBrush);
}

unsafe fn fill_polygon(g: *mut GpGraphics, color: u32, pts: &[PointF]) {
    let mut brush: *mut windows_sys::Win32::Graphics::GdiPlus::GpSolidFill = std::ptr::null_mut();
    if GdipCreateSolidFill(color, &mut brush) != 0 {
        return;
    }
    GdipFillPolygon(
        g,
        brush as *mut GpBrush,
        pts.as_ptr(),
        pts.len() as i32,
        FillModeAlternate,
    );
    GdipDeleteBrush(brush as *mut GpBrush);
}

fn hit_test(x: f32, y: f32, page: usize, total: usize) -> Option<usize> {
    let dx = x - CX;
    let dy = y - CY;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < INNER * 0.92 || dist > OUTER {
        return None;
    }
    let count = (total.saturating_sub(page * PAGE)).min(PAGE);
    if count == 0 {
        return None;
    }
    let step = 360.0 / count as f32;
    let angle = dy.atan2(dx).to_degrees();
    let rel = (angle + 90.0 + 360.0) % 360.0;
    let idx = (rel / step) as usize;
    if idx < count { Some(idx) } else { None }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let ptr =
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA)
            as *mut State;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let st = &mut *ptr;
    match msg {
        WM_MOUSEMOVE => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let hit = hit_test(x as f32, y as f32, st.page, st.items.len());
            if hit != st.hover {
                st.hover = hit;
                st.render();
            }
            0
        }
        WM_LBUTTONUP => {
            if let Some(idx) = st.hover {
                let global = st.page * PAGE + idx;
                if let Some((id, _)) = st.items.get(global) {
                    let _ = st.evt.send(RadialEvent::Selected(id.clone()));
                    crate::platform::wake();
                }
            }
            hide(st);
            0
        }
        WM_RBUTTONUP => {
            hide(st);
            0
        }
        WM_TIMER => {
            if wparam == 1 {
                hide(st);
            } else {
                let delta = crate::platform::take_wheel_delta();
                let pages = st.items.len().div_ceil(PAGE).max(1);
                if delta != 0 && pages > 1 {
                    let step = if delta > 0 { -1 } else { 1 };
                    st.page = ((st.page as i32 + step).rem_euclid(pages as i32)) as usize;
                    st.hover = None;
                    st.render();
                }
                if crate::platform::take_radial_close() {
                    hide(st);
                }
            }
            0
        }
        WM_CLOSE => {
            hide(st);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[allow(dead_code)]
unsafe fn post_state(hwnd: HWND, f: impl FnOnce(&mut State)) {
    let ptr =
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, GWLP_USERDATA)
            as *mut State;
    if !ptr.is_null() {
        f(&mut *ptr);
    }
}
