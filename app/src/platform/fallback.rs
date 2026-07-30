//! Windows 외 플랫폼용 `Platform` 구현.
//!
//! 이 저장소의 편의 기능 중 웹뷰 광고 차단·Windows 서비스 관리·자동 시작은 Win32와 SCM에
//! 직접 의존해 다른 OS에 대응물이 없다. 그래서 여기서는 **하지 않는다는 사실을 정직하게
//! 알리는 구현**만 둔다. 창을 못 찾은 척하거나 성공을 반환해 UI가 동작하는 것처럼 보이게
//! 만들지 않는다.
//!
//! 서비스 관련 메서드는 [`Platform`] trait의 기본 구현(빈 목록 또는 미지원 오류)을 그대로
//! 쓴다. 파일 동기화는 `sync.rs`가 순수 표준 라이브러리로 구현돼 있어 이 플랫폼에서도
//! 완전히 동작한다.

use anyhow::Result;

use super::{NativeWindowHandle, Platform};

/// 광고 차단 계열 기능이 없는 플랫폼의 기본 구현.
pub struct FallbackPlatform;

impl FallbackPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FallbackPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl Platform for FallbackPlatform {
    fn is_target_running(&self, _process_name: &str) -> bool {
        false
    }

    fn list_running_processes(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn find_ad_window(&self, _process_name: &str) -> Result<Option<NativeWindowHandle>> {
        Ok(None)
    }

    fn hide_ad(&self, _handle: NativeWindowHandle) -> Result<()> {
        Err(anyhow::anyhow!(
            "웹뷰 광고 차단은 Windows 플랫폼에서만 지원됩니다."
        ))
    }

    fn show_ad(&self, _handle: NativeWindowHandle) -> Result<()> {
        Err(anyhow::anyhow!(
            "웹뷰 광고 차단은 Windows 플랫폼에서만 지원됩니다."
        ))
    }
}
