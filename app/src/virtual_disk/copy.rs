//! 게스트 경로를 호스트 대상 경로로 변환하는 안전성 정책.
//!
//! 게스트 파일시스템에서 받은 상대 경로를 호스트 파일시스템에 매핑하고, 선택한
//! 파일·폴더를 청크 단위로 복사하는 안전 경계를 제공한다.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use super::{
    GuestFileEntry, GuestFileKind, GuestFileSource, GuestPath, IoOperation, VirtualDiskError,
};

pub use super::issues::{CopyIssue, CopyIssueLog};
pub use super::metadata::{MetadataFailure, MetadataPolicy};
pub use super::path_policy::HostPathPolicy;

/// 대상 파일이 이미 있을 때 적용하는 복사 정책.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollisionPolicy {
    /// 기존 대상 항목은 읽지 않고 건너뛴다.
    #[default]
    Skip,
    /// 기존 파일을 교체한다. 대상 디렉터리와 파일 종류가 충돌하면 안전을 위해 실패한다.
    Overwrite,
    /// 같은 폴더에 `name (1).ext`, `name (2).ext` 순서로 새 이름을 선택한다.
    Rename,
}

/// 한 번의 게스트 파일·폴더 복사 결과.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CopyReport {
    pub cancelled: bool,
    pub copied_files: u64,
    pub copied_bytes: u64,
    pub skipped_entries: u64,
    pub renamed_entries: u64,
    pub created_directories: u64,
    pub metadata_failures: Vec<MetadataFailure>,
    pub issues: Vec<CopyIssue>,
}

impl CopyReport {
    pub(crate) fn merge(&mut self, other: Self) {
        self.cancelled |= other.cancelled;
        self.copied_files += other.copied_files;
        self.copied_bytes += other.copied_bytes;
        self.skipped_entries += other.skipped_entries;
        self.renamed_entries += other.renamed_entries;
        self.created_directories += other.created_directories;
        self.metadata_failures.extend(other.metadata_failures);
        self.issues.extend(other.issues);
    }
}

/// VDE-009 경로 정책과 VDE-010 복사 정책을 결합한 파일 복사기.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestCopyEngine {
    path_policy: HostPathPolicy,
    collision_policy: CollisionPolicy,
    chunk_bytes: usize,
    metadata_policy: MetadataPolicy,
}

impl GuestCopyEngine {
    pub fn new(
        path_policy: HostPathPolicy,
        collision_policy: CollisionPolicy,
        chunk_bytes: usize,
    ) -> Result<Self, VirtualDiskError> {
        if chunk_bytes == 0 {
            return Err(VirtualDiskError::InvalidEntryKind(
                "복사 청크 크기는 0보다 커야 합니다".to_string(),
            ));
        }

        Ok(Self {
            path_policy,
            collision_policy,
            chunk_bytes,
            metadata_policy: MetadataPolicy::default(),
        })
    }

    pub fn path_policy(&self) -> &HostPathPolicy {
        &self.path_policy
    }

    pub const fn collision_policy(&self) -> CollisionPolicy {
        self.collision_policy
    }

    pub const fn chunk_bytes(&self) -> usize {
        self.chunk_bytes
    }

    pub const fn metadata_policy(&self) -> MetadataPolicy {
        self.metadata_policy
    }

    pub const fn with_metadata_policy(mut self, metadata_policy: MetadataPolicy) -> Self {
        self.metadata_policy = metadata_policy;
        self
    }

    /// 선택한 파일 또는 폴더를 대상 루트 아래에 복사한다.
    pub fn copy_entry<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
    ) -> Result<CopyReport, VirtualDiskError> {
        let destination = self.path_policy.map_entry(entry)?;

        match entry.kind {
            GuestFileKind::File => self.copy_file(source, entry, &destination, None),
            GuestFileKind::Directory => self.copy_directory(source, entry, &destination, None),
        }
    }

    /// 복사 중 항목별 오류를 수집하고 가능한 형제 항목은 계속 처리한다.
    ///
    /// UI·로그 계층은 반환된 `CopyReport::issues`를 `CopyIssueLog::record`에 전달해
    /// 토스트를 한 번만 보여주거나, 사용자가 확인한 키를 억제할 수 있다.
    pub fn copy_entry_collecting<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
        issue_log: &mut CopyIssueLog,
    ) -> CopyReport {
        self.copy_entry_collecting_controlled(source, entry, issue_log, None)
    }

    /// 복사 중 청크 단위 중지를 지원하면서 항목별 오류를 수집한다.
    ///
    /// `cancel`이 설정되면 현재 파일의 부분 결과를 제거하고, 상위 디렉터리와
    /// 형제 항목 순회를 더 진행하지 않는다. 이미 완료된 항목의 결과는 유지한다.
    pub fn copy_entry_collecting_with_cancel<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
        issue_log: &mut CopyIssueLog,
        cancel: &AtomicBool,
    ) -> CopyReport {
        self.copy_entry_collecting_controlled(source, entry, issue_log, Some(cancel))
    }

    fn copy_entry_collecting_controlled<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
        issue_log: &mut CopyIssueLog,
        cancel: Option<&AtomicBool>,
    ) -> CopyReport {
        let mut report = CopyReport::default();
        if is_cancelled(cancel) {
            report.cancelled = true;
            return report;
        }
        let destination = match self.path_policy.map_entry(entry) {
            Ok(destination) => destination,
            Err(error) => {
                self.record_issue(&mut report, issue_log, &entry.path, &error);
                return report;
            }
        };

        match entry.kind {
            GuestFileKind::File => match self.copy_file(source, entry, &destination, cancel) {
                Ok(child_report) => {
                    self.merge_collected_report(&mut report, child_report, issue_log)
                }
                Err(error) => self.record_issue(&mut report, issue_log, &entry.path, &error),
            },
            GuestFileKind::Directory => self.copy_directory_collecting(
                source,
                entry,
                &destination,
                &mut report,
                issue_log,
                cancel,
            ),
        }
        report
    }

    fn copy_directory_collecting<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
        destination: &Path,
        report: &mut CopyReport,
        issue_log: &mut CopyIssueLog,
        cancel: Option<&AtomicBool>,
    ) {
        if is_cancelled(cancel) {
            report.cancelled = true;
            return;
        }
        let destination = match self.prepare_directory(destination, report) {
            Ok(Some(destination)) => destination,
            Ok(None) => return,
            Err(error) => {
                self.record_issue(report, issue_log, &entry.path, &error);
                return;
            }
        };
        let children = match source.list_directory(entry) {
            Ok(children) => children,
            Err(error) => {
                self.record_issue(report, issue_log, &entry.path, &error);
                return;
            }
        };

        for child in children {
            if is_cancelled(cancel) {
                report.cancelled = true;
                return;
            }
            if let Err(error) = self.path_policy.map_entry(&child) {
                self.record_issue(report, issue_log, &child.path, &error);
                continue;
            }
            let child_destination = match self.child_destination(&destination, &entry.path, &child)
            {
                Ok(destination) => destination,
                Err(error) => {
                    self.record_issue(report, issue_log, &child.path, &error);
                    continue;
                }
            };
            match child.kind {
                GuestFileKind::File => {
                    match self.copy_file(source, &child, &child_destination, cancel) {
                        Ok(child_report) => {
                            let cancelled = child_report.cancelled;
                            self.merge_collected_report(report, child_report, issue_log);
                            if cancelled {
                                return;
                            }
                        }
                        Err(error) => self.record_issue(report, issue_log, &child.path, &error),
                    }
                }
                GuestFileKind::Directory => {
                    self.copy_directory_collecting(
                        source,
                        &child,
                        &child_destination,
                        report,
                        issue_log,
                        cancel,
                    );
                    if report.cancelled {
                        return;
                    }
                }
            }
        }

        if let Err(error) = self.path_policy.validate_target_path(&destination) {
            self.record_issue(report, issue_log, &entry.path, &error);
            return;
        }
        let issue_start = report.issues.len();
        self.append_metadata(report, &destination, entry);
        for issue in &report.issues[issue_start..] {
            issue_log.record(issue.clone());
        }
    }

    fn merge_collected_report(
        &self,
        report: &mut CopyReport,
        child_report: CopyReport,
        issue_log: &mut CopyIssueLog,
    ) {
        for issue in &child_report.issues {
            issue_log.record(issue.clone());
        }
        report.merge(child_report);
    }

    fn record_issue(
        &self,
        report: &mut CopyReport,
        issue_log: &mut CopyIssueLog,
        path: &GuestPath,
        error: &VirtualDiskError,
    ) {
        let issue = CopyIssue::from_error(path, error);
        issue_log.record(issue.clone());
        report.issues.push(issue);
    }

    fn append_metadata(&self, report: &mut CopyReport, destination: &Path, entry: &GuestFileEntry) {
        let failures = super::metadata::apply_metadata(destination, entry, self.metadata_policy);
        for failure in &failures {
            report
                .issues
                .push(CopyIssue::from_metadata_failure(failure));
        }
        report.metadata_failures.extend(failures);
    }

    fn copy_directory<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
        destination: &Path,
        cancel: Option<&AtomicBool>,
    ) -> Result<CopyReport, VirtualDiskError> {
        let mut report = CopyReport::default();
        if is_cancelled(cancel) {
            report.cancelled = true;
            return Ok(report);
        }
        let Some(destination) = self.prepare_directory(destination, &mut report)? else {
            return Ok(report);
        };

        let children = source
            .list_directory(entry)
            .map_err(|error| error.with_guest_path(&entry.path))?;
        for child in children {
            if is_cancelled(cancel) {
                report.cancelled = true;
                return Ok(report);
            }
            // 먼저 원래 게스트 경로의 안전성을 검사한 다음, 부모 디렉터리가 충돌로
            // 이름을 바꿨다면 자식도 그 새 대상 아래에 배치한다.
            self.path_policy.map_entry(&child)?;
            let child_destination = self.child_destination(&destination, &entry.path, &child)?;
            let child_report = match child.kind {
                GuestFileKind::File => {
                    self.copy_file(source, &child, &child_destination, cancel)?
                }
                GuestFileKind::Directory => {
                    self.copy_directory(source, &child, &child_destination, cancel)?
                }
            };
            let cancelled = child_report.cancelled;
            report.merge(child_report);
            if cancelled {
                return Ok(report);
            }
        }

        self.path_policy.validate_target_path(&destination)?;
        self.append_metadata(&mut report, &destination, entry);
        Ok(report)
    }

    fn child_destination(
        &self,
        destination: &Path,
        parent_path: &GuestPath,
        child: &GuestFileEntry,
    ) -> Result<PathBuf, VirtualDiskError> {
        let relative = if parent_path.is_root() {
            child.path.as_str()
        } else {
            child
                .path
                .as_str()
                .strip_prefix(parent_path.as_str())
                .and_then(|suffix| suffix.strip_prefix('/'))
                .ok_or_else(|| VirtualDiskError::UnsafePath {
                    path: child.path.to_string(),
                    detail: format!(
                        "게스트 자식 경로가 부모 경로 '{}' 아래에 있지 않습니다",
                        parent_path
                    ),
                })?
        };

        let mut child_destination = destination.to_path_buf();
        for component in relative
            .split('/')
            .filter(|component| !component.is_empty())
        {
            child_destination.push(component);
        }
        self.path_policy.validate_target_path(&child_destination)?;
        Ok(child_destination)
    }

    fn copy_file<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
        destination: &Path,
        cancel: Option<&AtomicBool>,
    ) -> Result<CopyReport, VirtualDiskError> {
        let mut report = CopyReport::default();
        if is_cancelled(cancel) {
            report.cancelled = true;
            return Ok(report);
        }
        let Some(destination) = self.prepare_file(destination, &mut report)? else {
            return Ok(report);
        };
        self.ensure_parent_directory(&destination, &mut report)?;

        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|source| VirtualDiskError::Io {
                operation: IoOperation::Open,
                source,
            })?;

        let mut offset = 0u64;
        let mut buffer = vec![0u8; self.chunk_bytes];
        while offset < entry.size_bytes {
            if is_cancelled(cancel) {
                drop(output);
                Self::remove_cancelled_file(&destination)?;
                report.cancelled = true;
                return Ok(report);
            }
            let remaining = entry.size_bytes - offset;
            let requested = remaining.min(buffer.len() as u64) as usize;
            let read = source
                .read_at(entry, offset, &mut buffer[..requested])
                .map_err(|error| error.with_guest_path(&entry.path))?;
            if read == 0 || read < requested {
                return Err(VirtualDiskError::SourceChanged(format!(
                    "게스트 파일 '{}'을 읽는 중 예상보다 일찍 끝났습니다 (offset={}, requested={}, read={})",
                    entry.path, offset, requested, read
                )));
            }

            if is_cancelled(cancel) {
                drop(output);
                Self::remove_cancelled_file(&destination)?;
                report.cancelled = true;
                return Ok(report);
            }

            output
                .write_all(&buffer[..read])
                .map_err(|source| VirtualDiskError::Io {
                    operation: IoOperation::Write,
                    source,
                })?;
            offset = offset.checked_add(read as u64).ok_or({
                VirtualDiskError::BoundsViolation {
                    offset,
                    length: read as u64,
                    capacity: entry.size_bytes,
                }
            })?;
        }

        output.flush().map_err(|source| VirtualDiskError::Io {
            operation: IoOperation::Write,
            source,
        })?;
        drop(output);
        self.append_metadata(&mut report, &destination, entry);
        report.copied_files = 1;
        report.copied_bytes = entry.size_bytes;
        Ok(report)
    }

    fn remove_cancelled_file(path: &Path) -> Result<(), VirtualDiskError> {
        fs::remove_file(path).map_err(|source| VirtualDiskError::Io {
            operation: IoOperation::RemoveFile,
            source,
        })
    }

    fn ensure_parent_directory(
        &self,
        destination: &Path,
        report: &mut CopyReport,
    ) -> Result<(), VirtualDiskError> {
        let Some(parent) = destination.parent() else {
            return Ok(());
        };
        self.path_policy.validate_target_path(parent)?;
        let Some(metadata) = existing_metadata(parent)? else {
            fs::create_dir_all(parent).map_err(|source| VirtualDiskError::Io {
                operation: IoOperation::CreateDirectory,
                source,
            })?;
            self.path_policy.validate_target_path(parent)?;
            report.created_directories += 1;
            return Ok(());
        };
        if !metadata.is_dir() {
            return Err(VirtualDiskError::InvalidEntryKind(format!(
                "파일 대상의 부모 경로가 디렉터리가 아닙니다: {}",
                parent.display()
            )));
        }
        Ok(())
    }

    fn prepare_directory(
        &self,
        destination: &Path,
        report: &mut CopyReport,
    ) -> Result<Option<PathBuf>, VirtualDiskError> {
        self.path_policy.validate_target_path(destination)?;
        let Some(metadata) = existing_metadata(destination)? else {
            fs::create_dir_all(destination).map_err(|source| VirtualDiskError::Io {
                operation: IoOperation::CreateDirectory,
                source,
            })?;
            self.path_policy.validate_target_path(destination)?;
            report.created_directories += 1;
            return Ok(Some(destination.to_path_buf()));
        };

        if metadata.is_dir() {
            return Ok(Some(destination.to_path_buf()));
        }

        match self.collision_policy {
            CollisionPolicy::Skip => {
                report.skipped_entries += 1;
                Ok(None)
            }
            CollisionPolicy::Overwrite => Err(VirtualDiskError::InvalidEntryKind(format!(
                "게스트 디렉터리 '{}'와 호스트 파일이 충돌해 덮어쓸 수 없습니다",
                destination.display()
            ))),
            CollisionPolicy::Rename => {
                let renamed = self.next_collision_path(destination)?;
                fs::create_dir_all(&renamed).map_err(|source| VirtualDiskError::Io {
                    operation: IoOperation::CreateDirectory,
                    source,
                })?;
                self.path_policy.validate_target_path(&renamed)?;
                report.renamed_entries += 1;
                report.created_directories += 1;
                Ok(Some(renamed))
            }
        }
    }

    fn prepare_file(
        &self,
        destination: &Path,
        report: &mut CopyReport,
    ) -> Result<Option<PathBuf>, VirtualDiskError> {
        self.path_policy.validate_target_path(destination)?;
        let Some(metadata) = existing_metadata(destination)? else {
            return Ok(Some(destination.to_path_buf()));
        };

        if metadata.is_dir() {
            return match self.collision_policy {
                CollisionPolicy::Skip => {
                    report.skipped_entries += 1;
                    Ok(None)
                }
                CollisionPolicy::Overwrite => Err(VirtualDiskError::InvalidEntryKind(format!(
                    "게스트 파일 '{}'와 호스트 디렉터리가 충돌해 덮어쓸 수 없습니다",
                    destination.display()
                ))),
                CollisionPolicy::Rename => {
                    let renamed = self.next_collision_path(destination)?;
                    report.renamed_entries += 1;
                    Ok(Some(renamed))
                }
            };
        }

        match self.collision_policy {
            CollisionPolicy::Skip => {
                report.skipped_entries += 1;
                Ok(None)
            }
            CollisionPolicy::Overwrite => {
                fs::remove_file(destination).map_err(|source| VirtualDiskError::Io {
                    operation: IoOperation::Write,
                    source,
                })?;
                Ok(Some(destination.to_path_buf()))
            }
            CollisionPolicy::Rename => {
                let renamed = self.next_collision_path(destination)?;
                report.renamed_entries += 1;
                Ok(Some(renamed))
            }
        }
    }

    fn next_collision_path(&self, destination: &Path) -> Result<PathBuf, VirtualDiskError> {
        for index in 1u64.. {
            let candidate = collision_path(destination, index);
            self.path_policy.validate_target_path(&candidate)?;
            if existing_metadata(&candidate)?.is_none() {
                return Ok(candidate);
            }
        }

        Err(VirtualDiskError::PathTooLong {
            path: destination.display().to_string(),
            length: usize::MAX,
            max: self.path_policy.max_path_units(),
        })
    }
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
}

fn existing_metadata(path: &Path) -> Result<Option<fs::Metadata>, VirtualDiskError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(VirtualDiskError::Io {
            operation: IoOperation::Open,
            source,
        }),
    }
}

fn collision_path(destination: &Path, index: u64) -> PathBuf {
    let stem = destination
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "copy".to_string());
    let extension = destination
        .extension()
        .map(|value| value.to_string_lossy().into_owned());
    let file_name = match extension {
        Some(extension) if !extension.is_empty() => format!("{stem} ({index}).{extension}"),
        _ => format!("{stem} ({index})"),
    };
    destination.with_file_name(file_name)
}

#[cfg(test)]
mod tests;
