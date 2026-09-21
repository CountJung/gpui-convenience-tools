//! 게스트 경로를 호스트 대상 경로로 변환하는 안전성 정책.
//!
//! 게스트 파일시스템에서 받은 상대 경로를 호스트 파일시스템에 매핑하고, 선택한
//! 파일·폴더를 청크 단위로 복사하는 안전 경계를 제공한다.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use super::{
    GuestFileEntry, GuestFileKind, GuestFileSource, GuestPath, IoOperation, VirtualDiskError,
};

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
    pub copied_files: u64,
    pub copied_bytes: u64,
    pub skipped_entries: u64,
    pub renamed_entries: u64,
    pub created_directories: u64,
}

impl CopyReport {
    fn merge(&mut self, other: Self) {
        self.copied_files += other.copied_files;
        self.copied_bytes += other.copied_bytes;
        self.skipped_entries += other.skipped_entries;
        self.renamed_entries += other.renamed_entries;
        self.created_directories += other.created_directories;
    }
}

/// VDE-009 경로 정책과 VDE-010 복사 정책을 결합한 파일 복사기.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestCopyEngine {
    path_policy: HostPathPolicy,
    collision_policy: CollisionPolicy,
    chunk_bytes: usize,
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

    /// 선택한 파일 또는 폴더를 대상 루트 아래에 복사한다.
    pub fn copy_entry<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
    ) -> Result<CopyReport, VirtualDiskError> {
        let destination = self.path_policy.map_entry(entry)?;

        match entry.kind {
            GuestFileKind::File => self.copy_file(source, entry, &destination),
            GuestFileKind::Directory => self.copy_directory(source, entry, &destination),
        }
    }

    fn copy_directory<S: GuestFileSource>(
        &self,
        source: &mut S,
        entry: &GuestFileEntry,
        destination: &Path,
    ) -> Result<CopyReport, VirtualDiskError> {
        let mut report = CopyReport::default();
        let Some(destination) = self.prepare_directory(destination, &mut report)? else {
            return Ok(report);
        };

        let children = source
            .list_directory(entry)
            .map_err(|error| error.with_guest_path(&entry.path))?;
        for child in children {
            // 먼저 원래 게스트 경로의 안전성을 검사한 다음, 부모 디렉터리가 충돌로
            // 이름을 바꿨다면 자식도 그 새 대상 아래에 배치한다.
            self.path_policy.map_entry(&child)?;
            let child_destination = self.child_destination(&destination, &entry.path, &child)?;
            let child_report = match child.kind {
                GuestFileKind::File => self.copy_file(source, &child, &child_destination)?,
                GuestFileKind::Directory => {
                    self.copy_directory(source, &child, &child_destination)?
                }
            };
            report.merge(child_report);
        }

        self.path_policy.validate_target_path(&destination)?;
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
    ) -> Result<CopyReport, VirtualDiskError> {
        let mut report = CopyReport::default();
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
        report.copied_files = 1;
        report.copied_bytes = entry.size_bytes;
        Ok(report)
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
mod tests {
    use std::{
        collections::HashMap,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::virtual_disk::{GuestFileAttributes, GuestFileKind, GuestFileSystem};

    struct FixtureSource {
        files: HashMap<String, Vec<u8>>,
        directories: HashMap<String, Vec<GuestFileEntry>>,
        requested_chunks: Vec<usize>,
    }

    impl GuestFileSource for FixtureSource {
        fn filesystem(&self) -> GuestFileSystem {
            GuestFileSystem::ntfs_3_1()
        }

        fn list_directory(
            &mut self,
            directory: &GuestFileEntry,
        ) -> Result<Vec<GuestFileEntry>, VirtualDiskError> {
            self.directories
                .get(directory.path.as_str())
                .cloned()
                .ok_or_else(|| VirtualDiskError::InvalidGuestPath(directory.path.to_string()))
        }

        fn read_at(
            &mut self,
            file: &GuestFileEntry,
            offset: u64,
            buffer: &mut [u8],
        ) -> Result<usize, VirtualDiskError> {
            self.requested_chunks.push(buffer.len());
            let contents = self
                .files
                .get(file.path.as_str())
                .ok_or_else(|| VirtualDiskError::InvalidGuestPath(file.path.to_string()))?;
            let offset =
                usize::try_from(offset).map_err(|_| VirtualDiskError::BoundsViolation {
                    offset,
                    length: buffer.len() as u64,
                    capacity: contents.len() as u64,
                })?;
            if offset >= contents.len() {
                return Ok(0);
            }

            let length = buffer.len().min(contents.len() - offset);
            buffer[..length].copy_from_slice(&contents[offset..offset + length]);
            Ok(length)
        }
    }

    fn policy(max_path_units: usize) -> HostPathPolicy {
        HostPathPolicy::new(std::env::temp_dir(), max_path_units).unwrap()
    }

    fn directory(path: &str) -> GuestFileEntry {
        GuestFileEntry {
            path: GuestPath::new(path).unwrap(),
            kind: GuestFileKind::Directory,
            size_bytes: 0,
            attributes: GuestFileAttributes::default(),
        }
    }

    fn file(path: &str, size_bytes: usize) -> GuestFileEntry {
        GuestFileEntry {
            path: GuestPath::new(path).unwrap(),
            kind: GuestFileKind::File,
            size_bytes: size_bytes as u64,
            attributes: GuestFileAttributes::default(),
        }
    }

    fn fixture_source() -> (FixtureSource, GuestFileEntry, GuestFileEntry) {
        let directory = directory("folder");
        let file = file("folder/data.txt", 10);
        let source = FixtureSource {
            files: HashMap::from([(file.path.to_string(), b"0123456789".to_vec())]),
            directories: HashMap::from([("folder".to_string(), vec![file.clone()])]),
            requested_chunks: Vec::new(),
        };
        (source, directory, file)
    }

    fn test_destination(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let destination = std::env::temp_dir().join(format!(
            "gpui-convenience-tools-vde010-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&destination).unwrap();
        destination
    }

    fn engine(destination: &Path, collision_policy: CollisionPolicy) -> GuestCopyEngine {
        GuestCopyEngine::new(
            HostPathPolicy::new(destination, 260).unwrap(),
            collision_policy,
            3,
        )
        .unwrap()
    }

    #[test]
    fn copies_directory_files_in_configured_chunks() {
        let destination = test_destination("chunks");
        let (mut source, directory, _) = fixture_source();

        let report = engine(&destination, CollisionPolicy::Skip)
            .copy_entry(&mut source, &directory)
            .unwrap();

        assert_eq!(report.copied_files, 1);
        assert_eq!(report.copied_bytes, 10);
        assert_eq!(source.requested_chunks, vec![3, 3, 3, 1]);
        assert_eq!(
            fs::read(destination.join("folder/data.txt")).unwrap(),
            b"0123456789"
        );
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn copying_a_selected_file_creates_missing_parent_directories() {
        let destination = test_destination("single-file");
        let (mut source, _, file) = fixture_source();

        let report = engine(&destination, CollisionPolicy::Skip)
            .copy_entry(&mut source, &file)
            .unwrap();

        assert_eq!(report.copied_files, 1);
        assert_eq!(report.created_directories, 1);
        assert_eq!(
            fs::read(destination.join("folder/data.txt")).unwrap(),
            b"0123456789"
        );
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn skip_collision_keeps_existing_file_without_reading_guest_data() {
        let destination = test_destination("skip");
        fs::create_dir_all(destination.join("folder")).unwrap();
        fs::write(destination.join("folder/data.txt"), b"old").unwrap();
        let (mut source, directory, _) = fixture_source();

        let report = engine(&destination, CollisionPolicy::Skip)
            .copy_entry(&mut source, &directory)
            .unwrap();

        assert_eq!(report.copied_files, 0);
        assert_eq!(report.skipped_entries, 1);
        assert!(source.requested_chunks.is_empty());
        assert_eq!(
            fs::read(destination.join("folder/data.txt")).unwrap(),
            b"old"
        );
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn overwrite_collision_replaces_existing_file() {
        let destination = test_destination("overwrite");
        fs::create_dir_all(destination.join("folder")).unwrap();
        fs::write(destination.join("folder/data.txt"), b"old").unwrap();
        let (mut source, directory, _) = fixture_source();

        let report = engine(&destination, CollisionPolicy::Overwrite)
            .copy_entry(&mut source, &directory)
            .unwrap();

        assert_eq!(report.copied_files, 1);
        assert_eq!(
            fs::read(destination.join("folder/data.txt")).unwrap(),
            b"0123456789"
        );
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn rename_collision_places_file_in_the_next_available_name() {
        let destination = test_destination("rename");
        fs::create_dir_all(destination.join("folder")).unwrap();
        fs::write(destination.join("folder/data.txt"), b"old").unwrap();
        fs::write(destination.join("folder/data (1).txt"), b"old-1").unwrap();
        let (mut source, directory, _) = fixture_source();

        let report = engine(&destination, CollisionPolicy::Rename)
            .copy_entry(&mut source, &directory)
            .unwrap();

        assert_eq!(report.copied_files, 1);
        assert_eq!(report.renamed_entries, 1);
        assert_eq!(
            fs::read(destination.join("folder/data (2).txt")).unwrap(),
            b"0123456789"
        );
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn renaming_a_directory_keeps_children_under_the_renamed_directory() {
        let destination = test_destination("rename-directory");
        fs::write(destination.join("folder"), b"blocking-file").unwrap();
        let (mut source, directory, _) = fixture_source();

        let report = engine(&destination, CollisionPolicy::Rename)
            .copy_entry(&mut source, &directory)
            .unwrap();

        assert_eq!(report.copied_files, 1);
        assert_eq!(report.renamed_entries, 1);
        assert_eq!(
            fs::read(destination.join("folder (1)/data.txt")).unwrap(),
            b"0123456789"
        );
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn rejects_zero_sized_copy_chunks() {
        let result = GuestCopyEngine::new(policy(260), CollisionPolicy::Skip, 0);

        assert!(matches!(
            result,
            Err(VirtualDiskError::InvalidEntryKind(message)) if message.contains("청크")
        ));
    }
}
