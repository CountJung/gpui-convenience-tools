use super::*;

static DATA_DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) struct IsolatedDataDir {
    previous: Option<std::ffi::OsString>,
    path: PathBuf,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl IsolatedDataDir {
    pub(crate) fn new(name: &str) -> Self {
        let lock = DATA_DIR_LOCK
            .lock()
            .expect("lock isolated config environment");
        let path =
            std::env::temp_dir().join(format!("gct-config-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create isolated config directory");
        let previous = std::env::var_os(DATA_DIR_ENV);
        std::env::set_var(DATA_DIR_ENV, &path);
        Self {
            previous,
            path,
            _lock: lock,
        }
    }
}

impl Drop for IsolatedDataDir {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var(DATA_DIR_ENV, previous);
        } else {
            std::env::remove_var(DATA_DIR_ENV);
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// 구버전 config.json(신규 필드 없음)이 그대로 로드되어야 한다.
#[test]
fn legacy_config_without_new_fields_still_parses() {
    let legacy = r#"{
        "service_enabled": true,
        "targets": [],
        "scan_interval_secs": 15
    }"#;

    let config: AppConfig = serde_json::from_str(legacy).expect("구버전 config 파싱");
    assert_eq!(config.scan_interval_secs, 15);
    assert!(!config.kakao_reclaim_ad_space);
    assert_eq!(config.theme_mode, None);
    assert!(config.sync_jobs.is_empty());
    // 스위치가 없던 config는 켜진 상태로 읽혀야 기존 동작이 유지된다.
    assert!(config.sync_enabled);
    // 신규 필드는 기본값으로 채워진다.
    assert!(config.log.file_enabled);
    assert_eq!(config.log.max_files, default_max_files());
    assert_eq!(config.sidebar_width, DEFAULT_SIDEBAR_WIDTH);
    assert!(!config.clipboard.enabled());
    assert!(config.virtual_disk_suppressed_issue_keys.is_empty());
    assert_eq!(config.virtual_disk, VirtualDiskConfig::default());
    assert_eq!(
        config.virtual_disk.layout,
        VirtualDiskLayoutConfig::default()
    );
}

#[test]
fn kakao_reclaim_is_opt_in_and_round_trips() {
    let mut config = AppConfig::default();
    assert!(!config.kakao_reclaim_ad_space);
    config.kakao_reclaim_ad_space = true;
    let serialized = serde_json::to_string(&config).expect("설정 직렬화");
    let restored: AppConfig = serde_json::from_str(&serialized).expect("설정 역직렬화");
    assert!(restored.kakao_reclaim_ad_space);
}

#[test]
fn trailing_legacy_brace_is_read_without_changing_original() {
    let _data_dir = IsolatedDataDir::new("legacy-extra-brace");
    let path = fallback_config_path();
    let mut expected = AppConfig {
        kakao_reclaim_ad_space: true,
        scan_interval_secs: 45,
        ..AppConfig::default()
    };
    expected.log.max_files = 17;
    let malformed = format!("{}}}", serde_json::to_string(&expected).unwrap());
    fs::write(&path, &malformed).unwrap();

    let restored = load_config().unwrap().unwrap();
    assert!(restored.kakao_reclaim_ad_space);
    assert_eq!(restored.scan_interval_secs, 45);
    assert_eq!(restored.log.max_files, 17);
    assert_eq!(fs::read_to_string(path).unwrap(), malformed);
}

#[test]
fn malformed_settings_other_than_legacy_trailing_brace_are_not_overwritten() {
    let _data_dir = IsolatedDataDir::new("invalid-settings-preserved");
    let path = config_path();
    fs::write(&path, "{ invalid json").unwrap();
    assert!(update_config(|config| config.kakao_reclaim_ad_space = true).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "{ invalid json");
}

#[test]
fn all_user_settings_survive_save_and_reload() {
    let _data_dir = IsolatedDataDir::new("all-settings-round-trip");
    let mut settings = AppConfig {
        service_enabled: false,
        ..AppConfig::default()
    };
    settings.targets[0].enabled = false;
    settings.kakao_reclaim_ad_space = true;
    settings.light_theme_name = Some("test-light".into());
    settings.dark_theme_name = Some("test-dark".into());
    settings.theme_mode = Some(ThemeMode::Dark);
    settings.scan_interval_secs = 42;
    settings.favorite_services = vec!["TestService".into()];
    settings.interval_presets = vec![10, 42, 90];
    settings.sync_enabled = false;
    settings.sync_jobs = vec![SyncJob {
        source: r"D:\source".into(),
        target: r"D:\target".into(),
        ..SyncJob::default()
    }];
    settings.sidebar_width = 300.0;
    settings.virtual_disk_suppressed_issue_keys = vec!["hidden-file".into()];
    settings.log.max_files = 12;
    settings.virtual_disk.last_vdi_path = Some(r"D:\VM\disk.vdi".into());

    save_config(&settings).unwrap();
    let restored = load_config().unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(&settings).unwrap()
    );
}

#[test]
fn theme_mode_and_theme_name_save_without_resetting_other_settings() {
    let _data_dir = IsolatedDataDir::new("theme-mode-save");
    let initial = AppConfig {
        kakao_reclaim_ad_space: true,
        ..AppConfig::default()
    };
    save_config(&initial).unwrap();
    save_theme_mode(ThemeMode::Dark).unwrap();
    save_theme_selection(ThemeMode::Dark, "test-dark").unwrap();
    let restored = load_config().unwrap().unwrap();
    assert_eq!(restored.theme_mode, Some(ThemeMode::Dark));
    assert_eq!(restored.dark_theme_name.as_deref(), Some("test-dark"));
    assert!(restored.kakao_reclaim_ad_space);
}

#[test]
fn concurrent_setting_updates_preserve_both_fields() {
    let _data_dir = IsolatedDataDir::new("concurrent-setting-updates");
    save_config(&AppConfig::default()).unwrap();
    let first = std::thread::spawn(|| {
        for value in 20..40 {
            update_config(|config| config.scan_interval_secs = value).unwrap();
        }
    });
    let second = std::thread::spawn(|| {
        for value in 20..40 {
            update_config(|config| config.log.max_files = value).unwrap();
        }
    });
    first.join().unwrap();
    second.join().unwrap();
    let restored = load_config().unwrap().unwrap();
    assert_eq!(restored.scan_interval_secs, 39);
    assert_eq!(restored.log.max_files, 39);
}

#[test]
fn isolated_override_keeps_settings_path_outside_the_executable_directory() {
    let data_dir = IsolatedDataDir::new("isolated-settings-path");

    assert_eq!(config_path(), data_dir.path.join(SETTINGS_FILE_NAME));
    assert_eq!(effective_config_path(), config_path());
}

#[test]
fn normal_runtime_path_uses_settings_json_next_to_the_executable() {
    let _lock = DATA_DIR_LOCK.lock().expect("lock config environment");
    let previous = std::env::var_os(DATA_DIR_ENV);
    std::env::remove_var(DATA_DIR_ENV);

    let expected_parent = std::env::current_exe()
        .expect("current executable")
        .parent()
        .expect("executable parent")
        .to_path_buf();
    let path = config_path();

    if let Some(previous) = previous {
        std::env::set_var(DATA_DIR_ENV, previous);
    }

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(SETTINGS_FILE_NAME)
    );
    assert_eq!(path.parent(), Some(expected_parent.as_path()));
}

#[test]
fn virtual_disk_settings_round_trip() {
    let _data_dir = IsolatedDataDir::new("virtual-disk-settings");
    let config = AppConfig {
        virtual_disk: VirtualDiskConfig {
            last_vdi_path: Some(r"D:\VM\Windows.vdi".to_string()),
            last_partition_number: Some(5),
            last_guest_path: Some("/Users/Public".to_string()),
            last_target_path: Some(r"D:\Recovered".to_string()),
            layout: VirtualDiskLayoutConfig {
                tree_width: 204.0,
                kind_width: 72.0,
                attributes_width: 132.0,
                size_width: 108.0,
            },
        },
        ..AppConfig::default()
    };

    save_config(&config).expect("save VirtualBox settings");
    let restored = load_config()
        .expect("load VirtualBox settings")
        .expect("VirtualBox settings exist");

    assert_eq!(restored.virtual_disk, config.virtual_disk);
}

#[test]
fn sidebar_width_is_normalized_to_supported_range() {
    assert_eq!(normalize_sidebar_width(100.0), MIN_SIDEBAR_WIDTH);
    assert_eq!(normalize_sidebar_width(480.0), MAX_SIDEBAR_WIDTH);
    assert_eq!(normalize_sidebar_width(f32::NAN), DEFAULT_SIDEBAR_WIDTH);
    assert_eq!(normalize_sidebar_width(320.0), 320.0);
}

#[test]
fn virtual_disk_layout_widths_are_normalized_to_readable_ranges() {
    assert_eq!(normalize_virtual_disk_tree_width(100.0), 150.0);
    assert_eq!(normalize_virtual_disk_tree_width(400.0), 240.0);
    assert_eq!(normalize_virtual_disk_tree_width(f32::NAN), 180.0);
    assert_eq!(normalize_virtual_disk_attributes_width(80.0), 96.0);
    assert_eq!(normalize_virtual_disk_size_width(200.0), 144.0);
    assert_eq!(normalize_virtual_disk_kind_width(72.0), 72.0);
}

/// 구버전 SyncJob에는 id가 없으므로 ensure_id로 채워야 한다.
#[test]
fn sync_job_without_id_gets_one() {
    let legacy = r#"{"source":"C:\\a","target":"C:\\b"}"#;
    let mut job: SyncJob = serde_json::from_str(legacy).expect("구버전 SyncJob 파싱");

    assert!(job.id.is_empty());
    assert!(job.exclude_patterns.is_empty());
    assert_eq!(job.watch_mode, WatchMode::Interval);
    job.ensure_id();
    assert!(!job.id.is_empty());

    // 이미 ID가 있으면 유지한다.
    let existing = job.id.clone();
    job.ensure_id();
    assert_eq!(job.id, existing);
}

/// 실시간 감시 모드는 설정에 저장·복원되고, 필드가 없는 구버전 설정은 주기 모드로 남는다.
#[test]
fn sync_job_watch_mode_round_trips_with_interval_compatibility() {
    let job = SyncJob {
        watch_mode: WatchMode::Realtime,
        ..SyncJob::default()
    };

    let json = serde_json::to_string(&job).expect("SyncJob 감시 방식 직렬화");
    assert!(json.contains("\"watch_mode\":\"realtime\""));

    let restored: SyncJob = serde_json::from_str(&json).expect("SyncJob 감시 방식 복원");
    assert_eq!(restored.watch_mode, WatchMode::Realtime);

    let legacy = r#"{"source":"C:\\a","target":"C:\\b","interval_secs":30}"#;
    let restored_legacy: SyncJob =
        serde_json::from_str(legacy).expect("감시 방식 없는 구버전 SyncJob");
    assert_eq!(restored_legacy.watch_mode, WatchMode::Interval);
    assert_eq!(restored_legacy.symlink_mode, SymlinkMode::Skip);
}

/// 링크 처리 모드는 snake_case로 저장되고 구버전 작업은 안전한 Skip으로 읽힌다.
#[test]
fn sync_job_symlink_mode_round_trips_with_skip_compatibility() {
    for (mode, encoded) in [
        (SymlinkMode::Skip, "skip"),
        (SymlinkMode::Follow, "follow"),
        (SymlinkMode::Recreate, "recreate"),
    ] {
        let job = SyncJob {
            symlink_mode: mode,
            ..SyncJob::default()
        };
        let json = serde_json::to_string(&job).expect("링크 처리 모드 직렬화");
        assert!(json.contains(&format!("\"symlink_mode\":\"{encoded}\"")));

        let restored: SyncJob = serde_json::from_str(&json).expect("링크 처리 모드 복원");
        assert_eq!(restored.symlink_mode, mode);
    }

    let legacy = r#"{"source":"C:\\a","target":"C:\\b"}"#;
    let restored: SyncJob = serde_json::from_str(legacy).expect("링크 처리 모드 없는 구버전");
    assert_eq!(restored.symlink_mode, SymlinkMode::Skip);
}

/// 새로 만든 작업들은 서로 다른 ID를 갖는다.
#[test]
fn new_sync_jobs_have_unique_ids() {
    let a = SyncJob::default();
    let b = SyncJob::default();
    assert_ne!(a.id, b.id);
    assert!(a.exclude_patterns.is_empty());
    assert!(b.exclude_patterns.is_empty());
}

/// 제외 패턴은 설정 파일에 저장·복원할 수 있어야 한다.
#[test]
fn sync_job_exclude_patterns_round_trip() {
    let job = SyncJob {
        exclude_patterns: vec!["**/*.tmp".to_string(), "cache/?".to_string()],
        ..SyncJob::default()
    };

    let json = serde_json::to_string(&job).expect("SyncJob 직렬화");
    let restored: SyncJob = serde_json::from_str(&json).expect("SyncJob 역직렬화");

    assert_eq!(restored.exclude_patterns, job.exclude_patterns);
}

/// UI 스냅샷을 저장해도 엔진이 기록한 실행 진행 상황은 살아남아야 한다.
#[test]
fn saving_ui_snapshot_keeps_engine_progress() {
    let stored = vec![SyncJob {
        id: "job-1".to_string(),
        source: r"D:\a".to_string(),
        last_run_unix: Some(1_700_000_000),
        resume_cursor: Some(r"nested\file.txt".to_string()),
        ..SyncJob::default()
    }];

    // UI는 진행 상황을 모른 채(=None) 이름만 바꿔서 저장한다.
    let mut incoming = vec![SyncJob {
        id: "job-1".to_string(),
        name: "이름 변경".to_string(),
        source: r"D:\a".to_string(),
        ..SyncJob::default()
    }];
    carry_over_engine_progress(&stored, &mut incoming);

    assert_eq!(
        incoming[0].name, "이름 변경",
        "사용자 수정은 그대로 반영된다"
    );
    assert_eq!(incoming[0].last_run_unix, Some(1_700_000_000));
    assert_eq!(
        incoming[0].resume_cursor.as_deref(),
        Some(r"nested\file.txt"),
        "이어서 시작할 지점이 사라지면 재시작마다 처음부터 다시 돈다"
    );

    // 저장본에 없던 새 작업은 건드리지 않는다.
    let mut fresh = vec![SyncJob {
        id: "job-2".to_string(),
        ..SyncJob::default()
    }];
    carry_over_engine_progress(&stored, &mut fresh);
    assert!(fresh[0].resume_cursor.is_none());
    assert!(fresh[0].last_run_unix.is_none());
}

#[test]
fn update_config_preserves_unedited_fields() {
    let _data_dir = IsolatedDataDir::new("preserves-fields");
    let mut initial = AppConfig {
        service_enabled: false,
        ..AppConfig::default()
    };
    initial.targets[0].display_name = "Test target".to_string();
    initial.favorite_services = vec!["TestService".to_string()];
    initial.sync_jobs = vec![SyncJob {
        source: r"D:\source".to_string(),
        target: r"D:\target".to_string(),
        last_run_unix: Some(1_700_000_000),
        resume_cursor: Some("nested/file.txt".to_string()),
        ..SyncJob::default()
    }];
    initial.sidebar_width = 320.0;
    initial.virtual_disk_suppressed_issue_keys = vec!["지원 불가:.hidden.sys".to_string()];
    initial.log.max_files = 17;
    save_config(&initial).expect("save initial config");

    let updated = update_config(|config| config.scan_interval_secs = 45).expect("update config");

    assert_eq!(updated.scan_interval_secs, 45);
    assert!(!updated.service_enabled);
    assert_eq!(updated.targets[0].display_name, "Test target");
    assert_eq!(updated.favorite_services, vec!["TestService"]);
    assert_eq!(updated.sync_jobs[0].last_run_unix, Some(1_700_000_000));
    assert_eq!(updated.sidebar_width, 320.0);
    assert_eq!(
        updated.virtual_disk_suppressed_issue_keys,
        vec!["지원 불가:.hidden.sys"]
    );
    assert_eq!(
        updated.sync_jobs[0].resume_cursor.as_deref(),
        Some("nested/file.txt")
    );
    assert_eq!(updated.log.max_files, 17);

    let persisted = load_config()
        .expect("load updated config")
        .expect("updated config exists");
    assert_eq!(persisted.scan_interval_secs, 45);
    assert_eq!(persisted.sync_jobs[0].source, r"D:\source");
    assert_eq!(persisted.sync_jobs[0].target, r"D:\target");
    assert_eq!(persisted.sidebar_width, 320.0);
    assert_eq!(
        persisted.virtual_disk_suppressed_issue_keys,
        vec!["지원 불가:.hidden.sys"]
    );
}

/// `label()`은 이름이 비었을 때 원본 폴더명으로 대체된다.
#[test]
fn label_falls_back_to_source_folder_name() {
    let job = SyncJob {
        source: r"D:\작업\원본폴더".to_string(),
        ..SyncJob::default()
    };
    assert_eq!(job.label(), "원본폴더");

    let named = SyncJob {
        name: "내 백업".to_string(),
        source: r"D:\작업\원본폴더".to_string(),
        ..SyncJob::default()
    };
    assert_eq!(named.label(), "내 백업");
}
