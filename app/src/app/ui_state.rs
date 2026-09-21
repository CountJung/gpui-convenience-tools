//! 기능별 `AppRoot` UI 상태 묶음.
//!
//! GPUI 엔티티는 하나로 유지하고, 기능별 상태의 소유권만 일반 구조체로 나눈다.
//! 이 모듈에는 렌더 생명주기나 별도 엔티티를 두지 않아 기존 이벤트 경계를 보존한다.

use super::state::{SyncJobStatus, SyncRunning, SyncSharedState};
use crate::config::SyncJob;
use crate::platform::SysServiceInfo;
use crate::sync::SyncFailure;
use gpui::{Entity, ScrollHandle};
use gpui_component::{input::InputState, VirtualListScrollHandle};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// Windows 서비스 패널이 소유하는 상태.
pub struct ServiceState {
    pub(crate) items: Vec<SysServiceInfo>,
    pub(crate) search_query: String,
    pub(crate) search_input: Option<Entity<InputState>>,
    pub(crate) scroll: VirtualListScrollHandle,
    pub(crate) right_scroll: ScrollHandle,
    pub(crate) pending_delete: Option<String>,
    pub(crate) filter: ServiceFilter,
    pub(crate) favorites: Vec<String>,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            search_query: String::new(),
            search_input: None,
            scroll: VirtualListScrollHandle::new(),
            right_scroll: ScrollHandle::default(),
            pending_delete: None,
            filter: ServiceFilter::All,
            favorites: Vec::new(),
        }
    }
}

/// 서비스 목록 상태 필터.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ServiceFilter {
    #[default]
    All,
    Running,
    Stopped,
    Favorites,
}

/// 파일 동기화 패널이 소유하는 상태와 백그라운드 공유 경계.
pub struct SyncState {
    pub(crate) enabled: bool,
    pub(crate) jobs: Vec<SyncJob>,
    pub(crate) selected_job: Option<usize>,
    pub(crate) status: HashMap<String, SyncJobStatus>,
    pub(crate) failures: Vec<SyncFailure>,
    pub(crate) running: Option<SyncRunning>,
    pub(crate) suppressed_failures: HashSet<String>,
    pub(crate) notify_enabled: bool,
    pub(crate) name_input: Option<Entity<InputState>>,
    pub(crate) source_input: Option<Entity<InputState>>,
    pub(crate) target_input: Option<Entity<InputState>>,
    pub(crate) exclude_input: Option<Entity<InputState>>,
    pub(crate) page_scroll: ScrollHandle,
    pub(crate) shared: Arc<Mutex<SyncSharedState>>,
    #[cfg(test)]
    pub(crate) external_side_effects_enabled: bool,
}

/// 웹뷰 광고 차단 패널의 스크롤 상태.
#[derive(Default)]
pub struct AdBlockState {
    pub(crate) left_scroll: ScrollHandle,
    pub(crate) right_scroll: ScrollHandle,
}
