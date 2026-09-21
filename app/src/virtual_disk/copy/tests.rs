use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use super::super::issues::CopyIssueKind;
use super::*;
use crate::virtual_disk::{GuestFileAttributes, GuestFileKind, GuestFileSystem};

struct FixtureSource {
    files: HashMap<String, Vec<u8>>,
    directories: HashMap<String, Vec<GuestFileEntry>>,
    requested_chunks: Vec<usize>,
    cancel_after_chunks: Option<usize>,
    cancel: Option<Arc<AtomicBool>>,
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
        if self.cancel_after_chunks == Some(self.requested_chunks.len()) {
            if let Some(cancel) = &self.cancel {
                cancel.store(true, Ordering::Relaxed);
            }
        }
        let contents = self
            .files
            .get(file.path.as_str())
            .ok_or_else(|| VirtualDiskError::InvalidGuestPath(file.path.to_string()))?;
        let offset = usize::try_from(offset).map_err(|_| VirtualDiskError::BoundsViolation {
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
        times: crate::virtual_disk::GuestFileTimes::default(),
    }
}

fn file(path: &str, size_bytes: usize) -> GuestFileEntry {
    GuestFileEntry {
        path: GuestPath::new(path).unwrap(),
        kind: GuestFileKind::File,
        size_bytes: size_bytes as u64,
        attributes: GuestFileAttributes::default(),
        times: crate::virtual_disk::GuestFileTimes::default(),
    }
}

fn fixture_source() -> (FixtureSource, GuestFileEntry, GuestFileEntry) {
    let directory = directory("folder");
    let file = file("folder/data.txt", 10);
    let source = FixtureSource {
        files: HashMap::from([(file.path.to_string(), b"0123456789".to_vec())]),
        directories: HashMap::from([("folder".to_string(), vec![file.clone()])]),
        requested_chunks: Vec::new(),
        cancel_after_chunks: None,
        cancel: None,
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
fn cancellation_stops_between_chunks_and_removes_partial_file() {
    let destination = test_destination("cancel-chunks");
    let (mut source, _, file) = fixture_source();
    let cancel = Arc::new(AtomicBool::new(false));
    source.cancel_after_chunks = Some(2);
    source.cancel = Some(Arc::clone(&cancel));
    let mut issue_log = CopyIssueLog::default();

    let report = engine(&destination, CollisionPolicy::Skip).copy_entry_collecting_with_cancel(
        &mut source,
        &file,
        &mut issue_log,
        &cancel,
    );

    assert!(report.cancelled);
    assert_eq!(report.copied_files, 0);
    assert_eq!(source.requested_chunks, vec![3, 3]);
    assert!(!destination.join("folder/data.txt").exists());
    assert!(issue_log.issues().is_empty());
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
fn collecting_copy_errors_removes_partial_output_and_keeps_issue() {
    let destination = test_destination("issues");
    let (mut source, _, mut file) = fixture_source();
    file.size_bytes = 12;
    let mut issue_log = CopyIssueLog::default();

    let report = engine(&destination, CollisionPolicy::Skip).copy_entry_collecting(
        &mut source,
        &file,
        &mut issue_log,
    );

    assert_eq!(report.copied_files, 0);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(issue_log.issues().len(), 1);
    assert_eq!(report.issues[0].kind, CopyIssueKind::SourceChanged);
    assert!(!destination.join("folder/data.txt").exists());
    fs::remove_dir_all(destination).unwrap();
}

#[test]
fn collecting_directory_copy_continues_with_sibling_files_after_failure() {
    let destination = test_destination("issues-continue");
    let (mut source, directory, valid_file) = fixture_source();
    let broken_file = file("folder/broken.txt", 3);
    source
        .files
        .insert(broken_file.path.to_string(), b"x".to_vec());
    source
        .directories
        .insert("folder".to_string(), vec![broken_file, valid_file]);
    let mut issue_log = CopyIssueLog::default();

    let report = engine(&destination, CollisionPolicy::Skip).copy_entry_collecting(
        &mut source,
        &directory,
        &mut issue_log,
    );

    assert_eq!(report.copied_files, 1);
    assert_eq!(report.issues.len(), 1);
    assert_eq!(issue_log.issues().len(), 1);
    assert_eq!(report.issues[0].kind, CopyIssueKind::SourceChanged);
    assert!(!destination.join("folder/broken.txt").exists());
    assert_eq!(
        fs::read(destination.join("folder/data.txt")).unwrap(),
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
