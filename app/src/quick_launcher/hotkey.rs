//! 전용 메시지 스레드의 RegisterHotKey 수명. 종료·변경 시 같은 스레드에서 해제한다.
use crate::config::launcher::LauncherHotkey;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyEvent {
    Ready,
    Activated,
    Failed(u32),
    Released,
}

pub struct HotkeyRegistration {
    stop: Sender<()>,
    pub events: Receiver<HotkeyEvent>,
    thread: Option<JoinHandle<()>>,
}

impl HotkeyRegistration {
    pub fn start(hotkey: LauncherHotkey) -> Result<Self, std::io::Error> {
        // Win32 virtual-key/modifier constants, kept numeric for cross-platform compilation.
        let binding = match hotkey {
            LauncherHotkey::CtrlAltSpace => (0x0002 | 0x0001, 0x20),
            LauncherHotkey::CtrlAltF9 => (0x0002 | 0x0001, 0x78),
        };
        Self::start_binding(binding)
    }

    fn start_binding(binding: (u32, u32)) -> Result<Self, std::io::Error> {
        let (stop, stop_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("launcher-hotkey".into())
            .spawn(move || run(binding, stop_rx, event_tx))?;
        Ok(Self {
            stop,
            events,
            thread: Some(thread),
        })
    }
}

impl Drop for HotkeyRegistration {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                log::error!("빠른 실행기 단축키 스레드 종료 실패");
            }
        }
    }
}

#[cfg(windows)]
fn run(binding: (u32, u32), stop: Receiver<()>, events: Sender<HotkeyEvent>) {
    use windows_sys::Win32::{
        Foundation::GetLastError,
        UI::{
            Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT},
            WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY},
        },
    };
    let (modifiers, key) = binding;
    // SAFETY: HWND=0 binds this ID to the current dedicated thread, not another window.
    if unsafe { RegisterHotKey(0, 1, modifiers | MOD_NOREPEAT, key) } == 0 {
        // SAFETY: read immediately after the failed call on this same thread.
        let error = unsafe { GetLastError() };
        let _ = events.send(HotkeyEvent::Failed(error));
        return;
    }
    let _ = events.send(HotkeyEvent::Ready);
    loop {
        // SAFETY: MSG is a plain Win32 output struct; all-zero initialization is valid.
        let mut message: MSG = unsafe { std::mem::zeroed() };
        // SAFETY: valid initialized output; query only this thread's queue, no window dispatch.
        while unsafe { PeekMessageW(&mut message, 0, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) } != 0 {
            if message.wParam == 1 {
                let _ = events.send(HotkeyEvent::Activated);
            }
        }
        match stop.recv_timeout(std::time::Duration::from_millis(20)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    // SAFETY: same thread and ID as successful registration. This code owns its lifetime.
    let released = unsafe { UnregisterHotKey(0, 1) } != 0;
    if released {
        let _ = events.send(HotkeyEvent::Released);
    } else {
        log::error!("빠른 실행기 단축키 해제 실패");
    }
}

#[cfg(not(windows))]
fn run(_: (u32, u32), _: Receiver<()>, events: Sender<HotkeyEvent>) {
    let _ = events.send(HotkeyEvent::Failed(50));
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn release_and_check(mut registration: HotkeyRegistration) {
        registration.stop.send(()).unwrap();
        registration.thread.take().unwrap().join().unwrap();
        assert!(
            registration
                .events
                .try_iter()
                .any(|event| event == HotkeyEvent::Released),
            "successful registration must explicitly unregister on its owning thread"
        );
    }

    fn check_product_binding(hotkey: LauncherHotkey) {
        // Temporarily register only if available. Never unregister an existing owner's binding,
        // send keys, activate a window, or retry a conflicting registration.
        let registration = HotkeyRegistration::start(hotkey).unwrap();
        match registration
            .events
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
        {
            HotkeyEvent::Ready => {
                release_and_check(registration);
                println!("{}: registered and explicitly released", hotkey.label());
            }
            HotkeyEvent::Failed(1409) => {
                drop(registration);
                println!("{}: existing binding preserved (OS 1409)", hotkey.label());
            }
            unexpected => panic!("unexpected registration result for {hotkey:?}: {unexpected:?}"),
        }
    }

    #[test]
    fn launcher_os_hotkey_default_register_or_conflict() {
        check_product_binding(LauncherHotkey::CtrlAltSpace);
    }

    #[test]
    fn launcher_os_hotkey_alternative_register_or_conflict() {
        check_product_binding(LauncherHotkey::CtrlAltF9);
    }

    #[test]
    fn launcher_os_hotkey_owned_queue_message_emits_activated() {
        use std::{
            os::windows::io::AsRawHandle,
            time::{Duration, Instant},
        };
        use windows_sys::Win32::{
            Foundation::{GetLastError, ERROR_INVALID_THREAD_ID},
            System::Threading::GetThreadId,
            UI::WindowsAndMessaging::{PostThreadMessageW, WM_HOTKEY},
        };

        // Separate from the F24 registration/conflict/release test. F23 is test-only;
        // no global key is sent and no application window receives these messages.
        let binding = (0x0007, 0x86);
        let registration = HotkeyRegistration::start_binding(binding).unwrap();
        match registration
            .events
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
        {
            HotkeyEvent::Ready => {}
            HotkeyEvent::Failed(1409) => {
                drop(registration);
                println!("QUEUE_TEST_BLOCKED: test binding owned elsewhere; no message posted");
                return;
            }
            unexpected => panic!("unexpected queue-test registration: {unexpected:?}"),
        }
        let thread = registration.thread.as_ref().unwrap();
        // SAFETY: the owned JoinHandle keeps this live thread handle valid until cleanup.
        let thread_id = unsafe { GetThreadId(thread.as_raw_handle() as isize) };
        assert_ne!(thread_id, 0);
        let post = |id| {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                // SAFETY: target only the thread kept alive by our registration. Parameters
                // are integer message payloads, not pointers, input events, or window handles.
                let posted = unsafe {
                    PostThreadMessageW(
                        thread_id,
                        WM_HOTKEY,
                        id,
                        ((binding.1 << 16) | binding.0) as isize,
                    )
                };
                if posted != 0 {
                    break;
                }
                // SAFETY: read this calling thread's error immediately after the failed post.
                let error = unsafe { GetLastError() };
                assert_eq!(
                    error, ERROR_INVALID_THREAD_ID,
                    "unexpected queue-post error"
                );
                assert!(
                    Instant::now() < deadline,
                    "owned message queue was not ready"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        post(2); // An ID that this registration does not own must be ignored.
        assert_eq!(
            registration.events.recv_timeout(Duration::from_millis(250)),
            Err(mpsc::RecvTimeoutError::Timeout),
            "unowned ID must not activate before the owned ID is posted"
        );
        post(1);
        assert_eq!(
            registration
                .events
                .recv_timeout(Duration::from_secs(2))
                .unwrap(),
            HotkeyEvent::Activated
        );
        assert!(matches!(
            registration.events.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        release_and_check(registration);
        println!("QUEUE_DELIVERED: owned WM_HOTKEY emitted Activated; registration released");
    }

    #[test]
    fn launcher_os_hotkey_register_conflict_release() {
        // Test-only Ctrl+Alt+Shift+F24. No input injection, focus changes, or configured user combo.
        let binding = (0x0007, 0x87);
        let first = HotkeyRegistration::start_binding(binding).unwrap();
        assert_eq!(
            first
                .events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            HotkeyEvent::Ready
        );
        let conflict = HotkeyRegistration::start_binding(binding).unwrap();
        assert_eq!(
            conflict
                .events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            HotkeyEvent::Failed(1409)
        );
        drop(conflict);
        release_and_check(first);
        let restored = HotkeyRegistration::start_binding(binding).unwrap();
        assert_eq!(
            restored
                .events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            HotkeyEvent::Ready
        );
        release_and_check(restored);
    }
}
