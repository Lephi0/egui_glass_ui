use crate::style::BackgroundMode;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(windows)]
use std::ffi::c_void;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::{
        Dwm::{
            DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND, DWM_SYSTEMBACKDROP_TYPE, DWMWA_CLOAK,
            DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW, DWMWA_BORDER_COLOR, DWMWA_SYSTEMBACKDROP_TYPE,
            DWMWA_TRANSITIONS_FORCEDISABLED, DWMWA_USE_IMMERSIVE_DARK_MODE,
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
            DwmEnableBlurBehindWindow, DwmExtendFrameIntoClientArea, DwmFlush,
            DwmSetWindowAttribute,
        },
        Gdi::{
            CreateRectRgn, DeleteObject, GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST,
            MONITOR_DEFAULTTONULL, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
            MonitorFromWindow,
        },
    },
    System::{
        Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
        Threading::{
            AttachThreadInput, GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId,
            SetProcessWorkingSetSize,
        },
    },
    UI::{
        Controls::{
            MARGINS, SetWindowThemeAttribute, WTA_NONCLIENT, WTA_OPTIONS, WTNCA_NODRAWCAPTION,
            WTNCA_NODRAWICON, WTNCA_NOMIRRORHELP, WTNCA_NOSYSMENU,
        },
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
            KEYEVENTF_KEYUP, SendInput, SetActiveWindow, SetFocus, VK_LWIN, VK_MENU, VK_RWIN,
        },
        WindowsAndMessaging::{
            BringWindowToTop, CallWindowProcW, DefWindowProcW, FindWindowExW, GWL_EXSTYLE,
            GWL_STYLE, GWLP_WNDPROC, GetCursorPos, GetForegroundWindow, GetPropW,
            GetSystemMetrics, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
            HWND_NOTOPMOST, HWND_TOPMOST, IsWindowVisible, RemovePropW, SM_CXSCREEN, SM_CYSCREEN,
            SW_HIDE, SW_RESTORE, SW_SHOW, SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetForegroundWindow, SetPropW,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, ShowWindowAsync, WM_NCACTIVATE,
            WM_NCDESTROY, WNDPROC, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX,
            WS_MINIMIZEBOX, WS_SYSMENU,
        },
    },
};

// DWMWA_COLOR_DEFAULT = 0xFFFFFFFF restores the default Windows 11 1px system border
#[cfg(windows)]
const DWMWA_COLOR_DEFAULT: u32 = 0xFFFFFFFF;

#[cfg(windows)]
const PROP_ORIG_WNDPROC: &[u16] = &[
    b'E' as u16, b'g' as u16, b'u' as u16, b'i' as u16, b'G' as u16, b'l' as u16, b'a' as u16,
    b's' as u16, b's' as u16, b'O' as u16, b'r' as u16, b'i' as u16, b'g' as u16, b'W' as u16,
    b'n' as u16, b'd' as u16, b'P' as u16, b'r' as u16, b'o' as u16, b'c' as u16, 0,
];

#[cfg(windows)]
const PROP_KEEP_ACTIVE: &[u16] = &[
    b'E' as u16, b'g' as u16, b'u' as u16, b'i' as u16, b'G' as u16, b'l' as u16, b'a' as u16,
    b's' as u16, b's' as u16, b'K' as u16, b'e' as u16, b'e' as u16, b'p' as u16, b'A' as u16,
    b'c' as u16, b't' as u16, b'i' as u16, b'v' as u16, b'e' as u16, 0,
];

#[cfg(windows)]
unsafe extern "system" fn glass_ncactivate_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        let orig = GetPropW(hwnd, PROP_ORIG_WNDPROC.as_ptr()) as isize;
        if orig == 0 {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let prev: WNDPROC = Some(std::mem::transmute::<
            isize,
            unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
        >(orig));

        if msg == WM_NCDESTROY {
            let _ = RemovePropW(hwnd, PROP_KEEP_ACTIVE.as_ptr());
            let _ = RemovePropW(hwnd, PROP_ORIG_WNDPROC.as_ptr());
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, orig);
            return CallWindowProcW(prev, hwnd, msg, wparam, lparam);
        }

        if msg == WM_NCACTIVATE && !GetPropW(hwnd, PROP_KEEP_ACTIVE.as_ptr()).is_null() {
            // Keep DWM Acrylic active across focus transitions; winit's WM_KILLFOCUS still updates focus state.
            if wparam == 0 {
                return CallWindowProcW(prev, hwnd, msg, 1, -1);
            }
            return CallWindowProcW(prev, hwnd, msg, 1, lparam);
        }

        CallWindowProcW(prev, hwnd, msg, wparam, lparam)
    }
}

#[cfg(windows)]
unsafe fn set_always_active_backdrop(hwnd: HWND, enabled: bool) {
    unsafe {
        if enabled {
            if GetPropW(hwnd, PROP_ORIG_WNDPROC.as_ptr()).is_null() {
                let cur = GetWindowLongPtrW(hwnd, GWLP_WNDPROC);
                let hook_ptr = glass_ncactivate_wndproc as *const () as isize;
                if cur != 0 && cur != hook_ptr {
                    let _ = SetPropW(hwnd, PROP_ORIG_WNDPROC.as_ptr(), cur as HANDLE);
                    SetWindowLongPtrW(hwnd, GWLP_WNDPROC, hook_ptr);
                }
            }
            let _ = SetPropW(hwnd, PROP_KEEP_ACTIVE.as_ptr(), 1usize as HANDLE);
        } else {
            let _ = RemovePropW(hwnd, PROP_KEEP_ACTIVE.as_ptr());
        }
    }
}

pub fn extract_hwnd(window: &(impl HasWindowHandle + ?Sized)) -> Option<isize> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
        _ => None,
    }
}

/// Queries the Windows Registry (`HKCU\...\Personalize\AppsUseLightTheme`) to detect Windows Light vs Dark mode.
pub fn is_windows_light_theme() -> bool {
    #[cfg(windows)]
    unsafe {
        let subkey: Vec<u16> =
            "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
                .encode_utf16()
                .collect();
        let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
        let mut data: u32 = 0;
        let mut size: u32 = std::mem::size_of::<u32>() as u32;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut c_void,
            &mut size,
        );
        if status == 0 {
            return data != 0;
        }
    }
    false
}

#[cfg(windows)]
const CAPTION_CONTROLS_STRIP_MASK: isize = (WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX) as isize;

/// Returns `true` if Windows caption control style bits (`WS_SYSMENU`, `WS_MINIMIZEBOX`, `WS_MAXIMIZEBOX`)
/// are currently present on the HWND and need to be stripped again.
pub fn needs_caption_suppression(hwnd_raw: Option<isize>) -> bool {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            return (style & CAPTION_CONTROLS_STRIP_MASK) != 0;
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
    false
}

/// Suppresses the Windows caption bar, icon, and Minimize/Maximize/Close (`—`, `□`, `✕`) buttons on the HWND.
/// Returns `true` if `GWL_STYLE` was modified.
#[cfg(windows)]
unsafe fn suppress_window_caption_controls(hwnd: HWND) -> bool {
    unsafe {
        // 1. Tell UxTheme never to draw caption text, system icon, or caption buttons
        let flags = WTNCA_NODRAWCAPTION | WTNCA_NODRAWICON | WTNCA_NOSYSMENU | WTNCA_NOMIRRORHELP;
        let opts = WTA_OPTIONS {
            dwFlags: flags,
            dwMask: flags,
        };
        let _ = SetWindowThemeAttribute(
            hwnd,
            WTA_NONCLIENT,
            &opts as *const WTA_OPTIONS as *const c_void,
            std::mem::size_of::<WTA_OPTIONS>() as u32,
        );

        // 2. Strip WS_SYSMENU, WS_MINIMIZEBOX, and WS_MAXIMIZEBOX from GWL_STYLE so DWM has no caption buttons to render
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let new_style = style & !CAPTION_CONTROLS_STRIP_MASK;
        if new_style != style {
            SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
            let _ = SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
            return true;
        }
        false
    }
}

/// Enables WGL OpenGL per-pixel alpha transparency and applies/removes Windows 11 Frosted Glass blur.
pub fn sync_window_blur(hwnd_raw: Option<isize>, mode: BackgroundMode, is_light: bool) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            // 1. Remove Windows caption buttons (`—`, `□`, `✕`) so they never appear in any mode
            let stripped_style = suppress_window_caption_controls(hwnd);

            // 2. Sync dark/light mode for DWM Acrylic backdrop recipe
            let dark_mode: u32 = if is_light { 0 } else { 1 };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                &dark_mode as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            let default_border: u32 = DWMWA_COLOR_DEFAULT;

            match mode {
                BackgroundMode::FrostedGlass => {
                    set_always_active_backdrop(hwnd, true);

                    // Extend frame into client area ONLY in FrostedGlass mode so Windows 11 Acrylic backdrop renders
                    let margins = MARGINS {
                        cxLeftWidth: -1,
                        cxRightWidth: -1,
                        cyTopHeight: -1,
                        cyBottomHeight: -1,
                    };
                    let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);

                    let corner_pref: u32 = DWMWCP_ROUND as u32;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                        &corner_pref as *const u32 as *const _,
                        std::mem::size_of::<u32>() as u32,
                    );

                    // Enable Windows 11 transient Acrylic frosted glass backdrop
                    let backdrop_acrylic: DWM_SYSTEMBACKDROP_TYPE = DWMSBT_TRANSIENTWINDOW;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_SYSTEMBACKDROP_TYPE as u32,
                        &backdrop_acrylic as *const _ as *const _,
                        std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
                    );
                }
                BackgroundMode::Transparent => {
                    set_always_active_backdrop(hwnd, false);

                    // Disable system backdrop blur completely
                    let backdrop_none: DWM_SYSTEMBACKDROP_TYPE = DWMSBT_NONE;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_SYSTEMBACKDROP_TYPE as u32,
                        &backdrop_none as *const _ as *const _,
                        std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
                    );

                    // CRITICAL: Use 0 margins in Transparent/Compact mode so DWM does NOT draw its
                    // dark non-client frame gradient or outer drop shadow behind the transparent text!
                    let zero_margins = MARGINS {
                        cxLeftWidth: 0,
                        cxRightWidth: 0,
                        cyTopHeight: 0,
                        cyBottomHeight: 0,
                    };
                    let _ = DwmExtendFrameIntoClientArea(hwnd, &zero_margins);

                    let corner_pref: u32 = DWMWCP_DONOTROUND as u32;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                        &corner_pref as *const u32 as *const _,
                        std::mem::size_of::<u32>() as u32,
                    );
                }
            }

            // Use the default Windows 11 1px system border
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                &default_border as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // 3. Always enable per-pixel alpha composition with an empty 0-pixel blur region `CreateRectRgn(0, 0, -1, -1)`
            // AFTER configuring margins/backdrop so WGL OpenGL (`glow`) alpha=0 pixels are 100% transparent.
            let empty_rgn = CreateRectRgn(0, 0, -1, -1);
            let bb = DWM_BLURBEHIND {
                dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                fEnable: 1,
                hRgnBlur: empty_rgn,
                fTransitionOnMaximized: 0,
            };
            let _ = DwmEnableBlurBehindWindow(hwnd, &bb);
            if !empty_rgn.is_null() {
                let _ = DeleteObject(empty_rgn as _);
            }

            if stripped_style {
                let _ = SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }

            if mode == BackgroundMode::FrostedGlass {
                let _ = DefWindowProcW(hwnd, WM_NCACTIVATE, 1, 0);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, mode, is_light);
}

/// Resizes the window to the exact physical dimensions `(width_px, height_px)` without moving its top-left corner.
pub fn resize_window_px(hwnd_raw: Option<isize>, width_px: i32, height_px: i32) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                width_px,
                height_px,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, width_px, height_px);
}

/// Reads the current physical screen position `(left, top)` of the window directly via Win32 `GetWindowRect`.
pub fn get_window_pos_px(hwnd_raw: Option<isize>) -> Option<[i32; 2]> {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if GetWindowRect(hwnd, &mut rect) != 0 && rect.left > -15000 && rect.top > -15000 {
                return Some([rect.left, rect.top]);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
    None
}

/// Checks if a given physical screen position `(x, y)` is visible on any connected monitor.
pub fn is_pos_on_any_monitor(x: i32, y: i32, width_px: i32, height_px: i32) -> bool {
    #[cfg(windows)]
    unsafe {
        let check_pt = POINT {
            x: x + (width_px / 2).max(40),
            y: y + (height_px / 2).max(40),
        };
        let hmon = MonitorFromPoint(check_pt, MONITOR_DEFAULTTONULL);
        !hmon.is_null()
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y, width_px, height_px);
        true
    }
}

/// Returns the centered physical `(x, y)` coordinates on the primary monitor.
pub fn primary_monitor_center_pos_px(width_px: i32, height_px: i32) -> [i32; 2] {
    #[cfg(windows)]
    unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        if screen_w > width_px && screen_h > height_px {
            return [
                (screen_w - width_px) / 2,
                ((screen_h - height_px) as f32 * 0.35) as i32,
            ];
        }
    }
    #[cfg(not(windows))]
    let _ = (width_px, height_px);
    [240, 180]
}

/// Moves the window to the exact physical screen coordinates `(x, y)`, or centers it on the primary monitor if off-screen.
pub fn set_window_pos_px_checked(
    hwnd_raw: Option<isize>,
    saved_pos: Option<[i32; 2]>,
    width_px: i32,
    height_px: i32,
) -> [i32; 2] {
    let target = match saved_pos {
        Some([x, y]) if is_pos_on_any_monitor(x, y, width_px, height_px) => [x, y],
        _ => primary_monitor_center_pos_px(width_px, height_px),
    };

    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                target[0],
                target[1],
                width_px,
                height_px,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;

    target
}

/// Brings the HWND to the foreground.
pub fn bring_hwnd_to_front(hwnd_raw: Option<isize>) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
}

/// Returns the current physical screen cursor position `(x, y)`.
pub fn get_cursor_pos_px() -> Option<[i32; 2]> {
    #[cfg(windows)]
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) != 0 {
            return Some([pt.x, pt.y]);
        }
    }
    None
}

/// Places a popup rectangle of `size` near `anchor` inside `[work_min, work_max]` with `margin`,
/// flipping across the anchor when it would overflow the right or bottom work-area edge.
pub fn clamp_rect_to_work_area(
    anchor: [i32; 2],
    size: [i32; 2],
    work_min: [i32; 2],
    work_max: [i32; 2],
    margin: i32,
) -> [i32; 2] {
    let place_axis = |pos: i32, len: i32, min_edge: i32, max_edge: i32| -> i32 {
        if pos + len + margin <= max_edge {
            pos.max(min_edge + margin)
        } else if pos - len >= min_edge + margin {
            pos - len
        } else {
            let lo = min_edge + margin;
            let hi = (max_edge - len - margin).max(lo);
            pos.clamp(lo, hi)
        }
    };

    [
        place_axis(anchor[0], size[0], work_min[0], work_max[0]),
        place_axis(anchor[1], size[1], work_min[1], work_max[1]),
    ]
}

/// Clamps a popup window position to the nearest monitor's work area in physical pixels.
pub fn clamp_popup_pos_to_monitor_px(
    anchor_px: [i32; 2],
    width_px: i32,
    height_px: i32,
) -> [i32; 2] {
    #[cfg(windows)]
    unsafe {
        let pt = POINT {
            x: anchor_px[0],
            y: anchor_px[1],
        };
        let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        if !hmon.is_null() {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                rcMonitor: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                rcWork: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                dwFlags: 0,
            };
            if GetMonitorInfoW(hmon, &mut info) != 0 {
                return clamp_rect_to_work_area(
                    anchor_px,
                    [width_px, height_px],
                    [info.rcWork.left, info.rcWork.top],
                    [info.rcWork.right, info.rcWork.bottom],
                    6,
                );
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (width_px, height_px);
    anchor_px
}

/// Finds a top-level window belonging to the current process by its exact title.
pub fn find_own_window_by_title(title: &str) -> Option<isize> {
    #[cfg(windows)]
    unsafe {
        let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let current_pid = GetCurrentProcessId();
        let mut prev: HWND = std::ptr::null_mut();
        loop {
            let hwnd = FindWindowExW(
                std::ptr::null_mut(),
                prev,
                std::ptr::null(),
                wide_title.as_ptr(),
            );
            if hwnd.is_null() {
                break;
            }
            let mut pid: u32 = 0;
            let _ = GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == current_pid {
                return Some(hwnd as isize);
            }
            prev = hwnd;
        }
    }
    #[cfg(not(windows))]
    let _ = title;
    None
}

#[cfg(windows)]
unsafe fn work_area_from_hmon_or_fallback(hmon: HMONITOR) -> [i32; 4] {
    unsafe {
        if !hmon.is_null() {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                rcMonitor: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                rcWork: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                dwFlags: 0,
            };
            if GetMonitorInfoW(hmon, &mut info) != 0 {
                return [
                    info.rcWork.left,
                    info.rcWork.top,
                    info.rcWork.right,
                    info.rcWork.bottom,
                ];
            }
        }

        let screen_w = GetSystemMetrics(SM_CXSCREEN).max(800);
        let screen_h = GetSystemMetrics(SM_CYSCREEN).max(600);
        [0, 0, screen_w, screen_h]
    }
}

/// Returns `[left, top, right, bottom]` of the target monitor's work area in physical pixels.
pub fn get_monitor_work_area_for_hwnd(
    hwnd_raw: Option<isize>,
    fallback_pos_px: Option<[i32; 2]>,
) -> [i32; 4] {
    #[cfg(windows)]
    unsafe {
        let hmon = if let Some(raw) = hwnd_raw {
            MonitorFromWindow(raw as HWND, MONITOR_DEFAULTTONEAREST)
        } else if let Some([x, y]) = fallback_pos_px {
            MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST)
        } else {
            MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY)
        };
        work_area_from_hmon_or_fallback(hmon)
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd_raw, fallback_pos_px);
        [0, 0, 1920, 1080]
    }
}

/// Returns `[left, top, right, bottom]` of the primary monitor's work area in physical pixels.
pub fn get_primary_monitor_work_area_px() -> [i32; 4] {
    #[cfg(windows)]
    unsafe {
        let hmon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        work_area_from_hmon_or_fallback(hmon)
    }
    #[cfg(not(windows))]
    [0, 0, 1920, 1080]
}

/// Returns `[left, top, right, bottom]` of the monitor containing the mouse cursor (falling back to
/// the foreground window's monitor, then the primary monitor) in physical pixels.
pub fn get_cursor_monitor_work_area_px() -> [i32; 4] {
    #[cfg(windows)]
    unsafe {
        let hmon = if let Some([cx, cy]) = get_cursor_pos_px() {
            MonitorFromPoint(POINT { x: cx, y: cy }, MONITOR_DEFAULTTONEAREST)
        } else {
            let fg = GetForegroundWindow();
            if !fg.is_null() {
                MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST)
            } else {
                MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY)
            }
        };
        work_area_from_hmon_or_fallback(hmon)
    }
    #[cfg(not(windows))]
    [0, 0, 1920, 1080]
}

/// Computes a centered `(pos_px, size_px)` overlay rectangle inside `work_rect` (`[left, top, right, bottom]`),
/// scaled by `width_ratio` and `height_ratio` and clamped between `min_size_px` and the available work area.
pub fn compute_overlay_rect_in_work_area(
    work_rect: [i32; 4],
    width_ratio: f32,
    height_ratio: f32,
    min_size_px: [i32; 2],
    margin_px: i32,
) -> ([i32; 2], [i32; 2]) {
    let [left, top, right, bottom] = work_rect;
    let work_w = (right - left).max(1);
    let work_h = (bottom - top).max(1);
    let max_w = (work_w - margin_px.max(0) * 2).max(200).min(work_w);
    let max_h = (work_h - margin_px.max(0) * 2).max(200).min(work_h);

    let desired_w = ((work_w as f32) * width_ratio).round() as i32;
    let desired_h = ((work_h as f32) * height_ratio).round() as i32;
    let size_w = desired_w.clamp(min_size_px[0].min(max_w), max_w);
    let size_h = desired_h.clamp(min_size_px[1].min(max_h), max_h);
    let size_px = [size_w, size_h];
    let pos_px = resolve_window_pos_in_work_area(None, size_px, work_rect);
    (pos_px, size_px)
}

/// Resolves a window position inside `work_rect` (`[left, top, right, bottom]`), restoring `saved_pos`
/// if its center lies within `work_rect` (clamped to stay inside), or centering inside `work_rect`.
pub fn resolve_window_pos_in_work_area(
    saved_pos: Option<[i32; 2]>,
    size: [i32; 2],
    work_rect: [i32; 4],
) -> [i32; 2] {
    let [left, top, right, bottom] = work_rect;
    let max_x = (right - size[0]).max(left);
    let max_y = (bottom - size[1]).max(top);

    if let Some([x, y]) = saved_pos {
        let cx = x + size[0] / 2;
        let cy = y + size[1] / 2;
        if cx >= left && cx < right && cy >= top && cy < bottom {
            return [x.clamp(left, max_x), y.clamp(top, max_y)];
        }
    }

    let x = (left + (right - left - size[0]) / 2).clamp(left, max_x);
    let y = (top + (bottom - top - size[1]) / 2).clamp(top, max_y);
    [x, y]
}

/// Disables Windows DWM open/close/restore transitions for a frameless window
/// while keeping normal taskbar and Alt-Tab visibility.
pub fn configure_frameless_window(hwnd_raw: Option<isize>) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let disable_transitions: u32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                &disable_transitions as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
}

/// Disables Windows DWM open/close/restore transitions for the frameless Settings window
/// while keeping normal taskbar and Alt-Tab visibility.
pub fn configure_settings_window(hwnd_raw: Option<isize>) {
    configure_frameless_window(hwnd_raw);
}

/// Disables Windows DWM open/close/restore transitions and configures tool-window extended style
/// so a tray-only or popup window never shows on the taskbar or Alt-Tab switcher.
pub fn configure_tray_only_window(hwnd_raw: Option<isize>) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let disable_transitions: u32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TRANSITIONS_FORCEDISABLED as u32,
                &disable_transitions as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let new_ex_style = (ex_style | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize);
            if new_ex_style != ex_style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style);
                let _ = SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
}

/// Disables Windows DWM open/close/restore transitions and configures tool-window extended style for popups.
pub fn configure_popup_window(hwnd_raw: Option<isize>) {
    configure_tray_only_window(hwnd_raw);
}

/// Positions a native topmost popup window at exact physical screen coordinates,
/// optionally showing and focusing it without invoking `SW_RESTORE` transition animations.
pub fn position_popup_window_px(
    hwnd_raw: Option<isize>,
    x_px: i32,
    y_px: i32,
    width_px: i32,
    height_px: i32,
    show: bool,
) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let flags = if show { SWP_SHOWWINDOW } else { SWP_NOACTIVATE };
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, x_px, y_px, width_px, height_px, flags);
            if show {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, x_px, y_px, width_px, height_px, show);
}

/// Hides a native popup window cleanly before viewport teardown.
pub fn hide_popup_window(hwnd_raw: Option<isize>) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
}

/// Hides a native window to the background / system tray (`SW_HIDE`).
pub fn hide_window(hwnd_raw: Option<isize>) {
    hide_popup_window(hwnd_raw);
}

/// Cloaks or uncloaks a window via `DWMWA_CLOAK` so position/size transitions can settle while hidden from view.
pub fn set_window_cloaked(hwnd_raw: Option<isize>, cloaked: bool) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let cloak: u32 = u32::from(cloaked);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CLOAK as u32,
                &cloak as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, cloaked);
}

/// Reveals a hidden window without DWM visual flash (cloaked) so its event loop can flush pending commands.
pub fn wake_cloaked_window(hwnd_raw: Option<isize>) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        set_window_cloaked(Some(raw), true);
        unsafe {
            let _ = ShowWindowAsync(raw as HWND, SW_SHOWNOACTIVATE);
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
}

/// Repositions and resizes a window at physical screen coordinates without changing Z-order or activating.
pub fn position_window_px_no_activate(hwnd_raw: Option<isize>, pos_px: [i32; 2], size_px: [i32; 2]) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        unsafe {
            let _ = SetWindowPos(
                raw as HWND,
                std::ptr::null_mut(),
                pos_px[0],
                pos_px[1],
                size_px[0],
                size_px[1],
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, pos_px, size_px);
}

#[cfg(windows)]
unsafe fn activate_hwnd_direct(hwnd: HWND) -> bool {
    unsafe {
        let _ = BringWindowToTop(hwnd);
        let ok = SetForegroundWindow(hwnd) != 0 && GetForegroundWindow() == hwnd;
        if ok {
            let _ = SetActiveWindow(hwnd);
            let _ = SetFocus(hwnd);
        }
        ok
    }
}

#[cfg(windows)]
unsafe fn send_alt_key_event(flags: u32) {
    unsafe {
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_MENU,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let _ = SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(windows)]
unsafe fn force_foreground_hwnd(hwnd: HWND) -> bool {
    unsafe {
        // Stage 1: Fast path (WM_HOTKEY, tray clicks, unlocked foreground)
        if activate_hwnd_direct(hwnd) {
            return true;
        }

        // Stage 2: AttachThreadInput path for same-integrity windows when summoned via WH_KEYBOARD_LL
        let fg = GetForegroundWindow();
        let cur_tid = GetCurrentThreadId();
        let fg_tid = if !fg.is_null() && fg != hwnd {
            GetWindowThreadProcessId(fg, std::ptr::null_mut())
        } else {
            0
        };
        let attached = fg_tid != 0 && fg_tid != cur_tid && AttachThreadInput(cur_tid, fg_tid, 1) != 0;
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = SetFocus(hwnd);
        if attached {
            let _ = AttachThreadInput(cur_tid, fg_tid, 0);
        }
        if GetForegroundWindow() == hwnd {
            return true;
        }

        // Stage 3: Paired VK_MENU fallback when Windows foreground lock blocked Stages 1 & 2
        let alt_physically_down = GetAsyncKeyState(VK_MENU as i32) < 0;
        if !alt_physically_down {
            send_alt_key_event(KEYEVENTF_EXTENDEDKEY);
        }
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = SetFocus(hwnd);
        if !alt_physically_down {
            send_alt_key_event(KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP);
        }

        GetForegroundWindow() == hwnd
    }
}

#[cfg(windows)]
unsafe fn show_and_order_hwnd(hwnd: HWND, always_on_top: bool) -> bool {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        if !always_on_top {
            let _ = SetWindowPos(
                hwnd,
                HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        force_foreground_hwnd(hwnd)
    }
}

#[cfg(windows)]
unsafe fn reveal_and_focus_hwnd(hwnd: HWND, always_on_top: bool) {
    unsafe {
        let _ = show_and_order_hwnd(hwnd, always_on_top);
        let _ = DwmFlush();
        let cloak: u32 = 0;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK as u32,
            &cloak as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Flushes DWM composition so pending backdrop/surface updates complete before uncloaking.
pub fn flush_dwm() {
    #[cfg(windows)]
    unsafe {
        let _ = DwmFlush();
    }
}

/// Attempts to bring `hwnd_raw` to the foreground and focus it.
pub fn focus_window(hwnd_raw: Option<isize>) -> bool {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        return unsafe { force_foreground_hwnd(raw as HWND) };
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
    false
}

/// Positions, shows, sets Z-order, and focuses a window while keeping `DWMWA_CLOAK` enabled
/// so the caller can render/sync DWM Acrylic before uncloaking cleanly.
pub fn prepare_window_cloaked_at_px(
    hwnd_raw: Option<isize>,
    pos_px: [i32; 2],
    size_px: [i32; 2],
    always_on_top: bool,
) -> bool {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        set_window_cloaked(Some(raw), true);
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                pos_px[0],
                pos_px[1],
                size_px[0],
                size_px[1],
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            return show_and_order_hwnd(hwnd, always_on_top);
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, pos_px, size_px, always_on_top);
    false
}

/// Shows a hidden or background window and focuses it without `SW_RESTORE` transition animations.
pub fn show_window_to_front(hwnd_raw: Option<isize>, always_on_top: bool) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        unsafe {
            reveal_and_focus_hwnd(raw as HWND, always_on_top);
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, always_on_top);
}

/// Positions, shows, and focuses a window at exact physical screen coordinates without `SW_RESTORE` transitions.
pub fn show_window_at_px(
    hwnd_raw: Option<isize>,
    pos_px: [i32; 2],
    size_px: [i32; 2],
    always_on_top: bool,
) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                pos_px[0],
                pos_px[1],
                size_px[0],
                size_px[1],
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            reveal_and_focus_hwnd(hwnd, always_on_top);
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, pos_px, size_px, always_on_top);
}

/// Synchronously sets or clears the native Win32 topmost (`WS_EX_TOPMOST`) Z-order on the HWND.
pub fn set_window_always_on_top(hwnd_raw: Option<isize>, always_on_top: bool) {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        let hwnd = raw as HWND;
        let insert_after = if always_on_top {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (hwnd_raw, always_on_top);
}

/// Returns `true` when the native window currently has `WS_VISIBLE` set.
pub fn is_window_visible(hwnd_raw: Option<isize>) -> bool {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        unsafe {
            return IsWindowVisible(raw as HWND) != 0;
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
    false
}

/// Returns the current OS foreground window handle (`HWND` as `isize`), if any.
pub fn get_foreground_hwnd() -> Option<isize> {
    #[cfg(windows)]
    unsafe {
        let fg = GetForegroundWindow();
        return (!fg.is_null()).then_some(fg as isize);
    }
    #[cfg(not(windows))]
    None
}

/// Returns `true` when the native window is currently the OS foreground window.
pub fn is_window_foreground(hwnd_raw: Option<isize>) -> bool {
    #[cfg(windows)]
    if let Some(raw) = hwnd_raw {
        unsafe {
            return GetForegroundWindow() == (raw as HWND);
        }
    }
    #[cfg(not(windows))]
    let _ = hwnd_raw;
    false
}

/// Returns the true centered physical `(x, y)` coordinates inside the primary monitor's work area.
pub fn center_on_primary_work_area_px(size_px: [i32; 2]) -> [i32; 2] {
    let work_rect = get_monitor_work_area_for_hwnd(None, None);
    resolve_window_pos_in_work_area(None, size_px, work_rect)
}

/// Returns `true` if either left or right Windows key is currently held down.
pub fn is_win_key_pressed() -> bool {
    #[cfg(windows)]
    unsafe {
        return GetAsyncKeyState(VK_LWIN as i32) < 0 || GetAsyncKeyState(VK_RWIN as i32) < 0;
    }
    #[cfg(not(windows))]
    false
}

/// Instructs Windows to reclaim unused startup, OpenGL shader compiler, and transient HTTP/TLS pages from the working set.
pub fn trim_process_working_set() {
    #[cfg(windows)]
    unsafe {
        let proc = GetCurrentProcess();
        let _ = SetProcessWorkingSetSize(proc, usize::MAX, usize::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_rect_to_work_area, compute_overlay_rect_in_work_area, resolve_window_pos_in_work_area,
    };

    #[test]
    fn test_resolve_window_pos_in_work_area() {
        let primary = [0, 0, 1920, 1040];
        let right_monitor = [1920, 0, 3840, 1040];
        let left_monitor = [-1920, 0, 0, 1040];
        let size = [468, 564];

        // Default centering on primary and secondary monitors
        assert_eq!(
            resolve_window_pos_in_work_area(None, size, primary),
            [(1920 - 468) / 2, (1040 - 564) / 2]
        );
        assert_eq!(
            resolve_window_pos_in_work_area(None, size, right_monitor),
            [1920 + (1920 - 468) / 2, (1040 - 564) / 2]
        );
        assert_eq!(
            resolve_window_pos_in_work_area(None, size, left_monitor),
            [-1920 + (1920 - 468) / 2, (1040 - 564) / 2]
        );

        // Exact restoration on the same monitor (including flush-edge parked position)
        assert_eq!(
            resolve_window_pos_in_work_area(Some([1920, 0]), size, right_monitor),
            [1920, 0]
        );
        assert_eq!(
            resolve_window_pos_in_work_area(Some([2100, 180]), size, right_monitor),
            [2100, 180]
        );

        // Clamping saved position on the same monitor when partially overflowing right/bottom edge
        assert_eq!(
            resolve_window_pos_in_work_area(Some([3600, 750]), size, right_monitor),
            [3840 - 468, 1040 - 564]
        );

        // Re-centering onto widget's current monitor when saved position belongs to another monitor
        assert_eq!(
            resolve_window_pos_in_work_area(Some([200, 180]), size, right_monitor),
            [1920 + (1920 - 468) / 2, (1040 - 564) / 2]
        );
        assert_eq!(
            resolve_window_pos_in_work_area(Some([2400, 180]), size, left_monitor),
            [-1920 + (1920 - 468) / 2, (1040 - 564) / 2]
        );

        // Small monitor where work area is smaller than window clamps safely to [left, top]
        assert_eq!(
            resolve_window_pos_in_work_area(None, [468, 564], [100, 50, 500, 450]),
            [100, 50]
        );
    }

    #[test]
    fn test_clamp_rect_to_work_area() {
        // Normal placement with room to the right and bottom
        assert_eq!(
            clamp_rect_to_work_area([100, 120], [236, 160], [0, 0], [1920, 1040], 6),
            [100, 120]
        );

        // Right-edge overflow flips left of anchor
        assert_eq!(
            clamp_rect_to_work_area([1850, 120], [236, 160], [0, 0], [1920, 1040], 6),
            [1614, 120]
        );

        // Bottom-edge overflow flips above anchor
        assert_eq!(
            clamp_rect_to_work_area([100, 980], [236, 160], [0, 0], [1920, 1040], 6),
            [100, 820]
        );

        // Multi-monitor high-offset work area
        assert_eq!(
            clamp_rect_to_work_area([5700, 1000], [236, 160], [3840, 0], [5760, 1040], 6),
            [5464, 840]
        );

        // Tight corner where neither right nor left fits cleanly clamps inside work area
        assert_eq!(
            clamp_rect_to_work_area([120, 80], [236, 160], [0, 0], [300, 200], 6),
            [58, 34]
        );
    }

    #[test]
    fn test_compute_overlay_rect_in_work_area() {
        let (pos, size) =
            compute_overlay_rect_in_work_area([0, 0, 1920, 1040], 0.72, 0.70, [680, 440], 24);
        assert_eq!(size, [1382, 728]);
        assert_eq!(pos, [(1920 - 1382) / 2, (1040 - 728) / 2]);

        // Secondary monitor with negative x offset
        let (pos_left, size_left) =
            compute_overlay_rect_in_work_area([-1920, 0, 0, 1040], 0.72, 0.70, [680, 440], 24);
        assert_eq!(size_left, [1382, 728]);
        assert_eq!(pos_left, [-1920 + (1920 - 1382) / 2, (1040 - 728) / 2]);

        // Small monitor respects margin and fits inside work area
        let (pos_small, size_small) =
            compute_overlay_rect_in_work_area([100, 50, 900, 650], 0.72, 0.70, [680, 440], 24);
        assert_eq!(size_small, [680, 440]);
        assert_eq!(pos_small, [100 + (800 - 680) / 2, 50 + (600 - 440) / 2]);
    }
}
