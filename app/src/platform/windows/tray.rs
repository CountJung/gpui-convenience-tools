//! 시스템 트레이 아이콘.
//!
//! 숨겨진 메시지 전용 윈도우를 만들어 트레이 아이콘의 클릭·메뉴 이벤트를 처리한다.
//! 메시지 루프는 전용 스레드에서 돌며, 창 복원과 종료 요청을 메인 윈도우로 전달한다.

use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;

use super::wide_null;

use windows_sys::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    },
    UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
        DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW, IDI_APPLICATION,
        LoadIconW, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW, SW_HIDE,
        SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE, WM_DESTROY, WM_LBUTTONDBLCLK,
        WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
    },
};

const TRAY_MESSAGE_ID: u32 = WM_APP + 1;
const TRAY_ICON_UID: u32 = 1;
const MAIN_WINDOW_TITLE: &str = "gpui-convenience-tools";
const TRAY_MENU_OPEN: usize = 1001;
const TRAY_MENU_TOGGLE: usize = 1002;
const TRAY_MENU_EXIT: usize = 1003;

static TRAY_INITIALIZED: OnceLock<()> = OnceLock::new();
static TRAY_TOGGLE_HANDLER: OnceLock<std::sync::Arc<dyn Fn(bool) + Send + Sync>> =
    OnceLock::new();
static TRAY_SERVICE_ACTIVE: AtomicBool = AtomicBool::new(true);

pub fn set_tray_toggle_handler(handler: std::sync::Arc<dyn Fn(bool) + Send + Sync>) {
    let _ = TRAY_TOGGLE_HANDLER.set(handler);
}

pub fn set_tray_service_active(active: bool) {
    TRAY_SERVICE_ACTIVE.store(active, Ordering::Relaxed);
}


fn with_tip(mut data: [u16; 128], tip: &str) -> [u16; 128] {
    let encoded = wide_null(tip);
    let max = data.len().saturating_sub(1);
    let len = encoded.len().min(max);
    data[..len].copy_from_slice(&encoded[..len]);
    data
}

fn try_find_main_window() -> Option<HWND> {
    let title = wide_null(MAIN_WINDOW_TITLE);
    // SAFETY: String pointer is a valid null-terminated UTF-16 title.
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd == 0 {
        None
    } else {
        Some(hwnd)
    }
}

fn restore_main_window() {
    if let Some(main_hwnd) = try_find_main_window() {
        // SAFETY: main_hwnd is validated by FindWindowW and used with standard show/activate APIs.
        unsafe {
            ShowWindow(main_hwnd, SW_SHOW);
            ShowWindow(main_hwnd, SW_RESTORE);
            SetForegroundWindow(main_hwnd);
        }
    }
}

fn request_app_exit() {
    if let Some(main_hwnd) = try_find_main_window() {
        // SAFETY: Posting WM_CLOSE to the main window requests a normal app shutdown.
        unsafe {
            let _ = PostMessageW(main_hwnd, WM_CLOSE, 0, 0);
        }
    } else {
        std::process::exit(0);
    }
}

fn show_tray_menu(hwnd: HWND) {
    // SAFETY: Menu and cursor APIs are called with valid parameters and cleaned up on all paths.
    unsafe {
        let menu = CreatePopupMenu();
        if menu == 0 {
            return;
        }

        let open = wide_null("Open");
        let toggle = if TRAY_SERVICE_ACTIVE.load(Ordering::Relaxed) {
            wide_null("Disable Blocking")
        } else {
            wide_null("Enable Blocking")
        };
        let exit = wide_null("Exit");

        let _ = AppendMenuW(menu, MF_STRING, TRAY_MENU_OPEN, open.as_ptr());
        let _ = AppendMenuW(menu, MF_STRING, TRAY_MENU_TOGGLE, toggle.as_ptr());
        let _ = AppendMenuW(menu, MF_STRING, TRAY_MENU_EXIT, exit.as_ptr());

        let mut pt = std::mem::zeroed::<POINT>();
        if GetCursorPos(&mut pt) == 0 {
            let _ = DestroyMenu(menu);
            return;
        }

        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );

        let _ = DestroyMenu(menu);

        match cmd as usize {
            TRAY_MENU_OPEN => restore_main_window(),
            TRAY_MENU_TOGGLE => {
                let next = !TRAY_SERVICE_ACTIVE.load(Ordering::Relaxed);
                TRAY_SERVICE_ACTIVE.store(next, Ordering::Relaxed);
                if let Some(handler) = TRAY_TOGGLE_HANDLER.get() {
                    handler(next);
                }
            }
            TRAY_MENU_EXIT => request_app_exit(),
            _ => {}
        }
    }
}

extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    _wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == TRAY_MESSAGE_ID {
        let mouse_msg = lparam as u32;
        if mouse_msg == WM_LBUTTONUP || mouse_msg == WM_LBUTTONDBLCLK {
            restore_main_window();
            return 0;
        }

        if mouse_msg == WM_RBUTTONUP {
            show_tray_menu(hwnd);
            return 0;
        }
    }

    if msg == WM_DESTROY {
        // SAFETY: Structure is zero-initialized and required fields are populated before API call.
        unsafe {
            let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_ICON_UID;
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
            PostQuitMessage(0);
        }
        return 0;
    }

    // SAFETY: Default window procedure is called for unhandled messages.
    unsafe { DefWindowProcW(hwnd, msg, _wparam, lparam) }
}

pub fn init_tray_icon() -> Result<()> {
    if TRAY_INITIALIZED.get().is_some() {
        return Ok(());
    }

    thread::spawn(|| {
        // SAFETY: Null module name requests current process module handle.
        let hinstance: HINSTANCE = unsafe { GetModuleHandleW(std::ptr::null()) };
        if hinstance == 0 {
            return;
        }

        let class_name = wide_null("gpui-convenience-toolsTrayClass");
        let mut wc = unsafe { std::mem::zeroed::<WNDCLASSW>() };
        wc = WNDCLASSW {
            lpfnWndProc: Some(tray_wnd_proc),
            hInstance: hinstance,
            lpszClassName: class_name.as_ptr(),
            ..wc
        };

        // SAFETY: Class descriptor and pointers are valid for this call.
        let atom = unsafe { RegisterClassW(&wc) };
        if atom == 0 {
            return;
        }

        // SAFETY: Creating a hidden helper window with the registered class.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                hinstance,
                std::ptr::null(),
            )
        };

        if hwnd == 0 {
            return;
        }

        // SAFETY: Application icon resource id is standard and valid.
        let hicon = unsafe { LoadIconW(0, IDI_APPLICATION) };

        // SAFETY: Structure is initialized and fields required by NIF flags are populated.
        unsafe {
            let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_ICON_UID;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = TRAY_MESSAGE_ID;
            nid.hIcon = hicon;
            nid.szTip = with_tip(nid.szTip, "gpui-convenience-tools");
            if Shell_NotifyIconW(NIM_ADD, &mut nid) == 0 {
                let _ = DestroyWindow(hwnd);
                return;
            }

            let _ = TRAY_INITIALIZED.set(());

            let mut msg = std::mem::zeroed::<MSG>();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            DestroyWindow(hwnd);
        }
    });

    Ok(())
}

pub fn hide_main_window_to_tray() -> Result<()> {
    let Some(hwnd) = try_find_main_window() else {
        return Err(anyhow!("main window not found"));
    };

    // SAFETY: hwnd is validated by FindWindowW and only used for visibility change.
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }

    Ok(())
}
