//! VirtualBox 복사 작업의 항목별 오류와 알림 억제 상태.

use super::{metadata::MetadataFailure, GuestPath, VirtualDiskError};

/// 복사 실패를 UI 알림과 로그에서 분류하기 위한 종류.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CopyIssueKind {
    SourceChanged,
    Destination,
    Unsupported,
    Metadata,
}

impl std::fmt::Display for CopyIssueKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::SourceChanged => "원본 변경",
            Self::Destination => "대상 쓰기",
            Self::Unsupported => "지원 불가",
            Self::Metadata => "메타데이터",
        })
    }
}

/// 한 게스트 항목에 대한 복사·메타데이터 실패.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyIssue {
    pub path: String,
    pub kind: CopyIssueKind,
    pub detail: String,
}

impl CopyIssue {
    pub fn from_error(path: &GuestPath, error: &VirtualDiskError) -> Self {
        Self {
            path: path.to_string(),
            kind: classify_error(error),
            detail: error.to_string(),
        }
    }

    pub fn from_metadata_failure(failure: &MetadataFailure) -> Self {
        Self {
            path: failure.path.clone(),
            kind: CopyIssueKind::Metadata,
            detail: format!("{} 적용 실패: {}", failure.operation, failure.detail),
        }
    }

    /// 경로와 종류만으로 안정적인 억제 키를 만든다. 세부 오류 문구는 OS에 따라 달라질 수
    /// 있으므로 키에 포함하지 않는다.
    pub fn key(&self) -> String {
        format!("{}:{}", self.kind, self.path)
    }
}

fn classify_error(error: &VirtualDiskError) -> CopyIssueKind {
    match error {
        VirtualDiskError::SourceChanged(_) => CopyIssueKind::SourceChanged,
        VirtualDiskError::UnsupportedFormat { .. } => CopyIssueKind::Unsupported,
        VirtualDiskError::GuestEntry { source, .. } => classify_error(source),
        VirtualDiskError::CorruptImage(_) => CopyIssueKind::SourceChanged,
        _ => CopyIssueKind::Destination,
    }
}

/// 항목별 오류를 보관하고 사용자가 확인한 키의 반복 알림을 억제한다.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CopyIssueLog {
    issues: Vec<CopyIssue>,
    suppressed_keys: std::collections::BTreeSet<String>,
}

impl CopyIssueLog {
    /// 새 항목이면 보관하고, 현재 알림 억제 상태가 아니면 `true`를 반환한다.
    pub fn record(&mut self, issue: CopyIssue) -> bool {
        let key = issue.key();
        let is_new = if self.issues.iter().any(|existing| existing.key() == key) {
            false
        } else {
            self.issues.push(issue);
            true
        };
        is_new && !self.suppressed_keys.contains(&key)
    }

    pub fn issues(&self) -> &[CopyIssue] {
        &self.issues
    }

    pub fn suppressed_keys(&self) -> &std::collections::BTreeSet<String> {
        &self.suppressed_keys
    }

    pub fn suppress(&mut self, key: impl Into<String>) {
        self.suppressed_keys.insert(key.into());
    }

    pub fn unsuppress(&mut self, key: &str) {
        self.suppressed_keys.remove(key);
    }

    pub fn is_suppressed(&self, key: &str) -> bool {
        self.suppressed_keys.contains(key)
    }

    pub fn clear_issues(&mut self) {
        self.issues.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_one_issue_per_path_and_suppresses_follow_up_notifications() {
        let path = GuestPath::new("folder/file.txt").unwrap();
        let error = VirtualDiskError::SourceChanged("원본이 변경되었습니다".to_string());
        let issue = CopyIssue::from_error(&path, &error);
        let key = issue.key();
        let mut log = CopyIssueLog::default();

        assert!(log.record(issue.clone()));
        assert!(!log.record(issue.clone()));
        log.suppress(&key);
        assert!(!log.record(issue));
        assert_eq!(log.issues().len(), 1);
        assert!(log.is_suppressed(&key));
        log.unsuppress(&key);
        assert!(!log.is_suppressed(&key));
    }

    #[test]
    fn classifies_nested_unsupported_errors_for_stable_issue_keys() {
        let path = GuestPath::new("hidden/archive.zip").unwrap();
        let error = VirtualDiskError::UnsupportedFormat {
            kind: super::super::UnsupportedFormatKind::FileStream,
            detail: "압축 스트림".to_string(),
        };
        let issue = CopyIssue::from_error(&path, &error);

        assert_eq!(issue.kind, CopyIssueKind::Unsupported);
        assert_eq!(issue.key(), "지원 불가:hidden/archive.zip");
    }
}
