//! 게스트 경로를 호스트 대상 경로로 변환하는 안전성 정책.

use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use super::{GuestFileAttributes, GuestFileEntry, GuestPath, IoOperation, VirtualDiskError};

/// 게스트 경로를 호스트 대상 경로로 변환할 때 적용하는 정책.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPathPolicy {
    destination_root: PathBuf,
    max_path_units: usize,
}

impl HostPathPolicy {
    /// 절대 경로인 호스트 대상 루트와 허용할 UTF-16 코드 단위 길이를 설정한다.
    ///
    /// 길이 제한은 OS·정책별로 달라질 수 있으므로 호출자가 설정한다. 루트 경로는
    /// 링크를 따라가지 않는 검사에 사용할 수 있도록 이미 존재하는 경로라면 함께
    /// 검사한다.
    pub fn new(
        destination_root: impl Into<PathBuf>,
        max_path_units: usize,
    ) -> Result<Self, VirtualDiskError> {
        let destination_root = destination_root.into();

        if !destination_root.is_absolute() {
            return Err(VirtualDiskError::UnsafePath {
                path: destination_root.display().to_string(),
                detail: "대상 루트는 절대 경로여야 합니다".to_string(),
            });
        }
        if max_path_units == 0 {
            return Err(VirtualDiskError::PathTooLong {
                path: destination_root.display().to_string(),
                length: 0,
                max: max_path_units,
            });
        }
        if destination_root
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(VirtualDiskError::UnsafePath {
                path: destination_root.display().to_string(),
                detail: "대상 루트에 . 또는 .. 구성요소를 사용할 수 없습니다".to_string(),
            });
        }

        let policy = Self {
            destination_root,
            max_path_units,
        };
        policy.validate_existing_components(&policy.destination_root)?;
        policy.ensure_length(&policy.destination_root)?;
        Ok(policy)
    }

    pub fn destination_root(&self) -> &Path {
        &self.destination_root
    }

    pub const fn max_path_units(&self) -> usize {
        self.max_path_units
    }

    /// 파일 항목의 속성까지 확인한 뒤 호스트 대상 경로를 반환한다.
    pub fn map_entry(&self, entry: &GuestFileEntry) -> Result<PathBuf, VirtualDiskError> {
        if entry
            .attributes
            .contains(GuestFileAttributes::REPARSE_POINT)
        {
            return Err(VirtualDiskError::UnsafePath {
                path: entry.path.to_string(),
                detail: "게스트 리파스 포인트 또는 심볼릭 링크는 추적하지 않습니다".to_string(),
            });
        }

        self.map_path(&entry.path)
    }

    /// 정규화된 게스트 상대 경로를 호스트 경로로 변환한다.
    pub fn map_path(&self, path: &GuestPath) -> Result<PathBuf, VirtualDiskError> {
        let mut candidate = self.destination_root.clone();

        for component in path
            .as_str()
            .split('/')
            .filter(|component| !component.is_empty())
        {
            validate_windows_component(component, path)?;
            candidate.push(component);
        }

        self.validate_target_path(&candidate)?;
        Ok(candidate)
    }

    /// 이미 계산된 호스트 후보 경로도 동일한 루트·길이·링크 정책으로 검사한다.
    /// 충돌 시 새 이름을 선택하는 복사 단계에서 사용한다.
    pub fn validate_target_path(&self, candidate: &Path) -> Result<(), VirtualDiskError> {
        if !candidate.starts_with(&self.destination_root) {
            return Err(VirtualDiskError::UnsafePath {
                path: candidate.display().to_string(),
                detail: "대상 루트 밖의 경로입니다".to_string(),
            });
        }

        self.ensure_length(candidate)?;
        self.validate_existing_components(candidate)
    }

    /// 대상 경로에 이미 존재하는 구성요소가 링크를 통해 다른 위치를 가리키지 않는지
    /// 검사한다. 존재하지 않는 마지막 구성요소는 복사 단계에서 생성할 수 있으므로
    /// 검사하지 않고 통과시킨다.
    pub fn validate_existing_components(&self, candidate: &Path) -> Result<(), VirtualDiskError> {
        if !candidate.starts_with(&self.destination_root) {
            return Err(VirtualDiskError::UnsafePath {
                path: candidate.display().to_string(),
                detail: "대상 루트 밖의 경로입니다".to_string(),
            });
        }

        let mut current = PathBuf::new();
        for component in candidate.components() {
            current.push(component.as_os_str());

            let metadata = match fs::symlink_metadata(&current) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                Err(source) => {
                    return Err(VirtualDiskError::Io {
                        operation: IoOperation::Open,
                        source,
                    });
                }
            };

            if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                return Err(VirtualDiskError::UnsafePath {
                    path: current.display().to_string(),
                    detail: "호스트 심볼릭 링크 또는 리파스 포인트는 통과하지 않습니다".to_string(),
                });
            }
        }

        Ok(())
    }

    fn ensure_length(&self, path: &Path) -> Result<(), VirtualDiskError> {
        let length = path.as_os_str().to_string_lossy().encode_utf16().count();
        if length > self.max_path_units {
            return Err(VirtualDiskError::PathTooLong {
                path: path.display().to_string(),
                length,
                max: self.max_path_units,
            });
        }
        Ok(())
    }
}

fn validate_windows_component(
    component: &str,
    full_path: &GuestPath,
) -> Result<(), VirtualDiskError> {
    if component.is_empty() || component == "." || component == ".." {
        return Err(VirtualDiskError::InvalidGuestPath(full_path.to_string()));
    }
    if component.ends_with([' ', '.']) {
        return Err(VirtualDiskError::InvalidGuestPath(format!(
            "{} (Windows 이름은 공백 또는 마침표로 끝날 수 없습니다)",
            full_path
        )));
    }
    if component
        .chars()
        .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
    {
        return Err(VirtualDiskError::InvalidGuestPath(format!(
            "{} (Windows 파일 이름 문자를 사용할 수 없습니다)",
            full_path
        )));
    }

    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .trim_end_matches([' ', '.']);
    let normalized = stem.to_ascii_uppercase();
    if matches!(
        normalized.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        return Err(VirtualDiskError::InvalidGuestPath(format!(
            "{} (Windows 예약 장치 이름)",
            full_path
        )));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(target_os = "windows"))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(max_path_units: usize) -> HostPathPolicy {
        HostPathPolicy::new(std::env::temp_dir(), max_path_units).unwrap()
    }

    fn entry(path: &str, attributes: GuestFileAttributes) -> GuestFileEntry {
        GuestFileEntry {
            path: GuestPath::new(path).unwrap(),
            kind: super::super::GuestFileKind::File,
            size_bytes: 1,
            attributes,
            times: super::super::GuestFileTimes::default(),
        }
    }

    #[test]
    fn maps_nested_guest_path_under_the_destination_root() {
        let mapper = policy(260);
        let mapped = mapper
            .map_entry(&entry(
                "Users/guest/notes.txt",
                GuestFileAttributes::default(),
            ))
            .unwrap();

        assert_eq!(mapped, std::env::temp_dir().join("Users/guest/notes.txt"));
        assert!(mapped.starts_with(mapper.destination_root()));
    }

    #[test]
    fn rejects_guest_traversal_before_mapping() {
        for path in ["../outside.txt", "Users/../outside.txt", "/outside.txt"] {
            assert!(matches!(
                GuestPath::new(path),
                Err(VirtualDiskError::InvalidGuestPath(_))
            ));
        }
    }

    #[test]
    fn rejects_windows_reserved_names_and_invalid_suffixes() {
        for path in [
            "CON",
            "con.txt",
            "LPT9.log",
            "folder/AUX.data",
            "trailing-space ",
            "trailing-dot.",
            "question?.txt",
        ] {
            let guest_path = GuestPath::new(path).unwrap();
            assert!(matches!(
                policy(260).map_path(&guest_path),
                Err(VirtualDiskError::InvalidGuestPath(_))
            ));
        }
    }

    #[test]
    fn rejects_paths_over_the_configured_limit() {
        let mapper = policy(40);
        let guest_path = GuestPath::new("this-name-is-too-long.txt").unwrap();

        assert!(matches!(
            mapper.map_path(&guest_path),
            Err(VirtualDiskError::PathTooLong { .. })
        ));
    }

    #[test]
    fn rejects_guest_reparse_points_without_following_them() {
        let mapper = policy(260);
        let reparse_entry = entry("linked/file.txt", GuestFileAttributes::REPARSE_POINT);

        assert!(matches!(
            mapper.map_entry(&reparse_entry),
            Err(VirtualDiskError::UnsafePath { .. })
        ));
    }

    #[test]
    fn rejects_relative_or_ambiguous_destination_roots() {
        assert!(matches!(
            HostPathPolicy::new("relative-target", 260),
            Err(VirtualDiskError::UnsafePath { .. })
        ));
        assert!(matches!(
            HostPathPolicy::new(std::env::temp_dir().join(".."), 260),
            Err(VirtualDiskError::UnsafePath { .. })
        ));
    }
}
