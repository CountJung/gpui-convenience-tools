//! 최근 동기화 이력 카드와 화면 문구.

use super::*;
use crate::sync_history::SyncHistoryEntry;

#[derive(Debug, PartialEq, Eq)]
struct HistoryRow {
    status: &'static str,
    counters: String,
    duration: String,
}

fn present(entry: &SyncHistoryEntry) -> HistoryRow {
    HistoryRow {
        status: if entry.cancelled {
            "중지"
        } else if entry.failed > 0 {
            "실패 포함"
        } else {
            "성공"
        },
        counters: format!(
            "복사 {} · 건너뜀 {} · 삭제 {} · 실패 {}",
            entry.copied, entry.skipped, entry.deleted, entry.failed,
        ),
        duration: format!(
            "소요 {}",
            format_interval(entry.duration_secs().min(u32::MAX as u64) as u32)
        ),
    }
}

pub(super) fn render(this: &AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;
    let mut rows = v_flex().gap_1();

    if this.sync.history.is_empty() {
        rows = rows.child(
            div()
                .text_color(muted_fg)
                .child("아직 실행된 동기화 이력이 없습니다."),
        );
    } else {
        for (index, entry) in this.sync.history.iter().take(20).enumerate() {
            let display = present(entry);
            let tone = if entry.cancelled {
                ui::Tone::Info
            } else if entry.failed > 0 {
                ui::Tone::Warning
            } else {
                ui::Tone::Success
            };
            rows = rows.child(
                h_flex()
                    .debug_selector(move || format!("sync-history-row-{index}"))
                    .w_full()
                    .gap_2()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme.list)
                    .child(ui::badge(display.status, tone, ui::Size::Sm, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_color(fg).child(entry.label.clone()))
                            .child(div().text_color(muted_fg).child(display.counters)),
                    )
                    .child(div().text_color(muted_fg).child(display.duration)),
            );
        }
    }

    div()
        .debug_selector(|| "file-sync-history-card".to_string())
        .rounded_lg()
        .bg(card)
        .border_1()
        .border_color(border)
        .p_3()
        .child(
            v_flex()
                .gap_2()
                .child(div().text_color(fg).child("최근 실행 이력"))
                .child(rows),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(cancelled: bool, failed: usize) -> SyncHistoryEntry {
        SyncHistoryEntry {
            job_id: "job".into(),
            label: "백업".into(),
            started_at_unix: 100,
            finished_at_unix: 125,
            copied: 3,
            skipped: 2,
            deleted: 1,
            failed,
            cancelled,
            summary: String::new(),
        }
    }

    #[test]
    fn presents_cancelled_failed_and_success_with_counts_and_duration() {
        let cancelled = present(&entry(true, 1));
        let failed = present(&entry(false, 1));
        let success = present(&entry(false, 0));
        assert_eq!(cancelled.status, "중지");
        assert_eq!(failed.status, "실패 포함");
        assert_eq!(success.status, "성공");
        assert_eq!(failed.counters, "복사 3 · 건너뜀 2 · 삭제 1 · 실패 1");
        assert_eq!(success.counters, "복사 3 · 건너뜀 2 · 삭제 1 · 실패 0");
        assert_eq!(success.duration, "소요 25초");
    }
}
