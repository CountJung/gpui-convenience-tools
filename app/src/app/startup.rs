//! 앱 시작 설정 복원·엔티티 생성·주기 갱신 루프.
use super::*;

impl AppRoot {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let platform: Arc<dyn Platform> = Arc::new(NativePlatform::new());
        let mut app_state = AppState::default();
        let mut initial_scan_interval_secs: u32 = 10;
        let mut kakao_reclaim_ad_space = false;
        let mut sync_jobs = Vec::new();
        let mut favorite_services = Vec::new();
        let mut log_config = LogConfig::default();
        let mut initial_interval_presets = default_interval_presets();
        let mut sync_enabled = true;
        let mut sidebar_width = DEFAULT_SIDEBAR_WIDTH;
        let mut virtual_disk_suppressed_issue_keys = BTreeSet::new();
        let mut virtual_disk_config = VirtualDiskConfig::default();
        let mut openssl_config = crate::config::OpenSslConfig::default();
        let mut timestamp_config = crate::config::TimestampConfig::default();
        let mut process_monitor_config = crate::config::ProcessMonitorConfig::default();
        let mut clipboard_config = crate::config::ClipboardConfig::default();
        let mut launcher_config = crate::config::LauncherConfig::default();
        let mut screenshot_config = crate::config::ScreenshotConfig::default();
        let initial_sync_history = crate::sync_history::load_recent(20).unwrap_or_else(|err| {
            log::warn!("동기화 이력 불러오기 실패: {err:#}");
            Vec::new()
        });

        let loaded_config = load_config();
        if let Err(error) = &loaded_config {
            log::error!("설정 파일을 불러오지 못했습니다: {error:#}");
            app_state.log_entries.push(LogEntry {
                level: "ERROR".to_string(),
                message: format!("설정 파일을 불러오지 못했습니다: {error:#}"),
            });
        }
        if let Ok(Some(cfg)) = loaded_config {
            openssl_config = cfg.openssl.clone();
            timestamp_config = cfg.timestamp.clone();
            process_monitor_config = cfg.process_monitor.clone();
            clipboard_config = cfg.clipboard.clone();
            launcher_config = cfg.launcher.clone();
            screenshot_config = cfg.screenshot.clone();
            virtual_disk_config = cfg.virtual_disk.clone();
            sync_enabled = cfg.sync_enabled;
            sidebar_width = normalize_sidebar_width(cfg.sidebar_width);
            virtual_disk_suppressed_issue_keys.extend(
                cfg.virtual_disk_suppressed_issue_keys
                    .into_iter()
                    .filter(|key| !key.trim().is_empty()),
            );
            app_state.is_active = cfg.service_enabled;
            kakao_reclaim_ad_space = cfg.kakao_reclaim_ad_space;
            if !cfg.targets.is_empty() {
                app_state.targets = cfg.targets;
            }
            initial_scan_interval_secs = cfg.scan_interval_secs.max(1);
            sync_jobs = cfg.sync_jobs;
            // 구버전 config에는 작업 ID가 없으므로 여기서 채운다.
            for job in sync_jobs.iter_mut() {
                job.ensure_id();
            }
            favorite_services = cfg.favorite_services;
            // 구버전 config에는 프리셋이 없어 기본값이 들어온다. 손상된 값은 여기서 거른다.
            initial_interval_presets = cfg.interval_presets;
            normalize_interval_presets(&mut initial_interval_presets);
            if initial_interval_presets.is_empty() {
                initial_interval_presets = default_interval_presets();
            }
            log_config = cfg.log;
            app_state.log_entries.push(LogEntry {
                level: "INFO".to_string(),
                message: "설정 파일을 불러왔습니다.".to_string(),
            });
        }

        let initial_running = app_state
            .targets
            .iter()
            .filter(|t| t.enabled)
            .any(|t| platform.is_target_running(&t.process_name));
        app_state.is_target_running = initial_running;

        let (event_tx, event_rx) = unbounded_channel();
        let scanner_state = Arc::new(Mutex::new(ScannerState {
            service_enabled: app_state.is_active,
            targets: app_state.targets.clone(),
            scan_interval_secs: initial_scan_interval_secs,
            kakao_reclaim_ad_space,
        }));
        let kakao_layout_state = Arc::new(Mutex::new(None));
        // 끊긴 지점은 작업에 저장돼 있다. 실행 중에는 공유 상태가 정본이므로 여기로 옮긴다.
        let cursors = sync_jobs
            .iter()
            .filter_map(|job| {
                job.resume_cursor
                    .clone()
                    .map(|cursor| (job.id.clone(), cursor))
            })
            .collect();
        let sync_shared = Arc::new(Mutex::new(SyncSharedState {
            jobs: sync_jobs.clone(),
            auto_enabled: sync_enabled,
            cursors,
            ..SyncSharedState::default()
        }));

        #[cfg(target_os = "windows")]
        {
            set_tray_service_active(app_state.is_active);

            let event_tx_for_tray = event_tx.clone();
            let scanner_state_for_tray = Arc::clone(&scanner_state);
            set_tray_toggle_handler(Arc::new(move |enabled| {
                if let Ok(mut state) = scanner_state_for_tray.lock() {
                    state.service_enabled = enabled;
                }

                if let Err(err) = update_config(|cfg| cfg.service_enabled = enabled) {
                    log::error!("트레이 토글 설정 저장 실패: {err}");
                }

                let _ = event_tx_for_tray.send(PlatformEvent::ServiceToggled(enabled));
            }));
        }

        Self::spawn_platform_loop(
            Arc::clone(&platform),
            event_tx.clone(),
            Arc::clone(&scanner_state),
            Arc::clone(&kakao_layout_state),
        );
        Self::spawn_sync_loop(event_tx.clone(), Arc::clone(&sync_shared));

        let running_processes = platform.list_running_processes().unwrap_or_default();
        let selected_sync_job = (!sync_jobs.is_empty()).then_some(0);

        let mut root = Self {
            active_panel: initial_panel_from_validation_env(),
            app_state,
            theme_filter_query: String::new(),
            theme_filter_active_only: false,
            theme_filter_input: None,
            running_processes,
            platform,
            event_tx,
            event_rx,
            log_scroll_handle: VirtualListScrollHandle::new(),
            scanner_state,
            kakao_layout_state,
            subscriptions: Vec::new(),
            scan_interval_secs: initial_scan_interval_secs,
            kakao_reclaim_ad_space,
            sidebar_width,
            interval_picker: IntervalPicker {
                presets: initial_interval_presets,
                ..IntervalPicker::default()
            },

            services: ServiceState {
                favorites: favorite_services,
                ..ServiceState::default()
            },
            sync: SyncState {
                enabled: sync_enabled,
                jobs: sync_jobs,
                selected_job: selected_sync_job,
                status: Default::default(),
                history: initial_sync_history,
                failures: Vec::new(),
                running: None,
                suppressed_failures: Default::default(),
                notify_enabled: true,
                name_input: None,
                source_input: None,
                target_input: None,
                exclude_input: None,
                browse_source: String::new(),
                browse_relative: Default::default(),
                browse_entries: Vec::new(),
                browse_page: 0,
                browse_error: None,
                browse_scroll: ScrollHandle::default(),
                page_scroll: ScrollHandle::default(),
                shared: sync_shared,
                #[cfg(test)]
                external_side_effects_enabled: true,
            },
            ad_block: AdBlockState::default(),
            openssl: openssl_ops::OpenSslState::from_config(openssl_config),
            timestamp: timestamp_ops::TimestampState::from_config(timestamp_config),
            process_monitor: process_ops::ProcessMonitorState::from_config(process_monitor_config),
            clipboard: clipboard_ops::ClipboardState::from_config(clipboard_config),
            launcher: launcher_ops::LauncherState::from_config(launcher_config),
            screenshot: screenshot_ops::ScreenshotState::from_config(screenshot_config),

            virtual_disk: VirtualDiskSession {
                path_text: virtual_disk_config
                    .last_vdi_path
                    .clone()
                    .unwrap_or_default(),
                initial_guest_path: virtual_disk_config
                    .last_guest_path
                    .as_deref()
                    .and_then(|path| crate::virtual_disk::GuestPath::new(path).ok()),
                initial_guest_path_partition_number: virtual_disk_config.last_partition_number,
                copy: virtual_disk_copy::VirtualDiskCopyState {
                    target_path_text: virtual_disk_config
                        .last_target_path
                        .clone()
                        .unwrap_or_default(),
                    ..Default::default()
                },
                layout: virtual_disk_config.layout.clone(),
                suppressed_issue_keys: virtual_disk_suppressed_issue_keys,
                ..VirtualDiskSession::default()
            },
            virtual_disk_page_scroll: ScrollHandle::default(),

            log_config,

            sidebar_scroll_handle: ScrollHandle::default(),
            content_scroll_handle: ScrollHandle::default(),
        };

        seed_validation_virtual_disk(&mut root, cx);
        root.tick_launcher(cx);
        Self::start_event_refresh_loop(cx);

        root
    }

    /// 백그라운드 채널에 새 이벤트가 있을 때 GPUI 렌더 루프를 깨운다.
    pub(super) fn start_event_refresh_loop(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            Timer::after(Duration::from_millis(200)).await;
            let Some(this) = this.upgrade() else {
                break;
            };
            if this
                .update(cx, |this, cx| {
                    this.tick_process_monitor(cx);
                    this.tick_clipboard(cx);
                    this.tick_launcher(cx);
                    if !this.event_rx.is_empty() {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
    }
}
