// 릴리즈 빌드에서 콘솔 서브시스템 분리:
// 터미널을 닫아도 CTRL_CLOSE_EVENT가 전달되지 않아 트레이 앱이 유지된다.
// 디버그 빌드는 그대로 콘솔에 연결되어 로그 출력이 가능하다.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod config;
mod platform;
mod window;

use app::AppRoot;
use gpui::*;
use gpui_component::Root;
use gpui_component::theme::{Theme, ThemeRegistry};

use config::{ensure_bundled_themes, load_config};

#[cfg(target_os = "windows")]
use platform::{init_tray_icon, hide_main_window_to_tray, run_as_windows_service};

fn main() {
    env_logger::init();

    // ── Windows 서비스 디스패처 진입점 ──────────────────────────────────────
    // SCM이 `--service` 플래그와 함께 이 바이너리를 실행하면
    // GUI 없이 서비스 루프를 시작하고 바로 반환한다.
    #[cfg(target_os = "windows")]
    if std::env::args().any(|a| a == "--service") {
        if let Err(e) = run_as_windows_service() {
            log::error!("Windows service failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    // ── 일반 GUI 앱 실행 ────────────────────────────────────────────────────
    let app = Application::new();
    app.run(move |cx| {
        gpui_component::init(cx);

        let restore_saved_themes = |cx: &mut App| {
            let Ok(Some(cfg)) = load_config() else {
                return;
            };

            let mut has_changes = false;

            if let Some(name) = cfg.light_theme_name.as_deref() {
                if let Some(theme) = ThemeRegistry::global(cx).themes().get(name).cloned() {
                    Theme::global_mut(cx).light_theme = theme;
                    has_changes = true;
                }
            }

            if let Some(name) = cfg.dark_theme_name.as_deref() {
                if let Some(theme) = ThemeRegistry::global(cx).themes().get(name).cloned() {
                    Theme::global_mut(cx).dark_theme = theme;
                    has_changes = true;
                }
            }

            if has_changes {
                let mode = Theme::global(cx).mode;
                Theme::change(mode, None, cx);
            }
        };

        match ensure_bundled_themes() {
            Ok(themes_dir) => {
                if let Err(err) = ThemeRegistry::watch_dir(themes_dir, cx, move |cx| {
                    restore_saved_themes(cx);
                    cx.refresh_windows();
                }) {
                    log::error!("failed to watch custom themes: {err}");
                }
            }
            Err(err) => {
                log::error!("failed to prepare bundled themes: {err}");
            }
        }

        #[cfg(target_os = "windows")]
        if let Err(err) = init_tray_icon() {
            log::error!("failed to initialize tray icon: {err}");
        }

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1000.0), px(700.0)),
                    cx,
                ))),
                titlebar: None,
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("gpui-convenience-tools");
                let view = cx.new(|cx| AppRoot::new(cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open main window");

        // --tray 플래그: 윈도우 생성 직후 트레이로 숨김
        #[cfg(target_os = "windows")]
        if std::env::args().any(|a| a == "--tray") {
            std::thread::spawn(|| {
                // GPUI가 윈도우를 완전히 그릴 때까지 짧게 대기
                std::thread::sleep(std::time::Duration::from_millis(300));
                let _ = hide_main_window_to_tray();
            });
        }
    });
}



