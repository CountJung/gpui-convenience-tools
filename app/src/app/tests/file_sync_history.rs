use super::*;
use crate::sync_history::SyncHistoryEntry;

#[gpui::test]
fn file_sync_renders_recent_history_with_result_counts_and_duration(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync.history = [(true, 1), (false, 1), (false, 0)]
            .into_iter()
            .enumerate()
            .map(|(index, (cancelled, failed))| SyncHistoryEntry {
                job_id: format!("history-job-{index}"),
                label: format!("백업 작업 {index}"),
                started_at_unix: 100,
                finished_at_unix: 125,
                copied: 3,
                skipped: 2,
                deleted: 1,
                failed,
                cancelled,
                summary: String::new(),
            })
            .collect();
        root
    });

    for (width, height) in [(920.0, 480.0), (994.0, 702.0), (1280.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        refresh(cx);
        let card = cx
            .debug_bounds("file-sync-history-card")
            .expect("history card");
        for selector in [
            "sync-history-row-0",
            "sync-history-row-1",
            "sync-history-row-2",
        ] {
            let row = cx
                .debug_bounds(selector)
                .expect("all three result rows should render");
            assert!(row.origin.x >= card.origin.x);
            assert!(row.origin.x + row.size.width <= card.origin.x + card.size.width + px(1.0));
        }

        if height == 480.0 {
            let max_scroll =
                cx.update(|_, app| view.read(app).sync.page_scroll.max_offset().height);
            assert!(
                max_scroll > px(0.0),
                "history should overflow the compact page"
            );
            wheel_to_end(cx, "file-sync-page", -10000.0);
            let offset = cx.update(|_, app| view.read(app).sync.page_scroll.offset().y);
            assert!(offset < px(0.0), "wheel input should scroll toward history");
            assert_inside_viewport(cx, "file-sync-page", "sync-history-row-2");
        }
    }
}
