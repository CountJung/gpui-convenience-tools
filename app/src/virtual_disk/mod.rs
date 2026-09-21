//! VirtualBox 오프라인 디스크 탐색에 사용하는 플랫폼 비의존 도메인 모델.
//!
//! 이 모듈은 아직 VDI·파일시스템을 직접 열지 않는다. 읽기 전용 컨테이너와 게스트
//! 파일시스템 구현이 같은 계약을 사용하도록 경계를 먼저 고정한다.

// 아직 UI·파티션 계층이 연결되지 않은 도메인 모델도 단계별로 먼저 고정한다.
#![allow(dead_code)]

pub mod copy;
pub mod issues;
pub mod metadata;
pub mod ntfs;
pub mod partition;
pub mod path_policy;
pub mod vdi;

use std::{fmt, io, path::PathBuf, time::SystemTime};

use thiserror::Error;

/// VDI pre-header의 버전 값. 상위 16비트가 major, 하위 16비트가 minor다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VdiVersion {
    major: u16,
    minor: u16,
}

impl VdiVersion {
    /// VirtualBox가 현재 사용하는 VDI 1.1 버전.
    pub const CURRENT: Self = Self { major: 1, minor: 1 };

    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    pub const fn from_raw(raw: u32) -> Self {
        Self {
            major: (raw >> 16) as u16,
            minor: raw as u16,
        }
    }

    pub const fn major(self) -> u16 {
        self.major
    }

    pub const fn minor(self) -> u16 {
        self.minor
    }

    pub const fn raw(self) -> u32 {
        ((self.major as u32) << 16) | self.minor as u32
    }
}

impl fmt::Display for VdiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// VDI 이미지 유형.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VdiImageType {
    /// VirtualBox의 normal 이미지이며 블록이 필요할 때 할당되는 동적 이미지다.
    Dynamic,
    /// 모든 블록이 미리 할당된 고정 이미지다.
    Fixed,
    /// 부모 VDI와의 체인이 필요한 차등 이미지다.
    Differencing,
}

impl VdiImageType {
    pub const fn is_supported_offline(self) -> bool {
        matches!(self, Self::Dynamic | Self::Fixed)
    }
}

/// 검증을 통과한 VDI 컨테이너의 메타데이터.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VdiImage {
    pub path: PathBuf,
    pub version: VdiVersion,
    pub image_type: VdiImageType,
    pub disk_size_bytes: u64,
    pub sector_size: u32,
}

/// 디스크 파티션 테이블 유형.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartitionTableKind {
    Mbr,
    Gpt,
}

/// VDI 안에서 발견한 게스트 파티션의 메타데이터.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VdiPartition {
    pub number: u32,
    pub table: PartitionTableKind,
    pub start_lba: u64,
    pub sector_count: u64,
    pub filesystem: Option<GuestFileSystem>,
}

/// 초기 릴리즈에서 파일 탐색을 제공하는 게스트 파일시스템.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestFileSystem {
    Ntfs { major: u8, minor: u8 },
}

impl GuestFileSystem {
    pub const fn ntfs_3_1() -> Self {
        Self::Ntfs { major: 3, minor: 1 }
    }
}

/// 게스트 파일 항목의 종류.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuestFileKind {
    File,
    Directory,
}

/// Windows/NTFS 파일 속성 비트.
///
/// 숨김·시스템 파일을 필터링하지 않고 표시하기 위해 파일 항목에 함께 전달한다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GuestFileAttributes(u32);

impl GuestFileAttributes {
    pub const READ_ONLY: Self = Self(0x0001);
    pub const HIDDEN: Self = Self(0x0002);
    pub const SYSTEM: Self = Self(0x0004);
    pub const ARCHIVE: Self = Self(0x0020);
    pub const TEMPORARY: Self = Self(0x0100);
    pub const SPARSE_FILE: Self = Self(0x0200);
    pub const REPARSE_POINT: Self = Self(0x0400);
    pub const COMPRESSED: Self = Self(0x0800);
    pub const OFFLINE: Self = Self(0x1000);
    pub const NOT_CONTENT_INDEXED: Self = Self(0x2000);
    pub const ENCRYPTED: Self = Self(0x4000);

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// 게스트 내부의 정규화된 상대 경로.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GuestPath(String);

impl GuestPath {
    pub fn new(path: impl AsRef<str>) -> Result<Self, VirtualDiskError> {
        let path = path.as_ref().replace('\\', "/");

        if path.starts_with('/') || path.contains(':') {
            return Err(VirtualDiskError::InvalidGuestPath(path));
        }

        let mut components = Vec::new();
        for component in path.split('/').filter(|component| !component.is_empty()) {
            if component == "." || component == ".." {
                return Err(VirtualDiskError::InvalidGuestPath(path));
            }
            components.push(component);
        }

        Ok(Self(components.join("/")))
    }

    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn join(&self, name: &str) -> Result<Self, VirtualDiskError> {
        if name.is_empty() || name.contains('/') || name.contains('\\') {
            return Err(VirtualDiskError::InvalidGuestPath(name.to_string()));
        }

        let path = if self.is_root() {
            name.to_string()
        } else {
            format!("{}/{}", self.0, name)
        };
        Self::new(path)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn file_name(&self) -> Option<&str> {
        self.0.rsplit('/').next().filter(|name| !name.is_empty())
    }
}

impl fmt::Display for GuestPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_root() {
            f.write_str("/")
        } else {
            f.write_str(self.as_str())
        }
    }
}

/// 탐색기 목록에 전달하는 게스트 파일 항목.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestFileEntry {
    pub path: GuestPath,
    pub kind: GuestFileKind,
    pub size_bytes: u64,
    pub attributes: GuestFileAttributes,
    pub times: GuestFileTimes,
}

/// 게스트 파일 항목에서 호스트로 전달할 수 있는 시간 메타데이터.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuestFileTimes {
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
}

impl GuestFileEntry {
    pub fn root() -> Self {
        Self {
            path: GuestPath::root(),
            kind: GuestFileKind::Directory,
            size_bytes: 0,
            attributes: GuestFileAttributes::default(),
            times: GuestFileTimes::default(),
        }
    }

    pub const fn is_directory(&self) -> bool {
        matches!(self.kind, GuestFileKind::Directory)
    }
}

/// 게스트 파일시스템을 읽기 전용 탐색 계층으로 추상화한다.
///
/// 구현체는 원본 VDI나 게스트 파일시스템에 쓰지 않아야 한다. 파일 읽기는 차후 VDI
/// 블록 리더와 결합할 수 있도록 offset 기반으로 정의하며, 파일 내용을 소유한 버퍼로
/// 복사하지 않고 호출자 버퍼에 직접 채운다.
pub trait GuestFileSource: Send {
    fn filesystem(&self) -> GuestFileSystem;

    fn root(&mut self) -> Result<GuestFileEntry, VirtualDiskError> {
        Ok(GuestFileEntry::root())
    }

    fn list_directory(
        &mut self,
        directory: &GuestFileEntry,
    ) -> Result<Vec<GuestFileEntry>, VirtualDiskError>;

    fn read_at(
        &mut self,
        file: &GuestFileEntry,
        offset: u64,
        buffer: &mut [u8],
    ) -> Result<usize, VirtualDiskError>;
}

/// VDI·파티션·게스트 파일시스템 계층에서 공유하는 플랫폼 비의존 오류.
#[derive(Debug, Error)]
pub enum VirtualDiskError {
    #[error("게스트 경로가 유효하지 않습니다: {0}")]
    InvalidGuestPath(String),

    #[error("파일 항목 종류가 작업과 맞지 않습니다: {0}")]
    InvalidEntryKind(String),

    #[error("지원하지 않는 형식({kind}): {detail}")]
    UnsupportedFormat {
        kind: UnsupportedFormatKind,
        detail: String,
    },

    #[error("VDI 메타데이터가 손상되었습니다: {0}")]
    CorruptImage(String),

    #[error("읽기 전용 접근 위반: {0}")]
    ReadOnlyViolation(String),

    #[error("디스크 범위를 벗어난 접근: offset={offset}, length={length}, capacity={capacity}")]
    BoundsViolation {
        offset: u64,
        length: u64,
        capacity: u64,
    },

    #[error("원본 변경이 감지되었습니다: {0}")]
    SourceChanged(String),

    #[error("안전하지 않은 경로({path}): {detail}")]
    UnsafePath { path: String, detail: String },

    #[error("호스트 경로가 너무 깁니다: length={length}, max={max}, path={path}")]
    PathTooLong {
        path: String,
        length: usize,
        max: usize,
    },

    #[error("게스트 항목({path}) 처리 실패: {source}")]
    GuestEntry {
        path: String,
        #[source]
        source: Box<Self>,
    },

    #[error("{operation} 중 I/O 오류: {source}")]
    Io {
        operation: IoOperation,
        #[source]
        source: io::Error,
    },
}

impl VirtualDiskError {
    pub fn with_guest_path(self, path: &GuestPath) -> Self {
        match self {
            Self::SourceChanged(_) | Self::GuestEntry { .. } => self,
            source => Self::GuestEntry {
                path: path.to_string(),
                source: Box::new(source),
            },
        }
    }
}

/// 지원하지 않는 형식을 사용자 메시지·알림 억제 키에 매핑할 분류.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedFormatKind {
    VdiVersion,
    VdiImageType,
    SectorSize,
    PartitionTable,
    FileSystem,
    FileStream,
}

impl fmt::Display for UnsupportedFormatKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::VdiVersion => "VDI 버전",
            Self::VdiImageType => "VDI 이미지 유형",
            Self::SectorSize => "섹터 크기",
            Self::PartitionTable => "파티션 테이블",
            Self::FileSystem => "파일시스템",
            Self::FileStream => "파일 스트림",
        };
        f.write_str(label)
    }
}

/// I/O 오류가 발생한 논리 연산.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoOperation {
    Open,
    Read,
    Seek,
    ListDirectory,
    CreateDirectory,
    Write,
    RemoveFile,
}

impl fmt::Display for IoOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Open => "열기",
            Self::Read => "읽기",
            Self::Seek => "탐색",
            Self::ListDirectory => "디렉터리 열거",
            Self::CreateDirectory => "디렉터리 생성",
            Self::Write => "쓰기",
            Self::RemoveFile => "부분 파일 제거",
        };
        f.write_str(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vdi_version_round_trips_raw_value() {
        let version = VdiVersion::from_raw(0x0001_0001);

        assert_eq!(version, VdiVersion::CURRENT);
        assert_eq!(version.raw(), 0x0001_0001);
        assert_eq!(version.to_string(), "1.1");
    }

    #[test]
    fn guest_path_normalizes_separators_and_joins_entries() {
        let path = GuestPath::new(r"Windows\System32").unwrap();
        let child = path.join("drivers").unwrap();

        assert_eq!(path.as_str(), "Windows/System32");
        assert_eq!(child.as_str(), "Windows/System32/drivers");
        assert_eq!(child.file_name(), Some("drivers"));
    }

    #[test]
    fn guest_path_rejects_absolute_and_traversal_paths() {
        for path in [
            r"C:\Windows",
            r"\Windows",
            "/Windows",
            "Windows/../Secrets",
            "../Secrets",
        ] {
            assert!(matches!(
                GuestPath::new(path),
                Err(VirtualDiskError::InvalidGuestPath(_))
            ));
        }
    }

    #[test]
    fn file_attributes_preserve_hidden_and_system_bits() {
        let attributes = GuestFileAttributes::HIDDEN.union(GuestFileAttributes::SYSTEM);

        assert!(attributes.contains(GuestFileAttributes::HIDDEN));
        assert!(attributes.contains(GuestFileAttributes::SYSTEM));
        assert!(!attributes.contains(GuestFileAttributes::ARCHIVE));
    }

    #[test]
    fn root_entry_is_a_directory_at_root_path() {
        let root = GuestFileEntry::root();

        assert!(root.is_directory());
        assert!(root.path.is_root());
    }
}
