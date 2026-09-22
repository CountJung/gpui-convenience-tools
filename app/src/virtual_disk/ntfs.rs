//! VDE-007: VDI 파티션 위의 NTFS 읽기 전용 탐색 어댑터.
//!
//! `ntfs` 크레이트가 요구하는 `Read + Seek` 스트림을 파티션 범위로 제한하고,
//! 프로젝트의 `GuestFileSource` 계약으로 변환한다. 파일시스템의 메타데이터와
//! 데이터 스트림은 모두 원본에 쓰지 않으며, 숨김·시스템·읽기 전용 속성은
//! 필터링하지 않고 항목에 보존한다.

use std::{
    io::{self, Read, Seek, SeekFrom},
    time::{Duration, SystemTime},
};

use ntfs::{indexes::NtfsFileNameIndex, structured_values::NtfsFileNamespace, Ntfs, NtfsReadSeek};

use super::{
    partition::PartitionSource, GuestFileAttributes, GuestFileEntry, GuestFileKind,
    GuestFileSource, GuestFileSystem, GuestFileTimes, GuestPath, IoOperation,
    UnsupportedFormatKind, VdiPartition, VirtualDiskError,
};

const NTFS_ATTRIBUTE_MASK: u32 = 0x0001
    | 0x0002
    | 0x0004
    | 0x0020
    | 0x0040
    | 0x0080
    | 0x0100
    | 0x0200
    | 0x0400
    | 0x0800
    | 0x1000
    | 0x2000
    | 0x4000;

/// 파티션의 바이트 범위만 노출하는 `Read + Seek` 어댑터.
struct PartitionIo<S> {
    source: S,
    base_offset: u64,
    length: u64,
    position: u64,
    source_error: Option<VirtualDiskError>,
}

impl<S: PartitionSource> PartitionIo<S> {
    fn new(
        source: S,
        partition: &VdiPartition,
        sector_size: u32,
    ) -> Result<Self, VirtualDiskError> {
        let disk_size = source.disk_size_bytes();
        let base_offset = partition
            .start_lba
            .checked_mul(sector_size as u64)
            .ok_or_else(|| corrupt("NTFS 파티션 시작 위치 계산이 오버플로되었습니다"))?;
        let length = partition
            .sector_count
            .checked_mul(sector_size as u64)
            .ok_or_else(|| corrupt("NTFS 파티션 크기 계산이 오버플로되었습니다"))?;
        let end_offset = base_offset
            .checked_add(length)
            .ok_or_else(|| corrupt("NTFS 파티션 끝 위치 계산이 오버플로되었습니다"))?;
        if end_offset > disk_size {
            return Err(VirtualDiskError::BoundsViolation {
                offset: base_offset,
                length,
                capacity: disk_size,
            });
        }

        Ok(Self {
            source,
            base_offset,
            length,
            position: 0,
            source_error: None,
        })
    }

    fn take_source_error(&mut self) -> Option<VirtualDiskError> {
        self.source_error.take()
    }

    fn absolute_offset(&self, relative: u64, length: usize) -> io::Result<u64> {
        let length = length as u64;
        let end = relative.checked_add(length).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "partition range overflow")
        })?;
        if end > self.length {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "partition range exceeded",
            ));
        }
        self.base_offset
            .checked_add(relative)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "disk offset overflow"))
    }
}

impl<S: PartitionSource> Read for PartitionIo<S> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }

        let offset = self.absolute_offset(self.position, buffer.len())?;
        match self.source.read_at(offset, buffer) {
            Ok(read) if read <= buffer.len() => {
                self.position = self
                    .position
                    .checked_add(read as u64)
                    .ok_or_else(|| io::Error::other("partition position overflow"))?;
                Ok(read)
            }
            Ok(read) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("partition source returned too many bytes: {read}"),
            )),
            Err(error) => {
                self.source_error = Some(error);
                Err(io::Error::other("partition source read failed"))
            }
        }
    }
}

impl<S: PartitionSource> Seek for PartitionIo<S> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let next = match position {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(offset) => checked_seek(self.position, offset)?,
            SeekFrom::End(offset) => checked_seek(self.length, offset)?,
        };
        if next > self.length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "partition seek exceeded",
            ));
        }
        self.position = next;
        Ok(next)
    }
}

fn checked_seek(base: u64, offset: i64) -> io::Result<u64> {
    if offset >= 0 {
        base.checked_add(offset as u64)
    } else {
        base.checked_sub(offset.unsigned_abs())
    }
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "partition seek underflow"))
}

/// NTFS 파티션을 읽기 전용 게스트 파일 소스로 노출한다.
pub struct NtfsGuestFileSource<S: PartitionSource> {
    fs: PartitionIo<S>,
    ntfs: Ntfs,
    filesystem: GuestFileSystem,
}

impl<S: PartitionSource> NtfsGuestFileSource<S> {
    /// 파티션의 시작 LBA부터 정확한 파티션 길이만 읽어 NTFS를 연다.
    pub fn open(source: S, partition: VdiPartition) -> Result<Self, VirtualDiskError> {
        if partition.filesystem != Some(GuestFileSystem::ntfs_3_1()) {
            let detail = match partition.filesystem {
                Some(GuestFileSystem::BitLocker) => {
                    "BitLocker로 암호화된 파티션이라 복구 키 없이는 파일을 열 수 없습니다"
                }
                Some(GuestFileSystem::MicrosoftReserved) => {
                    "Microsoft Reserved(MSR) 예약 영역에는 탐색할 게스트 파일이 없습니다"
                }
                Some(GuestFileSystem::Ntfs { .. }) => "지원하지 않는 NTFS 버전입니다",
                None => "NTFS 파티션으로 식별되지 않은 파티션입니다",
            };
            return Err(VirtualDiskError::UnsupportedFormat {
                kind: UnsupportedFormatKind::FileSystem,
                detail: detail.to_string(),
            });
        }

        let sector_size = source.sector_size();
        let mut fs = PartitionIo::new(source, &partition, sector_size)?;
        let mut ntfs = Ntfs::new(&mut fs)
            .map_err(|error| map_ntfs_error(&mut fs, error, IoOperation::Open))?;
        ntfs.read_upcase_table(&mut fs)
            .map_err(|error| map_ntfs_error(&mut fs, error, IoOperation::Open))?;
        let volume_info = ntfs
            .volume_info(&mut fs)
            .map_err(|error| map_ntfs_error(&mut fs, error, IoOperation::Open))?;
        let filesystem = GuestFileSystem::Ntfs {
            major: volume_info.major_version(),
            minor: volume_info.minor_version(),
        };
        if filesystem != GuestFileSystem::ntfs_3_1() {
            return Err(VirtualDiskError::UnsupportedFormat {
                kind: UnsupportedFormatKind::FileSystem,
                detail: format!("지원하는 NTFS 버전은 3.1이며 실제 버전은 {filesystem:?}입니다"),
            });
        }
        ntfs.root_directory(&mut fs)
            .map_err(|error| map_ntfs_error(&mut fs, error, IoOperation::Open))?;

        Ok(Self {
            fs,
            ntfs,
            filesystem,
        })
    }

    fn resolve_file<'n>(
        ntfs: &'n Ntfs,
        fs: &mut PartitionIo<S>,
        path: &GuestPath,
    ) -> Result<ntfs::NtfsFile<'n>, VirtualDiskError> {
        let mut current = ntfs
            .root_directory(fs)
            .map_err(|error| map_ntfs_error(fs, error, IoOperation::Read).with_guest_path(path))?;

        for component in path
            .as_str()
            .split('/')
            .filter(|component| !component.is_empty())
        {
            if !current.is_directory() {
                return Err(VirtualDiskError::InvalidEntryKind(format!(
                    "디렉터리가 아닙니다: {path}"
                )));
            }
            let next = {
                let index = current.directory_index(fs).map_err(|error| {
                    map_ntfs_error(fs, error, IoOperation::ListDirectory).with_guest_path(path)
                })?;
                let mut finder = index.finder();
                let entry = NtfsFileNameIndex::find(&mut finder, ntfs, fs, component)
                    .ok_or_else(|| VirtualDiskError::InvalidGuestPath(path.to_string()))?
                    .map_err(|error| {
                        map_ntfs_error(fs, error, IoOperation::ListDirectory).with_guest_path(path)
                    })?;
                entry.to_file(ntfs, fs).map_err(|error| {
                    map_ntfs_error(fs, error, IoOperation::Read).with_guest_path(path)
                })?
            };
            current = next;
        }

        Ok(current)
    }

    fn entry_from_name(
        directory: &GuestFileEntry,
        file_name: &ntfs::structured_values::NtfsFileName,
    ) -> Result<GuestFileEntry, VirtualDiskError> {
        let name = file_name.name().to_string_lossy();
        if name == "." || name == ".." {
            return Err(VirtualDiskError::InvalidGuestPath(name));
        }
        let path = directory.path.join(&name)?;
        let kind = if file_name.is_directory() {
            GuestFileKind::Directory
        } else {
            GuestFileKind::File
        };
        let size_bytes = if matches!(kind, GuestFileKind::Directory) {
            0
        } else {
            file_name.data_size()
        };
        let attributes = GuestFileAttributes::from_bits(
            file_name.file_attributes().bits() & NTFS_ATTRIBUTE_MASK,
        );
        let times = GuestFileTimes {
            created: ntfs_time_to_system_time(file_name.creation_time().nt_timestamp()),
            modified: ntfs_time_to_system_time(file_name.modification_time().nt_timestamp()),
            accessed: ntfs_time_to_system_time(file_name.access_time().nt_timestamp()),
        };

        Ok(GuestFileEntry {
            path,
            kind,
            size_bytes,
            attributes,
            times,
        })
    }
}

impl<S: PartitionSource> GuestFileSource for NtfsGuestFileSource<S> {
    fn filesystem(&self) -> GuestFileSystem {
        self.filesystem
    }

    fn list_directory(
        &mut self,
        directory: &GuestFileEntry,
    ) -> Result<Vec<GuestFileEntry>, VirtualDiskError> {
        if !directory.is_directory() {
            return Err(VirtualDiskError::InvalidEntryKind(format!(
                "디렉터리만 열거할 수 있습니다: {}",
                directory.path
            )));
        }

        let file = Self::resolve_file(&self.ntfs, &mut self.fs, &directory.path)?;
        let index = file.directory_index(&mut self.fs).map_err(|error| {
            map_ntfs_error(&mut self.fs, error, IoOperation::ListDirectory)
                .with_guest_path(&directory.path)
        })?;
        let mut entries = Vec::new();
        let mut iterator = index.entries();
        while let Some(entry) = iterator.next(&mut self.fs) {
            let entry = entry.map_err(|error| {
                map_ntfs_error(&mut self.fs, error, IoOperation::ListDirectory)
                    .with_guest_path(&directory.path)
            })?;
            let file_name = entry
                .key()
                .ok_or_else(|| corrupt("NTFS 디렉터리 인덱스 항목에 파일명이 없습니다"))?
                .map_err(|error| {
                    map_ntfs_error(&mut self.fs, error, IoOperation::ListDirectory)
                        .with_guest_path(&directory.path)
                })?;
            if matches!(file_name.namespace(), NtfsFileNamespace::Dos) {
                continue;
            }
            let name = file_name.name().to_string_lossy();
            if name == "." || name == ".." {
                continue;
            }
            let mut guest_entry = Self::entry_from_name(directory, &file_name)
                .map_err(|error| error.with_guest_path(&directory.path))?;
            let file = entry.to_file(&self.ntfs, &mut self.fs).map_err(|error| {
                map_ntfs_error(&mut self.fs, error, IoOperation::Read)
                    .with_guest_path(&guest_entry.path)
            })?;
            let info = file.info().map_err(|error| {
                map_ntfs_error(&mut self.fs, error, IoOperation::Read)
                    .with_guest_path(&guest_entry.path)
            })?;
            guest_entry.times = GuestFileTimes {
                created: ntfs_time_to_system_time(info.creation_time().nt_timestamp()),
                modified: ntfs_time_to_system_time(info.modification_time().nt_timestamp()),
                accessed: ntfs_time_to_system_time(info.access_time().nt_timestamp()),
            };
            entries.push(guest_entry);
        }
        Ok(entries)
    }

    fn read_at(
        &mut self,
        file: &GuestFileEntry,
        offset: u64,
        buffer: &mut [u8],
    ) -> Result<usize, VirtualDiskError> {
        if file.is_directory() {
            return Err(VirtualDiskError::InvalidEntryKind(format!(
                "파일만 읽을 수 있습니다: {}",
                file.path
            )));
        }
        if let Some(reason) = unsupported_file_attributes(file.attributes) {
            return Err(reason.with_guest_path(&file.path));
        }
        let length = buffer.len() as u64;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| corrupt("NTFS 파일 읽기 범위 계산이 오버플로되었습니다"))?;
        if end > file.size_bytes {
            return Err(VirtualDiskError::BoundsViolation {
                offset,
                length,
                capacity: file.size_bytes,
            }
            .with_guest_path(&file.path));
        }
        if buffer.is_empty() {
            return Ok(0);
        }

        let resolved = Self::resolve_file(&self.ntfs, &mut self.fs, &file.path)?;
        let standard_attributes = resolved
            .info()
            .map_err(|error| {
                map_ntfs_error(&mut self.fs, error, IoOperation::Read).with_guest_path(&file.path)
            })?
            .file_attributes();
        if let Some(reason) = unsupported_file_attributes(GuestFileAttributes::from_bits(
            standard_attributes.bits() & NTFS_ATTRIBUTE_MASK,
        )) {
            return Err(reason.with_guest_path(&file.path));
        }
        let item = resolved
            .data(&mut self.fs, "")
            .ok_or_else(|| unsupported_stream(&file.path).with_guest_path(&file.path))?
            .map_err(|error| {
                map_ntfs_error(&mut self.fs, error, IoOperation::Read).with_guest_path(&file.path)
            })?;
        let attribute = item.to_attribute().map_err(|error| {
            map_ntfs_error(&mut self.fs, error, IoOperation::Read).with_guest_path(&file.path)
        })?;
        let data_flags = attribute.flags();
        if data_flags.contains(ntfs::NtfsAttributeFlags::COMPRESSED) {
            return Err(
                unsupported_file_stream("압축된 NTFS 파일 스트림", &file.path)
                    .with_guest_path(&file.path),
            );
        }
        if data_flags.contains(ntfs::NtfsAttributeFlags::ENCRYPTED) {
            return Err(
                unsupported_file_stream("암호화된 NTFS 파일 스트림", &file.path)
                    .with_guest_path(&file.path),
            );
        }
        let mut value = attribute.value(&mut self.fs).map_err(|error| {
            map_ntfs_error(&mut self.fs, error, IoOperation::Read).with_guest_path(&file.path)
        })?;
        if end > value.len() {
            return Err(VirtualDiskError::BoundsViolation {
                offset,
                length,
                capacity: value.len(),
            }
            .with_guest_path(&file.path));
        }
        value
            .seek(&mut self.fs, SeekFrom::Start(offset))
            .map_err(|error| {
                map_ntfs_error(&mut self.fs, error, IoOperation::Seek).with_guest_path(&file.path)
            })?;
        value.read(&mut self.fs, buffer).map_err(|error| {
            map_ntfs_error(&mut self.fs, error, IoOperation::Read).with_guest_path(&file.path)
        })
    }
}

fn ntfs_time_to_system_time(timestamp: u64) -> Option<SystemTime> {
    const NTFS_EPOCH_IN_100NS: i128 = 116_444_736_000_000_000;
    const HUNDRED_NANOS_PER_SECOND: i128 = 10_000_000;

    let unix_intervals = timestamp as i128 - NTFS_EPOCH_IN_100NS;
    if unix_intervals >= 0 {
        let seconds = unix_intervals / HUNDRED_NANOS_PER_SECOND;
        let nanos = (unix_intervals % HUNDRED_NANOS_PER_SECOND) * 100;
        Some(SystemTime::UNIX_EPOCH + Duration::new(seconds as u64, nanos as u32))
    } else {
        let magnitude = -unix_intervals;
        let seconds = magnitude / HUNDRED_NANOS_PER_SECOND;
        let nanos = (magnitude % HUNDRED_NANOS_PER_SECOND) * 100;
        Some(SystemTime::UNIX_EPOCH - Duration::new(seconds as u64, nanos as u32))
    }
}

fn unsupported_stream(path: &GuestPath) -> VirtualDiskError {
    unsupported_file_stream("기본 데이터 스트림이 없는 NTFS 파일", path)
}

fn unsupported_file_stream(detail: &str, path: &GuestPath) -> VirtualDiskError {
    VirtualDiskError::UnsupportedFormat {
        kind: UnsupportedFormatKind::FileStream,
        detail: format!("{detail}: {path}"),
    }
}

fn unsupported_file_attributes(attributes: GuestFileAttributes) -> Option<VirtualDiskError> {
    let unsupported = [
        (GuestFileAttributes::COMPRESSED, "압축된 NTFS 파일 스트림"),
        (GuestFileAttributes::ENCRYPTED, "암호화된 NTFS 파일 스트림"),
    ];
    unsupported
        .into_iter()
        .find(|(flag, _)| attributes.contains(*flag))
        .map(|(_, detail)| VirtualDiskError::UnsupportedFormat {
            kind: UnsupportedFormatKind::FileStream,
            detail: format!("{detail}은 현재 오프라인 복사 경로에서 지원하지 않습니다"),
        })
}

fn map_ntfs_error<S: PartitionSource>(
    fs: &mut PartitionIo<S>,
    error: ntfs::NtfsError,
    operation: IoOperation,
) -> VirtualDiskError {
    if let Some(source_error) = fs.take_source_error() {
        return source_error;
    }
    match error {
        ntfs::NtfsError::Io(source) => VirtualDiskError::Io { operation, source },
        other => VirtualDiskError::CorruptImage(format!("NTFS {operation} 실패: {other}")),
    }
}

fn corrupt(detail: impl Into<String>) -> VirtualDiskError {
    VirtualDiskError::CorruptImage(detail.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, File},
        sync::atomic::{AtomicU64, Ordering},
    };

    #[derive(Debug)]
    struct MemorySource {
        bytes: Vec<u8>,
        sector_size: u32,
    }

    impl PartitionSource for MemorySource {
        fn disk_size_bytes(&self) -> u64 {
            self.bytes.len() as u64
        }

        fn sector_size(&self) -> u32 {
            self.sector_size
        }

        fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VirtualDiskError> {
            let start = usize::try_from(offset).map_err(|_| corrupt("test offset overflow"))?;
            let end = start
                .checked_add(buffer.len())
                .ok_or_else(|| corrupt("test range overflow"))?;
            let source = self
                .bytes
                .get(start..end)
                .ok_or_else(|| corrupt("test range exceeded"))?;
            buffer.copy_from_slice(source);
            Ok(buffer.len())
        }
    }

    #[derive(Debug)]
    struct FileSource {
        file: File,
        size: u64,
    }

    impl PartitionSource for FileSource {
        fn disk_size_bytes(&self) -> u64 {
            self.size
        }

        fn sector_size(&self) -> u32 {
            512
        }

        fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VirtualDiskError> {
            self.file
                .seek(SeekFrom::Start(offset))
                .map_err(|source| VirtualDiskError::Io {
                    operation: IoOperation::Seek,
                    source,
                })?;
            self.file
                .read(buffer)
                .map_err(|source| VirtualDiskError::Io {
                    operation: IoOperation::Read,
                    source,
                })
        }
    }

    #[derive(Debug)]
    struct FailingSource;

    impl PartitionSource for FailingSource {
        fn disk_size_bytes(&self) -> u64 {
            8 * 512
        }

        fn sector_size(&self) -> u32 {
            512
        }

        fn read_at(&mut self, _offset: u64, _buffer: &mut [u8]) -> Result<usize, VirtualDiskError> {
            Err(VirtualDiskError::SourceChanged(
                "test source changed".to_string(),
            ))
        }
    }

    fn partition() -> VdiPartition {
        VdiPartition {
            number: 1,
            table: super::super::PartitionTableKind::Gpt,
            start_lba: 2,
            sector_count: 4,
            filesystem: Some(GuestFileSystem::ntfs_3_1()),
        }
    }

    #[test]
    fn partition_io_never_reads_outside_partition() {
        let source = MemorySource {
            bytes: vec![0; 8 * 512],
            sector_size: 512,
        };
        let mut io = PartitionIo::new(source, &partition(), 512).unwrap();
        io.seek(SeekFrom::Start(4 * 512 - 1)).unwrap();
        let mut buffer = [0; 2];

        assert_eq!(
            io.read(&mut buffer).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn checked_seek_rejects_underflow_and_overflow() {
        assert!(checked_seek(0, -1).is_err());
        assert!(checked_seek(u64::MAX, 1).is_err());
        assert_eq!(checked_seek(10, -4).unwrap(), 6);
    }

    #[test]
    fn partition_io_rejects_a_partition_beyond_the_source() {
        let source = MemorySource {
            bytes: vec![0; 4 * 512],
            sector_size: 512,
        };
        let partition = VdiPartition {
            number: 1,
            table: super::super::PartitionTableKind::Gpt,
            start_lba: 3,
            sector_count: 2,
            filesystem: Some(GuestFileSystem::ntfs_3_1()),
        };

        assert!(matches!(
            PartitionIo::new(source, &partition, 512),
            Err(VirtualDiskError::BoundsViolation { .. })
        ));
    }

    #[test]
    fn partition_source_errors_are_preserved_for_the_caller() {
        let mut io = PartitionIo::new(FailingSource, &partition(), 512).unwrap();
        let mut buffer = [0; 1];

        assert!(io.read(&mut buffer).is_err());
        assert!(matches!(
            io.take_source_error(),
            Some(VirtualDiskError::SourceChanged(_))
        ));
    }

    #[test]
    fn ntfs_attributes_keep_explorer_relevant_bits() {
        let source_bits = 0x0002 | 0x0004 | 0x0001 | 0x8000_0000;
        let attributes = GuestFileAttributes::from_bits(source_bits & NTFS_ATTRIBUTE_MASK);

        assert!(attributes.contains(GuestFileAttributes::HIDDEN));
        assert!(attributes.contains(GuestFileAttributes::SYSTEM));
        assert!(attributes.contains(GuestFileAttributes::READ_ONLY));
        assert_eq!(attributes.bits() & 0x8000_0000, 0);
    }

    #[test]
    fn ntfs_timestamp_conversion_preserves_unix_time() {
        let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        let ntfs_timestamp = 116_444_736_000_000_000u64 + 1_600_000_000u64 * 10_000_000;

        assert_eq!(ntfs_time_to_system_time(ntfs_timestamp), Some(expected));
    }

    #[test]
    fn compressed_and_encrypted_attributes_become_file_stream_errors() {
        for attributes in [
            GuestFileAttributes::COMPRESSED,
            GuestFileAttributes::ENCRYPTED,
        ] {
            assert!(matches!(
                unsupported_file_attributes(attributes),
                Some(VirtualDiskError::UnsupportedFormat {
                    kind: UnsupportedFormatKind::FileStream,
                    ..
                })
            ));
        }
    }

    #[test]
    fn entry_errors_keep_the_guest_path() {
        let path = GuestPath::new("Windows/System32").unwrap();
        let error = VirtualDiskError::BoundsViolation {
            offset: 4,
            length: 8,
            capacity: 4,
        }
        .with_guest_path(&path);

        assert!(
            matches!(error, VirtualDiskError::GuestEntry { ref path, .. } if path == "Windows/System32")
        );
        assert!(error.to_string().contains("Windows/System32"));
    }

    #[test]
    fn opens_bundled_ntfs_image_read_only() {
        let path = bundled_ntfs_path();
        let original = fs::read(&path).unwrap();
        let file = File::open(&path).unwrap();
        let size = file.metadata().unwrap().len();
        let partition = VdiPartition {
            number: 1,
            table: super::super::PartitionTableKind::Gpt,
            start_lba: 0,
            sector_count: size / 512,
            filesystem: Some(GuestFileSystem::ntfs_3_1()),
        };
        let mut source = NtfsGuestFileSource::open(FileSource { file, size }, partition).unwrap();
        let root = source.root().unwrap();
        let entries = source.list_directory(&root).unwrap();
        assert!(!entries.is_empty());
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn rejects_a_corrupt_copy_of_the_bundled_ntfs_image() {
        let source_path = bundled_ntfs_path();
        let corrupt_path = corrupt_ntfs_path();
        let mut bytes = fs::read(&source_path).unwrap();
        bytes[510] ^= 1;
        fs::write(&corrupt_path, &bytes).unwrap();

        let file = File::open(&corrupt_path).unwrap();
        let size = file.metadata().unwrap().len();
        let partition = VdiPartition {
            number: 1,
            table: super::super::PartitionTableKind::Gpt,
            start_lba: 0,
            sector_count: size / 512,
            filesystem: Some(GuestFileSystem::ntfs_3_1()),
        };

        assert!(matches!(
            NtfsGuestFileSource::open(FileSource { file, size }, partition),
            Err(VirtualDiskError::CorruptImage(_))
        ));
        fs::remove_file(corrupt_path).unwrap();
    }

    #[test]
    #[ignore = "실제 NTFS 이미지는 GPUI_CONVENIENCE_TOOLS_NTFS_TEST_IMAGE로 주입한다"]
    fn opens_configured_ntfs_image_read_only() {
        let Some(path) = std::env::var_os("GPUI_CONVENIENCE_TOOLS_NTFS_TEST_IMAGE") else {
            return;
        };
        let file = File::open(path).unwrap();
        let size = file.metadata().unwrap().len();
        assert!(size > 0 && size.is_multiple_of(512));
        let partition = VdiPartition {
            number: 1,
            table: super::super::PartitionTableKind::Gpt,
            start_lba: 0,
            sector_count: size / 512,
            filesystem: Some(GuestFileSystem::ntfs_3_1()),
        };
        let mut source = NtfsGuestFileSource::open(FileSource { file, size }, partition).unwrap();
        let root = source.root().unwrap();
        let entries = source.list_directory(&root).unwrap();
        assert!(!entries.is_empty());

        if let Some(file) = entries
            .iter()
            .find(|entry| !entry.is_directory() && entry.size_bytes > 0)
        {
            let read_length = usize::try_from(file.size_bytes.min(64)).unwrap();
            let mut buffer = vec![0; read_length];
            assert_eq!(source.read_at(file, 0, &mut buffer).unwrap(), read_length);
        }
    }

    #[test]
    #[ignore = "손상 NTFS 이미지는 GPUI_CONVENIENCE_TOOLS_NTFS_CORRUPT_TEST_IMAGE로 주입한다"]
    fn rejects_configured_corrupt_ntfs_image() {
        let Some(path) = std::env::var_os("GPUI_CONVENIENCE_TOOLS_NTFS_CORRUPT_TEST_IMAGE") else {
            return;
        };
        let file = File::open(path).unwrap();
        let size = file.metadata().unwrap().len();
        let partition = VdiPartition {
            number: 1,
            table: super::super::PartitionTableKind::Gpt,
            start_lba: 0,
            sector_count: size / 512,
            filesystem: Some(GuestFileSystem::ntfs_3_1()),
        };

        assert!(matches!(
            NtfsGuestFileSource::open(FileSource { file, size }, partition),
            Err(VirtualDiskError::CorruptImage(_))
        ));
    }

    fn bundled_ntfs_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("ntfs-testfs1.img")
    }

    fn corrupt_ntfs_path() -> std::path::PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "gpui-convenience-vde018-corrupt-{}-{}.img",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
