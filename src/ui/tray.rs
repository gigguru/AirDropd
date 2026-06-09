//! Windows system tray integration (Win32 API).

#[cfg(windows)]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, OnceLock};
    use tracing::{info, warn};
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
        NOTIFYICONDATAW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyWindow,
        DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassW, TrackPopupMenu,
        TranslateMessage, HMENU, IMAGE_ICON, LR_DEFAULTSIZE, MF_STRING, MSG, TPM_BOTTOMALIGN,
        TPM_LEFTALIGN, TPM_RETURNCMD, WM_APP, WM_COMMAND, WM_DESTROY, WM_LBUTTONUP,
        WM_RBUTTONUP, WNDCLASSW, WS_OVERLAPPED, LoadIconW, LoadImageW, IDI_APPLICATION,
    };

    static TRAY_READY: AtomicBool = AtomicBool::new(false);
    static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
    static SHOW_REQUESTED: AtomicBool = AtomicBool::new(false);
    static TOOLTIP: OnceLock<Arc<str>> = OnceLock::new();

    const TRAY_ICON_ID: u32 = 1;
    const WM_TRAYICON: u32 = WM_APP + 1;
    const CMD_SHOW: usize = 1001;
    const CMD_QUIT: usize = 1002;

    fn load_tray_icon(instance: windows::Win32::Foundation::HINSTANCE) -> windows::Win32::UI::WindowsAndMessaging::HICON {
        unsafe {
            // Icon embedded by build.rs / winres (resource ID 1)
            if let Ok(icon) = LoadImageW(
                Some(instance),
                windows::core::PCWSTR(1usize as *const u16),
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE,
            ) {
                return icon;
            }
            LoadIconW(None, IDI_APPLICATION).unwrap_or_default()
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAYICON => match lparam.0 as u32 {
                WM_LBUTTONUP => {
                    SHOW_REQUESTED.store(true, Ordering::SeqCst);
                    LRESULT(0)
                }
                WM_RBUTTONUP => {
                    let menu = CreatePopupMenu().unwrap_or_default();
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING,
                        CMD_SHOW,
                        w!("Show AirDropd"),
                    );
                    let _ = AppendMenuW(menu, MF_STRING, CMD_QUIT, w!("Quit"));
                    let _ = TrackPopupMenu(
                        menu,
                        TPM_BOTTOMALIGN | TPM_LEFTALIGN | TPM_RETURNCMD,
                        0,
                        0,
                        0,
                        hwnd,
                        None,
                    );
                    LRESULT(0)
                }
                _ => LRESULT(0),
            },
            WM_COMMAND => {
                match wparam.0 {
                    CMD_SHOW => SHOW_REQUESTED.store(true, Ordering::SeqCst),
                    CMD_QUIT => QUIT_REQUESTED.store(true, Ordering::SeqCst),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    pub fn init_tray(tooltip: &str) -> anyhow::Result<()> {
        if TRAY_READY.load(Ordering::SeqCst) {
            return Ok(());
        }

        let _ = TOOLTIP.set(Arc::from(tooltip));
        let tip = TOOLTIP.get().unwrap().clone();

        std::thread::spawn(move || {
            unsafe {
                let instance = GetModuleHandleW(None).unwrap_or_default();
                let class_name = w!("AirDropdTrayClass");

                let wc = WNDCLASSW {
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: instance.into(),
                    lpszClassName: class_name,
                    ..Default::default()
                };

                if RegisterClassW(&wc) == 0 {
                    warn!("Failed to register tray window class");
                    return;
                }

                let hwnd = CreateWindowExW(
                    Default::default(),
                    class_name,
                    w!("AirDropd"),
                    WS_OVERLAPPED,
                    0,
                    0,
                    0,
                    0,
                    None,
                    HMENU::default(),
                    instance,
                    None,
                );

                let Ok(hwnd) = hwnd else {
                    warn!("Failed to create tray message window");
                    return;
                };

                let mut tip_buf: [u16; 128] = [0; 128];
                for (i, c) in tip.encode_utf16().take(127).enumerate() {
                    tip_buf[i] = c;
                }

                let icon = load_tray_icon(instance.into());

                let mut nid = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: TRAY_ICON_ID,
                    uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                    uCallbackMessage: WM_TRAYICON,
                    hIcon: icon,
                    szTip: tip_buf,
                    ..Default::default()
                };

                if Shell_NotifyIconW(NIM_ADD, &mut nid).is_ok() {
                    TRAY_READY.store(true, Ordering::SeqCst);
                    info!("System tray icon initialized");
                } else {
                    warn!("Shell_NotifyIconW(NIM_ADD) failed");
                }

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).into() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                nid.uFlags = NIF_MESSAGE;
                let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
                let _ = DestroyWindow(hwnd);
                TRAY_READY.store(false, Ordering::SeqCst);
            }
        });

        for _ in 0..20 {
            if TRAY_READY.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        Ok(())
    }

    pub fn poll_tray_action() -> Option<&'static str> {
        if QUIT_REQUESTED.swap(false, Ordering::SeqCst) {
            return Some("quit");
        }
        if SHOW_REQUESTED.swap(false, Ordering::SeqCst) {
            return Some("show");
        }
        None
    }

    pub fn set_tooltip(tooltip: &str) {
        let _ = TOOLTIP.set(Arc::from(tooltip));
        // Tooltip updates on next init; live modify would need stored HWND.
    }
}

#[cfg(windows)]
pub use imp::{init_tray, poll_tray_action, set_tooltip};

#[cfg(not(windows))]
pub fn init_tray(_tooltip: &str) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn poll_tray_action() -> Option<&'static str> {
    None
}

#[cfg(not(windows))]
pub fn set_tooltip(_tooltip: &str) {}
