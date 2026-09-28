use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, path::PathBuf};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn browse_fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "gct-gpui-sync-browse-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    let source = root.join("source");
    fs::create_dir_all(source.join(".hidden-folder")).unwrap();
    fs::write(source.join(".hidden-folder/inside.txt"), b"inside").unwrap();
    fs::write(source.join("visible.txt"), b"visible").unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM,
        };
        let wide: Vec<_> = source
            .join(".hidden-folder")
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: null 종단된 유효한 경로 포인터를 동기 호출 동안 유지한다.
        let result = unsafe {
            SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM)
        };
        assert_ne!(result, 0, "fixture hidden/system attributes");
    }
    (root, source)
}

#[gpui::test]
fn file_sync_browse_exposes_hidden_folders_and_saves_selected_exclusions(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (fixture, source) = browse_fixture();
    let source_text = source.display().to_string();
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync.jobs = vec![SyncJob {
            source: source_text,
            target: fixture.join("target").display().to_string(),
            ..SyncJob::default()
        }];
        root.sync.selected_job = Some(0);
        root
    });

    for width in [MIN_SUPPORTED_WINDOW_WIDTH, 1280.0] {
        cx.simulate_resize(size(px(width), px(DEFAULT_WINDOW_HEIGHT)));
        refresh(cx);
        assert!(cx.debug_bounds("sync-browse-card").is_some());
        assert!(cx.debug_bounds("sync-browse-row-0").is_some());
        cx.update(|_, app| {
            let root = view.read(app);
            assert!(root
                .sync
                .browse_entries
                .iter()
                .any(|entry| entry.name == ".hidden-folder"
                    && entry.is_directory
                    && entry.is_hidden_or_system));
        });
    }

    cx.simulate_resize(size(px(1280.0), px(1600.0)));
    refresh(cx);
    wheel_to_end(cx, "file-sync-page", -10000.0);
    click_debug_element(cx, "sync-browse-toggle-0");
    click_debug_element(cx, "sync-browse-enter-0");
    cx.update(|_, app| {
        let root = view.read(app);
        assert_eq!(root.sync.jobs[0].exclude_patterns, vec![".hidden-folder"]);
        assert_eq!(root.sync.browse_relative, PathBuf::from(".hidden-folder"));
        assert!(root
            .sync
            .browse_entries
            .iter()
            .any(|entry| entry.name == "inside.txt"));
    });
    let _ = fs::remove_dir_all(fixture);
}

#[gpui::test]
fn file_sync_link_mode_choices_update_the_selected_job(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync.jobs = vec![SyncJob::default()];
        root.sync.selected_job = Some(0);
        root
    });
    cx.simulate_resize(size(px(1280.0), px(1800.0)));
    refresh(cx);
    assert!(cx.debug_bounds("sync-link-skip").is_some());
    click_debug_element(cx, "sync-link-follow");
    cx.update(|_, app| {
        assert_eq!(
            view.read(app).sync.jobs[0].symlink_mode,
            crate::config::SymlinkMode::Follow
        );
    });
    click_debug_element(cx, "sync-link-recreate");
    cx.update(|_, app| {
        assert_eq!(
            view.read(app).sync.jobs[0].symlink_mode,
            crate::config::SymlinkMode::Recreate
        );
    });
}

#[gpui::test]
fn file_sync_source_edit_invalidates_old_exclusion_selection(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (fixture, source) = browse_fixture();
    let next_source = fixture.join("another-source");
    fs::create_dir_all(&next_source).unwrap();
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync.jobs = vec![SyncJob {
            source: source.display().to_string(),
            ..SyncJob::default()
        }];
        root.sync.selected_job = Some(0);
        root
    });
    cx.simulate_resize(size(px(1280.0), px(1600.0)));
    refresh(cx);
    wheel_to_end(cx, "file-sync-page", -10000.0);
    cx.update(|window, app| {
        let input = view.read(app).sync.source_input.clone().unwrap();
        input.update(app, |state, cx| {
            state.set_value(next_source.display().to_string(), window, cx)
        });
    });
    refresh(cx);
    click_debug_element(cx, "sync-browse-toggle-0");
    cx.update(|_, app| {
        let root = view.read(app);
        assert!(root.sync.jobs[0].exclude_patterns.is_empty());
        assert_eq!(root.sync.jobs[0].source, source.display().to_string());
        assert_eq!(root.sync.browse_source, next_source.display().to_string());
    });
    let _ = fs::remove_dir_all(fixture);
}
