use serde_json::json;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows_sys::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowLongW, IsIconic,
    IsWindowVisible, IsZoomed, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

struct EnumData {
    self_hwnd: HWND,
    win_x: i32,
    win_y: i32,
    win_w: i32,
    win_h: i32,
    sf: f64,
    wins: Vec<serde_json::Value>,
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let data = &mut *(lparam as *mut EnumData);
    if data.wins.len() >= 40 {
        return 0; // max 40 windows
    }
    if hwnd == data.self_hwnd || IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
        return 1;
    }
    let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if (ex & WS_EX_TOOLWINDOW) != 0 || (ex & WS_EX_NOACTIVATE) != 0 {
        return 1;
    }
    let mut cloaked: i32 = 0;
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED as u32,
        &mut cloaked as *mut _ as *mut _,
        size_of::<i32>() as u32,
    ) == 0
        && cloaked != 0
    {
        return 1;
    }

    let mut class_buf = [0u16; 64];
    let class_len = GetClassNameW(hwnd, class_buf.as_mut_ptr(), 64);
    if class_len > 0 {
        let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);
        if class_name == "Progman"
            || class_name == "WorkerW"
            || class_name == "Shell_TrayWnd"
            || class_name == "Shell_SecondaryTrayWnd"
        {
            return 1;
        }
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS as u32,
        &mut rect as *mut _ as *mut _,
        size_of::<RECT>() as u32,
    ) != 0
    {
        return 1;
    }

    let pw = rect.right - rect.left;
    let ph = rect.bottom - rect.top;
    if pw < 200 || ph < 120 {
        return 1;
    }

    let x = ((rect.left - data.win_x) as f64 / data.sf).round() as i32;
    let y = ((rect.top - data.win_y) as f64 / data.sf).round() as i32;
    let w = (pw as f64 / data.sf).round() as i32;
    let h = (ph as f64 / data.sf).round() as i32;
    let max = IsZoomed(hwnd) != 0;

    let overlay_w = (data.win_w as f64 / data.sf).round() as i32;
    let overlay_h = (data.win_h as f64 / data.sf).round() as i32;

    if x < overlay_w && x + w > 0 && y < overlay_h && y + h > 0 {
        data.wins.push(json!({
            "id": hwnd as i64,
            "x": x,
            "y": y,
            "w": w,
            "h": h,
            "max": max
        }));
    }

    1
}

pub fn snapshot_windows(app: &AppHandle) -> serde_json::Value {
    let (self_hwnd, win_x, win_y, win_w, win_h, sf) = if let Some(w) = app.get_webview_window("main") {
        let hwnd = w.hwnd().map(|h| h.0 as HWND).unwrap_or(std::ptr::null_mut());
        let p = w.inner_position().unwrap_or(tauri::PhysicalPosition { x: 0, y: 0 });
        let s = w.inner_size().unwrap_or(tauri::PhysicalSize {
            width: 1920,
            height: 1080,
        });
        let sf = w.scale_factor().unwrap_or(1.0);
        (hwnd, p.x, p.y, s.width as i32, s.height as i32, sf)
    } else {
        (std::ptr::null_mut(), 0, 0, 1920, 1080, 1.0)
    };

    let fg = unsafe { GetForegroundWindow() };
    let mut taskbar = false;
    if !fg.is_null() {
        let mut class_buf = [0u16; 64];
        let class_len = unsafe { GetClassNameW(fg, class_buf.as_mut_ptr(), 64) };
        if class_len > 0 {
            let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);
            taskbar = class_name == "Shell_TrayWnd" || class_name == "Shell_SecondaryTrayWnd";
        }
    }

    let mut data = EnumData {
        self_hwnd,
        win_x,
        win_y,
        win_w,
        win_h,
        sf,
        wins: Vec::new(),
    };

    unsafe {
        EnumWindows(Some(enum_windows_callback), &mut data as *mut _ as LPARAM);
    }

    json!({
        "fg": fg as i64,
        "taskbar": taskbar,
        "ai": [],
        "game": null,
        "usb": [],
        "plug": null,
        "wins": data.wins
    })
}

#[allow(dead_code)]
pub struct WindowTracker {
    running: Arc<AtomicBool>,
}

impl WindowTracker {
    pub fn start(app: AppHandle) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        thread::spawn(move || {
            unsafe {
                let _ = timeBeginPeriod(1);
            }

            let mut last_pt = POINT { x: -9999, y: -9999 };
            let mut last_lbutton = false;
            let mut last_windows_snap = String::new();
            let mut fast_until = std::time::Instant::now();
            let fast_duration = Duration::from_millis(350);

            let mut last_breach_check = std::time::Instant::now();
            let mut breach_was_active = false;
            let mut last_solution_codes: serde_json::Value = serde_json::Value::Null;

            while r.load(Ordering::Relaxed) {
                // 1. Cursor position tracking in exact CSS pixels
                let mut current_pt = POINT { x: 0, y: 0 };
                unsafe {
                    GetCursorPos(&mut current_pt);
                }

                let lbutton = unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0 };
                let rbutton = unsafe { (GetAsyncKeyState(VK_RBUTTON as i32) as u16 & 0x8000) != 0 };

                let (win_x, win_y, win_w, win_h, sf, monitors) = if let Some(w) = app.get_webview_window("main") {
                    let p = w.inner_position().unwrap_or(tauri::PhysicalPosition { x: 0, y: 0 });
                    let s = w.inner_size().unwrap_or(tauri::PhysicalSize {
                        width: 1920,
                        height: 1080,
                    });
                    let sf = w.scale_factor().unwrap_or(1.0);
                    let mons = w.available_monitors().unwrap_or_default();
                    (p.x, p.y, s.width as i32, s.height as i32, sf, mons)
                } else {
                    (0, 0, 1920, 1080, 1.0, Vec::new())
                };

                let display_idx = crate::commands::find_monitor_index(&monitors, current_pt);

                let (mon_x, mon_y, mon_sf) = if let Some(m) = monitors.get(display_idx) {
                    let p = m.position();
                    (p.x, p.y, m.scale_factor())
                } else {
                    (win_x, win_y, sf)
                };

                let local_x = ((current_pt.x - mon_x) as f64 / mon_sf).round() as i32;
                let local_y = ((current_pt.y - mon_y) as f64 / mon_sf).round() as i32;

                let inside = current_pt.x >= win_x
                    && current_pt.x < win_x + win_w
                    && current_pt.y >= win_y
                    && current_pt.y < win_y + win_h;

                let rel_x = ((current_pt.x - win_x) as f64 / sf).round() as i32;
                let rel_y = ((current_pt.y - win_y) as f64 / sf).round() as i32;

                if current_pt.x != last_pt.x || current_pt.y != last_pt.y {
                    last_pt = current_pt;
                    let _ = app.emit(
                        "cursor:move",
                        json!({
                            "x": local_x,
                            "y": local_y,
                            "local_x": local_x,
                            "local_y": local_y,
                            "win_rel_x": rel_x,
                            "win_rel_y": rel_y,
                            "screen_x": current_pt.x,
                            "screen_y": current_pt.y,
                            "inside": inside,
                            "display": display_idx,
                            "lbutton": lbutton,
                            "rbutton": rbutton
                        }),
                    );
                }

                // Global mouse release fallback
                if last_lbutton && !lbutton {
                    let _ = app.emit(
                        "cursor:up",
                        json!({
                            "x": local_x,
                            "y": local_y,
                            "local_x": local_x,
                            "local_y": local_y,
                            "screen_x": current_pt.x,
                            "screen_y": current_pt.y,
                            "button": 0
                        }),
                    );
                } else if !last_lbutton && lbutton {
                    let _ = app.emit(
                        "cursor:down",
                        json!({
                            "x": local_x,
                            "y": local_y,
                            "local_x": local_x,
                            "local_y": local_y,
                            "screen_x": current_pt.x,
                            "screen_y": current_pt.y,
                            "button": 0
                        }),
                    );
                }
                last_lbutton = lbutton;

                // 2. High-precision dynamic window snapshotting:
                // When moving/dragging or recently changed, poll at ~144 Hz for seamless tracking
                let in_motion = lbutton || rbutton || (std::time::Instant::now() < fast_until);

                let snap = snapshot_windows(&app);
                let snap_str = snap.to_string();
                if snap_str != last_windows_snap {
                    last_windows_snap = snap_str;
                    fast_until = std::time::Instant::now() + fast_duration;
                    let _ = app.emit("windows:update", snap);
                }

                // 3. Cyberpunk 2077 Breach Protocol background detection (~600ms)
                let now_inst = std::time::Instant::now();
                if now_inst.duration_since(last_breach_check) >= Duration::from_millis(600) {
                    last_breach_check = now_inst;
                    if let Some(solution) = crate::cyberpunk::scan_and_solve_breach(win_x, win_y, win_w, win_h, sf) {
                        let codes = solution["codes"].clone();
                        if !breach_was_active || codes != last_solution_codes {
                            last_solution_codes = codes;
                            breach_was_active = true;
                            let _ = app.emit("cyberpunk:breach_detected", solution);
                        }
                    } else if breach_was_active {
                        breach_was_active = false;
                        last_solution_codes = serde_json::Value::Null;
                        let _ = app.emit("cyberpunk:breach_cleared", json!({ "cleared": true }));
                    }
                }

                if in_motion {
                    // ~144 Hz polling for high refresh-rate monitors
                    thread::sleep(Duration::from_millis(7));
                } else {
                    // Resting state
                    thread::sleep(Duration::from_millis(80));
                }
            }

            unsafe {
                let _ = timeEndPeriod(1);
            }
        });

        Self { running }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
