//! 파일 동기화 실행 이력의 저장 경계.
//!
//! 실행 스레드는 이 모듈을 통해서만 이력을 추가한다. UI 설정 스냅샷과 분리된 JSON 배열로
//! 저장해 앱을 다시 시작해도 최근 실행 결과를 읽을 수 있는 기반을 마련한다.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

use crate::{config, sync::SyncOutcome};

const HISTORY_FILE_NAME: &str = "sync-history.json";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct SyncHistoryEntry {
    pub(crate) job_id: String,
    pub(crate) label: String,
    pub(crate) started_at_unix: u64,
    pub(crate) finished_at_unix: u64,
    pub(crate) copied: usize,
    pub(crate) skipped: usize,
    pub(crate) deleted: usize,
    pub(crate) failed: usize,
    pub(crate) cancelled: bool,
    pub(crate) summary: String,
}

impl SyncHistoryEntry {
    pub(crate) fn from_outcome(
        job_id: &str,
        label: &str,
        started_at_unix: u64,
        finished_at_unix: u64,
        outcome: &SyncOutcome,
    ) -> Self {
        Self {
            job_id: job_id.to_string(),
            label: label.to_string(),
            started_at_unix,
            finished_at_unix,
            copied: outcome.copied,
            skipped: outcome.skipped,
            deleted: outcome.deleted,
            failed: outcome.failures.len() + outcome.truncated_failures,
            cancelled: outcome.cancelled,
            summary: outcome.summary(),
        }
    }
}

pub(crate) fn history_path() -> PathBuf {
    config::data_dir().join(HISTORY_FILE_NAME)
}

pub(crate) fn append(entry: &SyncHistoryEntry) -> Result<()> {
    append_to_path(&history_path(), entry)
}

fn append_to_path(path: &Path, entry: &SyncHistoryEntry) -> Result<()> {
    let mut entries = if path.exists() {
        let data = fs::read_to_string(path)
            .with_context(|| format!("동기화 이력 읽기 실패: {}", path.display()))?;
        if data.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str::<Vec<SyncHistoryEntry>>(&data)
                .with_context(|| format!("동기화 이력 JSON 파싱 실패: {}", path.display()))?
        }
    } else {
        Vec::new()
    };

    entries.push(entry.clone());
    let json = serde_json::to_string_pretty(&entries).context("동기화 이력 JSON 직렬화 실패")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("동기화 이력 폴더 생성 실패: {}", parent.display()))?;
    }
    fs::write(path, json).with_context(|| format!("동기화 이력 저장 실패: {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_path() -> PathBuf {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let id = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "gct-sync-history-{}-{id}.json",
            std::process::id()
        ))
    }

    fn entry(job_id: &str, copied: usize) -> SyncHistoryEntry {
        SyncHistoryEntry {
            job_id: job_id.to_string(),
            label: format!("작업 {job_id}"),
            started_at_unix: 100,
            finished_at_unix: 110,
            copied,
            skipped: 2,
            deleted: 1,
            failed: 0,
            cancelled: false,
            summary: format!("복사 {copied}건"),
        }
    }

    #[test]
    fn appends_history_entries_in_execution_order() {
        let path = temp_path();
        append_to_path(&path, &entry("first", 1)).expect("append first history entry");
        append_to_path(&path, &entry("second", 3)).expect("append second history entry");

        let entries: Vec<SyncHistoryEntry> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].job_id, "first");
        assert_eq!(entries[1].job_id, "second");
        assert_eq!(entries[1].copied, 3);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_malformed_history_without_overwriting_it() {
        let path = temp_path();
        fs::write(&path, b"not-json").unwrap();

        let error = append_to_path(&path, &entry("broken", 1)).unwrap_err();

        assert!(error.to_string().contains("동기화 이력 JSON 파싱 실패"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "not-json");
        let _ = fs::remove_file(path);
    }
}
