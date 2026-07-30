//! 주기 선택(드롭다운 + 직접 추가) 회귀 테스트.

use super::*;

use crate::app::IntervalTarget;

/// 입력값·단위를 채우고 '추가' 버튼을 누른다.
fn add_preset(cx: &mut gpui::VisualTestContext, view: &Entity<AppRoot>, amount: &str, unit: &str) {
    // `set_value`는 `'static` 문자열을 요구하므로 빌린 값을 그대로 넘길 수 없다.
    let amount = amount.to_string();
    let unit = unit.to_string();

    cx.update(|window, app| {
        view.update(app, |root, cx| {
            let input = root
                .interval_picker
                .amount_input
                .clone()
                .expect("amount input should exist after the first render");
            input.update(cx, |state, cx| state.set_value(amount, window, cx));

            let select = root
                .interval_picker
                .unit_select
                .clone()
                .expect("unit select should exist after the first render");
            let index = crate::util::TimeUnit::ALL
                .iter()
                .position(|candidate| candidate.label() == unit)
                .expect("unit label should exist");
            select.update(cx, |state, cx| {
                state.set_selected_index(Some(gpui_component::IndexPath::new(index)), window, cx);
            });
        });
    });
    refresh(cx);
    click_debug_element(cx, "interval-add");
}

#[gpui::test]
fn scan_interval_starts_from_the_default_presets(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::AdBlock));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    cx.update(|_, app| {
        assert_eq!(
            view.read(app).interval_picker.presets,
            vec![10, 30, 60],
            "기본 프리셋은 10초·30초·1분이다"
        );
    });

    // 드롭다운과 프리셋 목록이 실제로 그려진다.
    assert!(cx.debug_bounds("interval-select").is_some());
    for selector in ["interval-preset-10", "interval-preset-30", "interval-preset-60"] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} should be rendered"
        );
    }
}

#[gpui::test]
fn adding_a_custom_unit_registers_the_preset_and_selects_it(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::AdBlock));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    add_preset(cx, &view, "5", "분");

    cx.update(|_, app| {
        let root = view.read(app);
        assert!(
            root.interval_picker.presets.contains(&300),
            "5분이 프리셋에 추가돼야 한다: {:?}",
            root.interval_picker.presets
        );
        assert_eq!(
            root.interval_picker.presets,
            vec![10, 30, 60, 300],
            "프리셋은 항상 오름차순·중복 없음"
        );
        assert_eq!(
            root.scan_interval_secs, 300,
            "추가한 주기를 바로 사용하는 것이 사용자의 의도다"
        );
        assert!(root.interval_picker.error.is_none());
    });

    assert!(
        cx.debug_bounds("interval-preset-300").is_some(),
        "추가한 프리셋이 목록에 보여야 한다"
    );
}

#[gpui::test]
fn invalid_input_reports_a_reason_and_changes_nothing(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::AdBlock));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    add_preset(cx, &view, "abc", "초");

    cx.update(|_, app| {
        let root = view.read(app);
        assert_eq!(
            root.interval_picker.presets,
            vec![10, 30, 60],
            "잘못된 입력은 프리셋을 바꾸지 않는다"
        );
        assert_eq!(root.scan_interval_secs, 10, "현재 주기도 그대로여야 한다");
        assert!(
            root.interval_picker.error.is_some(),
            "사용자에게 보여줄 사유가 있어야 한다"
        );
    });
    assert!(
        cx.debug_bounds("interval-error").is_some(),
        "사유가 화면에 표시돼야 한다"
    );

    // 24시간을 넘는 값도 거부한다.
    add_preset(cx, &view, "25", "시간");
    cx.update(|_, app| {
        assert_eq!(view.read(app).interval_picker.presets, vec![10, 30, 60]);
    });
}

#[gpui::test]
fn presets_are_shared_between_ad_block_and_file_sync(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::AdBlock);
        root.sync_jobs = vec![SyncJob::default()];
        root.selected_sync_job = Some(0);
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);
    add_preset(cx, &view, "2", "시간");

    // 파일 동기화 패널로 옮겨도 같은 목록을 쓴다.
    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.active_panel = ActivePanel::FileSync;
            cx.notify();
        });
    });
    refresh(cx);

    cx.update(|_, app| {
        assert!(
            view.read(app).interval_picker.presets.contains(&7_200),
            "프리셋 목록은 두 패널이 공유한다"
        );
    });
    assert!(
        cx.debug_bounds("interval-preset-7200").is_some(),
        "파일 동기화 패널에도 같은 프리셋이 보여야 한다"
    );
}

#[gpui::test]
fn removing_a_preset_keeps_at_least_one(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::AdBlock));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    cx.update(|window, app| {
        view.update(app, |root, cx| {
            root.remove_interval_preset(30, IntervalTarget::Scan, window, cx);
            root.remove_interval_preset(60, IntervalTarget::Scan, window, cx);
            // 마지막 하나는 남아야 한다.
            root.remove_interval_preset(10, IntervalTarget::Scan, window, cx);
        });
    });
    refresh(cx);

    cx.update(|_, app| {
        let root = view.read(app);
        assert_eq!(
            root.interval_picker.presets,
            vec![10],
            "마지막 프리셋까지 지우면 드롭다운이 비어 고를 수 없게 된다"
        );
        assert!(root.interval_picker.error.is_some());
    });
}

#[gpui::test]
fn current_value_outside_the_presets_stays_selectable(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::AdBlock);
        // 구버전 config에서 온 값처럼 프리셋에 없는 주기.
        root.scan_interval_secs = 45;
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    cx.update(|_, app| {
        let root = view.read(app);
        assert_eq!(
            root.interval_options(45),
            vec![10, 30, 45, 60],
            "현재 값이 프리셋에 없어도 드롭다운에는 들어가야 한다"
        );
        assert_eq!(
            root.interval_picker.presets,
            vec![10, 30, 60],
            "저장된 프리셋 목록 자체는 오염되지 않는다"
        );
    });
}
