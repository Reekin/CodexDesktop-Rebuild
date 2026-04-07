#[cfg(target_os = "windows")]
mod win {
    use crate::model::WindowRect;
    use std::mem::size_of;
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromRect, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    };

    #[derive(Debug, Clone)]
    pub struct WindowMatch {
        pub hwnd: isize,
        pub rect: WindowRect,
        pub title: String,
        pub process_name: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ForegroundKind {
        Codex,
        Overlay,
        Other,
    }

    pub fn classify_foreground_window(overlay_title: &str) -> ForegroundKind {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() {
            return ForegroundKind::Other;
        }

        let pid = process_id_for_hwnd(hwnd);
        if pid == unsafe { GetCurrentProcessId() } {
            let title = window_title(hwnd);
            if title.contains(overlay_title) {
                return ForegroundKind::Overlay;
            }
        }

        if is_codex_window(hwnd) {
            ForegroundKind::Codex
        } else {
            ForegroundKind::Other
        }
    }

    pub fn probe_foreground_codex_window() -> Option<WindowMatch> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.0.is_null() || !is_codex_window(hwnd) {
            return None;
        }
        window_match(hwnd)
    }

    pub fn overlay_work_area(target: WindowRect) -> WindowRect {
        let rect = RECT {
            left: target.left,
            top: target.top,
            right: target.right,
            bottom: target.bottom,
        };
        let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
        monitor_work_area(monitor).unwrap_or(target)
    }

    fn is_codex_window(hwnd: HWND) -> bool {
        if hwnd.0.is_null() {
            return false;
        }
        let visible = unsafe { IsWindowVisible(hwnd).as_bool() };
        let iconic = unsafe { IsIconic(hwnd).as_bool() };
        if !visible || iconic {
            return false;
        }
        let process_name = process_name_for_hwnd(hwnd);
        if process_name.eq_ignore_ascii_case("Codex.exe") {
            return true;
        }
        if process_name.eq_ignore_ascii_case("codex-monitor.exe") {
            return false;
        }
        let title = window_title(hwnd);
        title == "Codex" || (title.starts_with("Codex ") && !title.contains("Monitor"))
    }

    fn window_match(hwnd: HWND) -> Option<WindowMatch> {
        let rect = window_rect(hwnd)?;
        Some(WindowMatch {
            hwnd: hwnd.0 as isize,
            rect,
            title: window_title(hwnd),
            process_name: process_name_for_hwnd(hwnd),
        })
    }

    fn window_rect(hwnd: HWND) -> Option<WindowRect> {
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            return None;
        }
        Some(WindowRect::new(
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
        ))
    }

    fn window_title(hwnd: HWND) -> String {
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length <= 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let read = unsafe { GetWindowTextW(hwnd, &mut buffer) };
        String::from_utf16_lossy(&buffer[..read as usize])
    }

    fn process_id_for_hwnd(hwnd: HWND) -> u32 {
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        pid
    }

    fn process_name_for_hwnd(hwnd: HWND) -> String {
        process_name_for_pid(process_id_for_hwnd(hwnd))
    }

    fn process_name_for_pid(pid: u32) -> String {
        if pid == 0 {
            return String::new();
        }

        let snapshot = match unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
            Ok(value) => value,
            Err(_) => return String::new(),
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if unsafe { Process32FirstW(snapshot, &mut entry) }.is_err() {
            return String::new();
        }

        loop {
            if entry.th32ProcessID == pid {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                return String::from_utf16_lossy(&entry.szExeFile[..end]);
            }
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }

        String::new()
    }

    fn monitor_work_area(monitor: HMONITOR) -> Option<WindowRect> {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
            Some(WindowRect::new(
                info.rcWork.left,
                info.rcWork.top,
                info.rcWork.right,
                info.rcWork.bottom,
            ))
        } else {
            None
        }
    }
}

#[cfg(target_os = "windows")]
pub use win::*;
