//! 합성 토폴로지만 사용한다. 캡처 함수나 실제 OS 열거를 호출하지 않는다.
use super::*;

pub(crate) fn fixture_monitors() -> Vec<Monitor> {
    vec![
        Monitor {
            device_id: "fixture-A".into(),
            rect: DesktopRect {
                left: 0,
                top: 0,
                width: 1920,
                height: 1080,
            },
            primary: true,
        },
        Monitor {
            device_id: "fixture-B".into(),
            rect: DesktopRect {
                left: -2560,
                top: -400,
                width: 2560,
                height: 1440,
            },
            primary: false,
        },
    ]
}

#[test]
fn screenshot_monitor_primary_change_and_disconnect_require_reselection() {
    let original = fixture_monitors();
    let request = CaptureRequest {
        target: CaptureTarget::Monitor(0),
        device_id: Some("fixture-A".into()),
        topology: topology(&original),
    };
    assert_eq!(
        resolve_capture_rect(&request, &original).unwrap(),
        original[0].rect
    );
    let mut switched = original.clone();
    switched[0].primary = false;
    switched[1].primary = true;
    switched.reverse();
    assert_eq!(
        resolve_capture_rect(&request, &switched),
        Err(ScreenshotError::MonitorChanged)
    );
    assert_eq!(
        resolve_capture_rect(&request, &switched[..1]),
        Err(ScreenshotError::MonitorChanged)
    );
    let mut replacement = original.clone();
    replacement[0].device_id = "fixture-replacement".into();
    assert_eq!(
        resolve_capture_rect(&request, &replacement),
        Err(ScreenshotError::MonitorChanged)
    );
    // 명시적으로 재선택한 뒤에도 index=0의 B가 아니라 ID=A의 사각형을 사용한다.
    let reselected = CaptureRequest {
        topology: topology(&switched),
        ..request
    };
    assert_eq!(
        resolve_capture_rect(&reselected, &switched).unwrap(),
        original[0].rect
    );
}

#[test]
fn screenshot_monitor_legacy_ambiguous_and_layout_changes_fail_closed() {
    let original = fixture_monitors();
    let legacy = CaptureRequest {
        target: CaptureTarget::Monitor(0),
        device_id: None,
        topology: topology(&original),
    };
    assert_eq!(
        resolve_capture_rect(&legacy, &original),
        Err(ScreenshotError::MonitorChanged)
    );
    let mut ambiguous = original.clone();
    ambiguous[1].device_id = ambiguous[0].device_id.clone();
    let request = CaptureRequest {
        device_id: Some("fixture-A".into()),
        topology: topology(&ambiguous),
        ..legacy
    };
    assert_eq!(
        resolve_capture_rect(&request, &ambiguous),
        Err(ScreenshotError::MonitorChanged)
    );
    let all = CaptureRequest {
        target: CaptureTarget::AllDisplays,
        device_id: None,
        topology: topology(&original),
    };
    let mut moved = original.clone();
    moved[0].rect.left += 1;
    assert_eq!(
        resolve_capture_rect(&all, &moved),
        Err(ScreenshotError::MonitorChanged)
    );
    let mut reversed = original.clone();
    reversed.reverse();
    assert_eq!(
        resolve_capture_rect(&all, &reversed).unwrap(),
        desktop_union(&original).unwrap()
    );
}
