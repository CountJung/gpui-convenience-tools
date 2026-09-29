//! 카카오톡 기본 채팅창의 광고 자리 회수. 창 계층·크기 지문이 맞지 않으면 아무것도 바꾸지 않는다.

use anyhow::{anyhow, Result};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::{
        GetAncestor, GetParent, GetWindow, GetWindowRect, IsWindow, IsWindowVisible, SetWindowPos,
        GA_ROOT, GW_CHILD, GW_HWNDNEXT, GW_OWNER, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER,
    },
};

use super::window_ops::{class_name_from_hwnd, process_name_from_pid, window_process_id};
use crate::platform::{KakaoLayoutSnapshot, KakaoWindowSize};

const MAX_AD_HEIGHT: i32 = 160;
const MIN_AD_HEIGHT: i32 = 50;

fn direct_children(parent: HWND) -> Vec<HWND> {
    let mut children = Vec::new();
    let mut seen = Vec::new();
    // SAFETY: 읽기 전용으로 부모의 자식과 형제 HWND를 순회한다. 핸들 재사용·순환은 제한한다.
    let mut child = unsafe { GetWindow(parent, GW_CHILD) };
    while child != 0 && seen.len() < 256 && !seen.contains(&child) {
        seen.push(child);
        if unsafe { GetParent(child) } == parent {
            children.push(child);
        }
        child = unsafe { GetWindow(child, GW_HWNDNEXT) };
    }
    children
}

fn descendants(parent: HWND) -> Vec<HWND> {
    let mut found = Vec::new();
    let mut pending = vec![parent];
    while let Some(next) = pending.pop() {
        for child in direct_children(next) {
            if !found.contains(&child) && found.len() < 512 {
                pending.push(child);
                found.push(child);
            }
        }
    }
    found
}

fn rect(hwnd: HWND) -> Result<RECT> {
    let mut value = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: 반환된 HWND의 현재 화면 좌표를 로컬 출력 버퍼로 읽는다.
    if unsafe { GetWindowRect(hwnd, &mut value) } == 0 {
        return Err(anyhow!(
            "카카오톡 창 크기를 읽지 못했습니다 (HWND {hwnd:?})"
        ));
    }
    Ok(value)
}

fn width(rect: RECT) -> i32 {
    rect.right - rect.left
}
fn height(rect: RECT) -> i32 {
    rect.bottom - rect.top
}

fn unique_match(
    children: &[HWND],
    class_name: &str,
    predicate: impl Fn(RECT) -> bool,
) -> Option<HWND> {
    let matches = children
        .iter()
        .copied()
        .filter(|hwnd| {
            (unsafe { IsWindowVisible(*hwnd) }) != 0
                && class_name_from_hwnd(*hwnd) == class_name
                && rect(*hwnd).is_ok_and(&predicate)
        })
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        matches.first().copied()
    } else {
        None
    }
}

fn size_change(hwnd: HWND, target_height: i32, collapse: bool) -> Result<KakaoWindowSize> {
    let original = rect(hwnd)?;
    Ok(KakaoWindowSize {
        handle: hwnd,
        process_id: window_process_id(hwnd)?,
        class_name: class_name_from_hwnd(hwnd),
        parent: unsafe { GetParent(hwnd) },
        owner: unsafe { GetWindow(hwnd, GW_OWNER) },
        original_width: width(original),
        original_height: height(original),
        target_width: if collapse { 0 } else { width(original) },
        target_height: if collapse { 0 } else { target_height },
    })
}

/// 광고 WebView가 속한 팝업과 기본 채팅창의 하단 슬롯·목록을 정확히 식별한다.
pub(super) fn capture(ad_handle: HWND) -> Result<Option<KakaoLayoutSnapshot>> {
    if unsafe { IsWindow(ad_handle) } == 0
        || class_name_from_hwnd(ad_handle) != "Chrome_WidgetWin_1"
    {
        return Ok(None);
    }
    let popup = unsafe { GetAncestor(ad_handle, GA_ROOT) };
    if popup == 0 || class_name_from_hwnd(popup) != "EVA_Window_Dblclk" {
        return Ok(None);
    }
    let main = unsafe { GetWindow(popup, GW_OWNER) };
    if main == 0 || class_name_from_hwnd(main) != "EVA_Window_Dblclk" {
        return Ok(None);
    }
    let main_pid = window_process_id(main)?;
    if process_name_from_pid(main_pid).as_deref() != Some("kakaotalk.exe")
        || window_process_id(popup)? != main_pid
        || unsafe { GetWindow(main, GW_OWNER) } != 0
    {
        return Ok(None);
    }

    let main_rect = rect(main)?;
    let popup_rect = rect(popup)?;
    let ad_height = height(popup_rect);
    if !(MIN_AD_HEIGHT..=MAX_AD_HEIGHT).contains(&ad_height)
        || (popup_rect.bottom - main_rect.bottom).abs() > 12
        || (popup_rect.left - main_rect.left).abs() > 12
        || width(popup_rect) < width(main_rect) - 16
    {
        return Ok(None);
    }

    let children = direct_children(main);
    let slot = unique_match(&children, "EVA_ChildWindow", |r| {
        (r.left - popup_rect.left).abs() <= 2
            && (r.top - popup_rect.top).abs() <= 2
            && (r.right - popup_rect.right).abs() <= 2
            && (r.bottom - popup_rect.bottom).abs() <= 2
    });
    let content = unique_match(&children, "EVA_ChildWindow", |r| {
        width(r) >= width(main_rect) - 10
            && (r.left - main_rect.left).abs() <= 10
            && r.top < popup_rect.top - 200
            && (popup_rect.top - r.bottom) >= 0
            && (popup_rect.top - r.bottom) <= 12
    });
    let (Some(slot), Some(content)) = (slot, content) else {
        return Ok(None);
    };
    let content_rect = rect(content)?;
    let reclaim = popup_rect.bottom - content_rect.bottom;
    if !(MIN_AD_HEIGHT..=MAX_AD_HEIGHT + 16).contains(&reclaim) {
        return Ok(None);
    }
    let content_children = descendants(content);
    let list = unique_match(&content_children, "EVA_VH_ListControl_Dblclk", |r| {
        (r.bottom - content_rect.bottom).abs() <= 2
            && r.top > content_rect.top
            && r.top < content_rect.top + 100
            && width(r) > 200
            && height(r) > 200
    });
    let scrollbar = unique_match(&content_children, "_EVA_CustomScrollCtrl", |r| {
        (r.bottom - content_rect.bottom).abs() <= 2
            && r.top > content_rect.top
            && r.top < content_rect.top + 100
            && width(r) <= 24
            && height(r) > 200
    });
    let (Some(list), Some(scrollbar)) = (list, scrollbar) else {
        return Ok(None);
    };
    let list_host = unsafe { GetParent(list) };
    if list_host == 0 || class_name_from_hwnd(list_host) != "EVA_Window" {
        return Ok(None);
    }
    let list_host_rect = rect(list_host)?;
    if (list_host_rect.bottom - content_rect.bottom).abs() > 2
        || list_host_rect.top != content_rect.top
    {
        return Ok(None);
    }
    let windows = vec![
        size_change(slot, 0, true)?,
        size_change(popup, 0, true)?,
        size_change(content, height(content_rect) + reclaim, false)?,
        size_change(list_host, height(list_host_rect) + reclaim, false)?,
        size_change(list, height(rect(list)?) + reclaim, false)?,
        size_change(scrollbar, height(rect(scrollbar)?) + reclaim, false)?,
    ];
    if windows.iter().any(|entry| entry.process_id != main_pid) {
        return Ok(None);
    }
    // 순서가 실제 조작·역순 복원 순서다. 광고 창은 본래의 별도 스냅샷으로 관리한다.
    Ok(Some(KakaoLayoutSnapshot {
        main_window: main,
        main_process_id: main_pid,
        windows,
    }))
}

fn resize(entry: &KakaoWindowSize, target: bool) -> Result<()> {
    if !same_window(entry) {
        return Err(anyhow!(
            "카카오톡 창이 닫히거나 HWND 소유·계층이 변경되었습니다"
        ));
    }
    let (w, h) = if target {
        (entry.target_width, entry.target_height)
    } else {
        (entry.original_width, entry.original_height)
    };
    // SAFETY: 동일 PID의 살아 있는 HWND에 저장한 크기만 적용한다. 위치·포커스·Z 순서는 유지한다.
    if unsafe {
        SetWindowPos(
            entry.handle,
            0,
            0,
            0,
            w,
            h,
            SWP_NOMOVE | SWP_NOACTIVATE | SWP_NOZORDER,
        )
    } == 0
    {
        return Err(anyhow!(
            "카카오톡 창 크기를 변경하지 못했습니다 (HWND {:?})",
            entry.handle
        ));
    }
    Ok(())
}

fn same_window(entry: &KakaoWindowSize) -> bool {
    (unsafe { IsWindow(entry.handle) }) != 0
        && window_process_id(entry.handle).ok() == Some(entry.process_id)
        && class_name_from_hwnd(entry.handle) == entry.class_name
        && unsafe { GetParent(entry.handle) } == entry.parent
        && unsafe { GetWindow(entry.handle, GW_OWNER) } == entry.owner
}

pub(super) fn apply(snapshot: &KakaoLayoutSnapshot) -> Result<()> {
    if window_process_id(snapshot.main_window)? != snapshot.main_process_id {
        return Err(anyhow!("카카오톡 메인 창이 변경되었습니다"));
    }
    // 모든 대상이 아직 캡처 당시 크기인지 확인한 후에만 조작한다.
    if snapshot.windows.iter().any(|entry| {
        rect(entry.handle).map_or(true, |r| {
            width(r) != entry.original_width || height(r) != entry.original_height
        }) || !same_window(entry)
    }) {
        return Err(anyhow!(
            "카카오톡 레이아웃이 바뀌어 광고 자리 회수를 건너뜁니다"
        ));
    }
    for (index, entry) in snapshot.windows.iter().enumerate() {
        if let Err(err) = resize(entry, true) {
            let rollback_failures = snapshot.windows[..index]
                .iter()
                .rev()
                .filter_map(|previous| resize(previous, false).err())
                .map(|error| error.to_string())
                .collect::<Vec<_>>();
            if rollback_failures.is_empty() {
                return Err(err);
            }
            return Err(anyhow!(
                "{err}; 부분 적용 롤백 실패: {}",
                rollback_failures.join("; ")
            ));
        }
    }
    Ok(())
}

pub(super) fn restore(snapshot: &KakaoLayoutSnapshot) -> Result<()> {
    if window_process_id(snapshot.main_window)? != snapshot.main_process_id {
        return Err(anyhow!("카카오톡 메인 창이 변경되어 복원하지 않습니다"));
    }
    let mut failures = Vec::new();
    for entry in snapshot.windows.iter().rev() {
        if !same_window(entry) {
            continue;
        }
        let Ok(current) = rect(entry.handle) else {
            continue;
        };
        // 사용자가 그 사이 창 크기를 바꿨으면 오래된 치수를 덮어쓰지 않는다.
        if width(current) == entry.target_width && height(current) == entry.target_height {
            if let Err(err) = resize(entry, false) {
                failures.push(err.to_string());
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(failures.join("; ")))
    }
}

pub(super) fn is_applied(snapshot: &KakaoLayoutSnapshot) -> bool {
    window_process_id(snapshot.main_window).ok() == Some(snapshot.main_process_id)
        && snapshot.windows.iter().all(|entry| {
            same_window(entry)
                && rect(entry.handle).is_ok_and(|current| {
                    width(current) == entry.target_width && height(current) == entry.target_height
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_or_unrelated_window_never_matches_kakao_layout() {
        assert!(capture(0).expect("읽기 전용 식별").is_none());
    }

    #[test]
    #[ignore = "현재 머신에서 실행 중인 카카오톡 창이 필요합니다"]
    fn live_kakao_layout_detection_is_read_only() {
        let candidates =
            super::super::window_ops::find_ad_windows("KakaoTalk.exe", "Chrome_WidgetWin_1");
        let layouts = candidates
            .into_iter()
            .filter_map(|hwnd| capture(hwnd).ok().flatten())
            .collect::<Vec<_>>();
        assert!(
            !layouts.is_empty(),
            "카카오톡 기본 채팅창 광고 영역을 식별하지 못했습니다"
        );
        for layout in layouts {
            assert_eq!(layout.windows.len(), 6);
            assert!(layout.windows[0].original_height >= MIN_AD_HEIGHT);
            assert!(layout.windows[2].target_height > layout.windows[2].original_height);
        }
    }

    #[test]
    #[ignore = "실행 중인 카카오톡 기본 채팅창 크기를 잠시 변경한 뒤 복원합니다"]
    fn live_kakao_layout_apply_and_restore() {
        struct Restore<'a>(&'a KakaoLayoutSnapshot);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                restore(self.0).expect("카카오톡 원래 크기 복원");
            }
        }
        let layout =
            super::super::window_ops::find_ad_windows("KakaoTalk.exe", "Chrome_WidgetWin_1")
                .into_iter()
                .find_map(|hwnd| capture(hwnd).ok().flatten())
                .expect("카카오톡 기본 채팅창 식별");
        let restore_on_exit = Restore(&layout);
        apply(&layout).expect("광고 자리 회수 적용");
        assert!(is_applied(&layout), "모든 창이 목표 크기여야 합니다");
        for second in 1..=6 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            assert!(
                is_applied(&layout),
                "{second}초 후에도 회수한 크기를 유지해야 합니다"
            );
        }
        drop(restore_on_exit);
        assert!(
            layout.windows.iter().all(|entry| {
                rect(entry.handle).is_ok_and(|r| {
                    width(r) == entry.original_width && height(r) == entry.original_height
                })
            }),
            "모든 창이 원래 크기로 돌아와야 합니다"
        );
    }
}
