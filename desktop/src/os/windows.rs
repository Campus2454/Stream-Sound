//! Windows: notification-area icon, Run-key autostart (the same list Task
//! Manager's Startup apps shows), and keeping the window off the taskbar.

use super::Waker;
use raw_window_handle::RawWindowHandle;
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicIsize, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_BINARY, REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_TRAY: u32 = WM_APP + 1;
const MENU_SHOW: usize = 1;
const MENU_QUIT: usize = 2;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// Where Task Manager keeps its on/off switch for each Run entry.
const APPROVED_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const RUN_NAME: &str = "Stream Sound";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn hwnd(w: RawWindowHandle) -> Option<isize> {
    match w {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

fn h(v: isize) -> HWND {
    v as HWND
}

pub fn is_visible(v: isize) -> bool {
    unsafe { IsWindowVisible(h(v)) != 0 }
}

/// Show and raise the window. Safe from any thread.
pub fn show_native(v: isize) {
    unsafe {
        if IsIconic(h(v)) != 0 {
            ShowWindow(h(v), SW_RESTORE);
        } else {
            ShowWindow(h(v), SW_SHOW);
        }
        SetForegroundWindow(h(v));
    }
}

/// Hide the window right away, from any thread.
pub fn hide_native(v: isize) {
    unsafe {
        ShowWindow(h(v), SW_HIDE);
    }
}

/// Start a program as a double-click would, so Windows asks for permission
/// when the program needs to run as administrator (the installer). Fails
/// with error 1223 (`ERROR_CANCELLED`) if the user says no.
pub fn shell_open(file: &std::path::Path, params: &str) -> std::io::Result<()> {
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW};
    let verb = wide("open");
    let file = wide(&file.to_string_lossy());
    let params = wide(params);
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_SHOWNORMAL;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Called by a second copy before it wakes the first, so the first may
/// bring its window to the front (Windows only lets the app the user just
/// started take focus).
pub fn let_other_copy_come_forward() {
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
    }
}

// ---- tray -------------------------------------------------------------------

struct State {
    window: HWND,
    icon: HICON,
    waker: Option<Waker>,
    taskbar_created: u32,
}

/// The tray's window while its icon is showing, else 0. Shared so the icon
/// can be removed from any thread (an installer asking the app to close).
static SHOWN: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    // The tray's hidden window belongs to the UI thread, and so does this.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub struct Tray;

impl Tray {
    pub fn new() -> Option<Tray> {
        unsafe {
            let inst = GetModuleHandleW(null());
            let class = wide("StreamSoundTray");
            let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: inst, lpszClassName: class.as_ptr(), ..std::mem::zeroed() };
            RegisterClassW(&wc);
            // A plain hidden window rather than a message-only one, so it
            // hears "TaskbarCreated" when Explorer restarts.
            let window = CreateWindowExW(0, class.as_ptr(), class.as_ptr(), 0, 0, 0, 0, 0, null_mut(), null_mut(), inst, null());
            if window.is_null() {
                return None;
            }
            let state = State {
                window,
                icon: load_icon(),
                waker: None,
                taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            };
            STATE.with(|s| *s.borrow_mut() = Some(state));
            add_icon();
            Some(Tray)
        }
    }

    pub fn connect(&self, waker: Waker) {
        STATE.with(|s| {
            if let Some(st) = s.borrow_mut().as_mut() {
                st.waker = Some(waker);
            }
        });
    }

    pub fn available(&self) -> bool {
        SHOWN.load(Ordering::Relaxed) != 0
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        remove_tray_icon();
        // Take the state out first: destroying the window calls `wndproc`,
        // which reads the state too.
        let Some(st) = STATE.with(|s| s.borrow_mut().take()) else { return };
        unsafe {
            DestroyWindow(st.window);
            if !st.icon.is_null() {
                DestroyIcon(st.icon);
            }
        }
    }
}

fn icon_data(window: HWND) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = window;
    nid.uID = 1;
    nid
}

fn add_icon() {
    let Some((window, icon)) = STATE.with(|s| s.try_borrow().ok()?.as_ref().map(|st| (st.window, st.icon))) else {
        return;
    };
    let mut nid = icon_data(window);
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon;
    for (d, c) in nid.szTip.iter_mut().zip("Stream Sound".encode_utf16()) {
        *d = c;
    }
    if unsafe { Shell_NotifyIconW(NIM_ADD, &nid) } != 0 {
        SHOWN.store(window as isize, Ordering::Relaxed);
    }
}

/// Take the icon out of the notification area (it would otherwise linger
/// until the mouse passes over it). Safe from any thread.
pub fn remove_tray_icon() {
    let window = SHOWN.swap(0, Ordering::Relaxed);
    if window != 0 {
        let nid = icon_data(window as HWND);
        unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
    }
}

/// The icon built into the .exe (build.rs), or the PNG if it is missing.
fn load_icon() -> HICON {
    unsafe {
        let inst = GetModuleHandleW(null());
        let (w, h) = (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
        let icon = LoadImageW(inst, 1 as _, IMAGE_ICON, w, h, LR_DEFAULTCOLOR);
        if !icon.is_null() {
            return icon;
        }
        let img = crate::gui::app_icon();
        // Windows wants BGRA with a 1-bit mask; the alpha channel does the work.
        let bgra: Vec<u8> = img.rgba.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect();
        let mask = vec![0u8; (img.width as usize).div_ceil(8) * img.height as usize];
        CreateIcon(inst, img.width as i32, img.height as i32, 1, 32, mask.as_ptr(), bgra.as_ptr())
    }
}

// `wndproc` must never panic: a panic there ends the process on the spot,
// leaving the window frozen on screen while Windows reports the crash. So
// it only ever tries to read the state.

fn waker() -> Option<Waker> {
    STATE.with(|s| s.try_borrow().ok()?.as_ref()?.waker.clone())
}

fn menu(window: HWND) {
    unsafe {
        let m = CreatePopupMenu();
        AppendMenuW(m, MF_STRING, MENU_SHOW, wide("เปิด Stream Sound").as_ptr());
        AppendMenuW(m, MF_SEPARATOR, 0, null());
        AppendMenuW(m, MF_STRING, MENU_QUIT, wide("ออก").as_ptr());
        SetMenuDefaultItem(m, MENU_SHOW as u32, 0);
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        // Without this the menu stays open when clicking elsewhere.
        SetForegroundWindow(window);
        let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON, pt.x, pt.y, 0, window, null());
        PostMessageW(window, WM_NULL, 0, 0);
        DestroyMenu(m);
        match cmd as usize {
            MENU_SHOW => {
                if let Some(w) = waker() {
                    w.show();
                }
            }
            MENU_QUIT => {
                if let Some(w) = waker() {
                    w.quit();
                }
            }
            _ => {}
        }
    }
}

unsafe extern "system" fn wndproc(window: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_TRAY {
        match lp as u32 {
            WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                if let Some(w) = waker() {
                    w.show();
                }
            }
            WM_RBUTTONUP | WM_CONTEXTMENU => menu(window),
            _ => {}
        }
        return 0;
    }
    let restarted = STATE.with(|s| {
        s.try_borrow().is_ok_and(|s| s.as_ref().is_some_and(|s| s.taskbar_created != 0 && msg == s.taskbar_created))
    });
    if restarted {
        add_icon();
        return 0;
    }
    unsafe { DefWindowProcW(window, msg, wp, lp) }
}

// ---- window placement ---------------------------------------------------------

/// Keep the whole window inside the monitor's work area (the screen minus
/// the taskbar), shrinking it if it is taller or wider than that.
pub fn fit_to_work_area(v: isize) -> bool {
    unsafe {
        let window = h(v);
        if IsWindowVisible(window) == 0 || IsIconic(window) != 0 || IsZoomed(window) != 0 {
            return false;
        }
        let mon = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return false;
        }
        let work = mi.rcWork;
        let mut outer: RECT = std::mem::zeroed();
        if GetWindowRect(window, &mut outer) == 0 {
            return false;
        }
        // The visible frame; the outer rect includes invisible resize borders.
        let mut seen = outer;
        DwmGetWindowAttribute(
            window,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            &mut seen as *mut RECT as *mut _,
            std::mem::size_of::<RECT>() as u32,
        );
        let (l, t) = (seen.left - outer.left, seen.top - outer.top);
        let (r, b) = (outer.right - seen.right, outer.bottom - seen.bottom);
        let w = (seen.right - seen.left).min(work.right - work.left);
        let hgt = (seen.bottom - seen.top).min(work.bottom - work.top);
        let x = seen.left.clamp(work.left, work.right - w);
        let y = seen.top.clamp(work.top, work.bottom - hgt);
        if (x, y, w, hgt) == (seen.left, seen.top, seen.right - seen.left, seen.bottom - seen.top) {
            return true;
        }
        SetWindowPos(window, null_mut(), x - l, y - t, w + l + r, hgt + t + b, SWP_NOZORDER | SWP_NOACTIVATE);
        true
    }
}

// ---- start with Windows -----------------------------------------------------------

struct Key(HKEY);

impl Key {
    fn open(path: &str, create: bool) -> Option<Key> {
        let mut k: HKEY = null_mut();
        let p = wide(path);
        let ok = unsafe {
            if create {
                RegCreateKeyExW(HKEY_CURRENT_USER, p.as_ptr(), 0, null(), REG_OPTION_NON_VOLATILE, KEY_READ | KEY_WRITE, null(), &mut k, null_mut())
            } else {
                RegOpenKeyExW(HKEY_CURRENT_USER, p.as_ptr(), 0, KEY_READ | KEY_WRITE, &mut k)
            }
        };
        (ok == 0).then_some(Key(k))
    }

    fn get(&self, name: &str) -> Option<Vec<u8>> {
        let n = wide(name);
        let mut len = 0u32;
        unsafe {
            if RegQueryValueExW(self.0, n.as_ptr(), null(), null_mut(), null_mut(), &mut len) != 0 {
                return None;
            }
            let mut buf = vec![0u8; len as usize];
            if RegQueryValueExW(self.0, n.as_ptr(), null(), null_mut(), buf.as_mut_ptr(), &mut len) != 0 {
                return None;
            }
            buf.truncate(len as usize);
            Some(buf)
        }
    }

    fn set(&self, name: &str, kind: u32, data: &[u8]) -> bool {
        let n = wide(name);
        unsafe { RegSetValueExW(self.0, n.as_ptr(), 0, kind, data.as_ptr(), data.len() as u32) == 0 }
    }

    fn delete(&self, name: &str) {
        let n = wide(name);
        unsafe { RegDeleteValueW(self.0, n.as_ptr()) };
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { RegCloseKey(self.0) };
    }
}

fn run_command() -> Option<String> {
    let exe = super::exe_path()?;
    Some(format!("\"{}\" {}", exe.display(), super::MINIMIZED_FLAG))
}

fn utf16_bytes(s: &str) -> Vec<u8> {
    wide(s).iter().flat_map(|c| c.to_le_bytes()).collect()
}

pub fn autostart_enabled() -> bool {
    let Some(run) = Key::open(RUN_KEY, false) else { return false };
    if run.get(RUN_NAME).is_none() {
        return false;
    }
    // Task Manager marks a disabled entry with an odd first byte (3).
    let disabled = Key::open(APPROVED_KEY, false).and_then(|k| k.get(RUN_NAME)).is_some_and(|d| d.first().is_some_and(|b| b & 1 == 1));
    !disabled
}

pub fn set_autostart(on: bool) -> anyhow::Result<()> {
    let run = Key::open(RUN_KEY, true).ok_or_else(|| anyhow::anyhow!("can't open the startup list"))?;
    let approved = Key::open(APPROVED_KEY, on);
    if on {
        let cmd = run_command().ok_or_else(|| anyhow::anyhow!("can't find the app's file"))?;
        if !run.set(RUN_NAME, REG_SZ, &utf16_bytes(&cmd)) {
            anyhow::bail!("can't write the startup list");
        }
        if let Some(a) = approved {
            // 2 = enabled, as Task Manager writes it.
            let mut v = [0u8; 12];
            v[0] = 2;
            a.set(RUN_NAME, REG_BINARY, &v);
        }
    } else {
        run.delete(RUN_NAME);
        if let Some(a) = approved {
            a.delete(RUN_NAME);
        }
    }
    Ok(())
}

pub fn refresh_integration() {
    // Keep the entry pointing at this file if the app was moved.
    if !autostart_enabled() {
        return;
    }
    let (Some(run), Some(cmd)) = (Key::open(RUN_KEY, false), run_command()) else { return };
    if run.get(RUN_NAME) != Some(utf16_bytes(&cmd)) {
        run.set(RUN_NAME, REG_SZ, &utf16_bytes(&cmd));
    }
}
