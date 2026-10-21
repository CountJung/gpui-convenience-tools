use super::*;
use gpui::AppContext;

// 실제 실행과 같이 Root로 감싸야 텍스트 입력의 포커스·IME 경로도 검증할 수 있다.
fn add_openssl_window(
    cx: &mut TestAppContext,
    root: AppRoot,
) -> (Entity<AppRoot>, &mut gpui::VisualTestContext) {
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let inner = cx.new(|_| root);
        view = Some(inner.clone());
        gpui_component::Root::new(inner, window, cx)
    });
    (view.unwrap(), cx)
}

#[gpui::test]
fn openssl_buttons_transform_validate_and_clear_text(cx: &mut TestAppContext) {
    initialize_components(cx);
    assert!(NAV_TOOLS
        .iter()
        .any(|(panel, _, _)| *panel == ActivePanel::OpenSsl));
    let (view, cx) = add_openssl_window(cx, test_app_root(ActivePanel::OpenSsl));
    cx.simulate_resize(size(px(1000.0), px(850.0)));
    refresh(cx);
    let input = view.read_with(cx, |view, _| view.openssl.input.clone().unwrap());
    let passphrase = view.read_with(cx, |view, _| view.openssl.passphrase.clone().unwrap());
    // 실제 입력 포커스·키 입력을 거쳐 인코딩을 실행한다.
    cx.update(|window, cx| input.update(cx, |state, cx| state.focus(window, cx)));
    cx.simulate_keystrokes("P C F");
    refresh(cx);
    assert_eq!(
        input.read_with(cx, |state, _| state.value().to_string()),
        "PCF"
    );
    click_debug_element(cx, "openssl-encode");
    let cipher = view.read_with(cx, |view, cx| {
        assert!(!view.openssl.failed);
        view.openssl
            .output
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .to_string()
    });
    assert!(cipher.starts_with("U2FsdGVkX1"));
    assert_eq!(
        crate::openssl_codec::decrypt(&cipher, "911").unwrap().0,
        "PCF"
    );
    let output = view.read_with(cx, |view, _| view.openssl.output.clone().unwrap());
    cx.update(|window, cx| output.update(cx, |state, cx| state.focus(window, cx)));
    cx.simulate_keystrokes("ctrl-a ctrl-c");
    assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), cipher);
    cx.update(|window, cx| input.update(cx, |state, cx| state.focus(window, cx)));
    cx.simulate_keystrokes("ctrl-a ctrl-v");
    refresh(cx);
    click_debug_element(cx, "openssl-decode");
    view.read_with(cx, |view, cx| {
        assert_eq!(
            view.openssl
                .output
                .as_ref()
                .unwrap()
                .read(cx)
                .value()
                .as_str(),
            "PCF"
        );
        assert!(view.openssl.status.contains("SHA"));
    });
    // 결과 편집 이력도 지워야 이전 복호화 평문을 undo로 되살릴 수 없다.
    let edited_output = view.read_with(cx, |view, _| view.openssl.output.clone().unwrap());
    cx.update(|window, cx| edited_output.update(cx, |state, cx| state.focus(window, cx)));
    cx.simulate_keystrokes("X");
    click_debug_element(cx, "openssl-clear");
    let inputs = view.read_with(cx, |view, _| {
        [
            view.openssl.input.clone().unwrap(),
            view.openssl.output.clone().unwrap(),
        ]
    });
    for widget in inputs {
        cx.update(|window, cx| widget.update(cx, |state, cx| state.focus(window, cx)));
        cx.simulate_keystrokes("ctrl-z ctrl-y");
        assert!(widget.read_with(cx, |state, _| state.value().is_empty()));
    }
    let input = view.read_with(cx, |view, _| view.openssl.input.clone().unwrap());
    cx.update(|window, cx| input.update(cx, |state, cx| state.set_value(cipher, window, cx)));
    cx.update(|window, cx| {
        passphrase.update(cx, |state, cx| state.set_value("wrong-code", window, cx))
    });
    refresh(cx);
    click_debug_element(cx, "openssl-decode");
    view.read_with(cx, |view, cx| {
        assert!(view.openssl.failed);
        assert!(view
            .openssl
            .output
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .is_empty());
        assert!(!view.openssl.status.contains("wrong-code"));
    });
    click_debug_element(cx, "openssl-clear");
    view.read_with(cx, |view, cx| {
        assert!(view
            .openssl
            .input
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .is_empty());
        assert!(view
            .openssl
            .output
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .is_empty());
        assert_eq!(view.openssl.config.passphrase, "wrong-code");
        assert!(!view.openssl.failed);
    });
}

#[gpui::test]
fn openssl_settings_autosave_and_rehydrate_inputs(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("openssl-ui");
    crate::config::update_config(|cfg| cfg.scan_interval_secs = 73).unwrap();
    initialize_components(cx);
    let (view, cx) = add_openssl_window(cx, {
        let mut root = test_app_root(ActivePanel::OpenSsl);
        root.sync.external_side_effects_enabled = true;
        root
    });
    cx.simulate_resize(size(px(1000.0), px(850.0)));
    refresh(cx);
    let passphrase = view.read_with(cx, |view, _| view.openssl.passphrase.clone().unwrap());
    cx.update(|window, cx| {
        passphrase.update(cx, |state, cx| {
            state.focus(window, cx);
            state.set_value("", window, cx);
        })
    });
    cx.simulate_keystrokes("4 2 7");
    refresh(cx);
    assert_eq!(
        crate::config::load_config()
            .unwrap()
            .unwrap()
            .openssl
            .passphrase,
        "427"
    );
    click_debug_element(cx, "openssl-newline");
    let restored = crate::config::load_config().unwrap().unwrap();
    assert!(!restored.openssl.append_newline);
    assert_eq!(restored.scan_interval_secs, 73);
    view.update(cx, |root, _| {
        root.save_current_config().unwrap();
    });
    // 손상 파일은 보존하며 실패를 표시하고, 원인 해소 후 전체 저장 성공 시 해제한다.
    let path = crate::config::effective_config_path();
    let saved = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{").unwrap();
    view.update(cx, |root, cx| root.persist_openssl_config(cx));
    view.read_with(cx, |root, _| assert!(root.openssl.save_error));
    assert_eq!(std::fs::read(&path).unwrap(), b"{");
    std::fs::write(&path, saved).unwrap();
    view.update(cx, |root, cx| {
        root.save_current_config().unwrap();
        assert!(!root.openssl.save_error);
        root.sync.external_side_effects_enabled = false;
        cx.notify();
    });
    let restored = crate::config::load_config().unwrap().unwrap().openssl;
    let (reopened, cx2) = add_openssl_window(cx, {
        let mut root = test_app_root(ActivePanel::OpenSsl);
        root.openssl = super::super::openssl_ops::OpenSslState::from_config(restored);
        root
    });
    refresh(cx2);
    reopened.read_with(cx2, |root, cx| {
        assert_eq!(
            root.openssl
                .passphrase
                .as_ref()
                .unwrap()
                .read(cx)
                .value()
                .as_str(),
            "427"
        );
        assert!(!root.openssl.config.append_newline);
        assert!(root
            .openssl
            .input
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .is_empty());
        assert!(root
            .openssl
            .output
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .is_empty());
    });
}

#[gpui::test]
fn openssl_page_contains_inputs_and_scrolls_at_supported_sizes(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_openssl_window(cx, test_app_root(ActivePanel::OpenSsl));
    for (width, height) in [(920.0, 480.0), (1000.0, 700.0), (1500.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        view.update(cx, |view, cx| {
            view.openssl.scroll.set_offset(point(px(0.0), px(0.0)));
            cx.notify();
        });
        refresh(cx);
        let viewport = cx.debug_bounds("openssl-page").unwrap();
        let label = cx.debug_bounds("openssl-newline-label").unwrap();
        assert!(
            label.size.width > px(350.0),
            "option label must not collapse: {label:?}"
        );
        assert!(
            label.size.height <= px(44.0),
            "option label must not wrap vertically: {label:?}"
        );
        for selector in [
            "openssl-passphrase",
            "openssl-input",
            "openssl-output",
            "openssl-encode",
            "openssl-decode",
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.origin.x >= viewport.origin.x);
            assert!(
                bounds.origin.x + bounds.size.width
                    <= viewport.origin.x + viewport.size.width + px(1.0),
                "{selector}: {bounds:?}"
            );
            assert!(bounds.size.width > px(100.0));
        }
        wheel_to_end(cx, "openssl-page", -2000.0);
        let output = cx.debug_bounds("openssl-output").unwrap();
        assert!(output.origin.y >= viewport.origin.y - px(1.0));
        assert!(
            output.origin.y + output.size.height
                <= viewport.origin.y + viewport.size.height + px(1.0)
        );
    }
}
