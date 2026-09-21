//! 게스트 파일 메타데이터를 호스트 항목에 적용하는 경계.

use std::{fs, io, path::Path, time::SystemTime};

use super::{GuestFileAttributes, GuestFileEntry, GuestFileTimes};

/// 복사 후 적용할 메타데이터 종류를 선택한다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetadataPolicy {
    pub apply_attributes: bool,
    pub apply_timestamps: bool,
}

impl Default for MetadataPolicy {
    fn default() -> Self {
        Self {
            apply_attributes: true,
            apply_timestamps: true,
        }
    }
}

/// 메타데이터 적용에 실패했지만 파일 복사는 계속된 항목.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataFailure {
    pub path: String,
    pub operation: MetadataOperation,
    pub detail: String,
}

/// 메타데이터 적용 작업의 분류.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataOperation {
    Attributes,
    Timestamps,
}

impl std::fmt::Display for MetadataOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Attributes => "파일 속성",
            Self::Timestamps => "타임스탬프",
        })
    }
}

/// 메타데이터를 가능한 범위에서 적용하고 실패 사유를 구조화해 반환한다.
pub fn apply_metadata(
    path: &Path,
    entry: &GuestFileEntry,
    policy: MetadataPolicy,
) -> Vec<MetadataFailure> {
    let mut failures = Vec::new();
    if policy.apply_attributes {
        if let Err(detail) = apply_attributes(path, entry.attributes) {
            failures.push(MetadataFailure {
                path: entry.path.to_string(),
                operation: MetadataOperation::Attributes,
                detail,
            });
        }
    }
    if policy.apply_timestamps {
        if let Err(detail) = apply_timestamps(path, &entry.times) {
            failures.push(MetadataFailure {
                path: entry.path.to_string(),
                operation: MetadataOperation::Timestamps,
                detail,
            });
        }
    }
    failures
}

#[cfg(target_os = "windows")]
fn apply_attributes(path: &Path, attributes: GuestFileAttributes) -> Result<(), String> {
    use std::os::windows::fs::MetadataExt;

    use windows_sys::Win32::Storage::FileSystem::{
        SetFileAttributesW, FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL,
        FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_SYSTEM,
    };

    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let mut flags = metadata.file_attributes();
    flags &= !(FILE_ATTRIBUTE_READONLY
        | FILE_ATTRIBUTE_HIDDEN
        | FILE_ATTRIBUTE_SYSTEM
        | FILE_ATTRIBUTE_ARCHIVE);
    if attributes.contains(GuestFileAttributes::READ_ONLY) {
        flags |= FILE_ATTRIBUTE_READONLY;
    }
    if attributes.contains(GuestFileAttributes::HIDDEN) {
        flags |= FILE_ATTRIBUTE_HIDDEN;
    }
    if attributes.contains(GuestFileAttributes::SYSTEM) {
        flags |= FILE_ATTRIBUTE_SYSTEM;
    }
    if attributes.contains(GuestFileAttributes::ARCHIVE) {
        flags |= FILE_ATTRIBUTE_ARCHIVE;
    }
    if flags == 0 {
        flags = FILE_ATTRIBUTE_NORMAL;
    }

    let wide = wide_path(path)?;
    // SAFETY: `wide` is a null-terminated UTF-16 path owned for the duration of
    // the call, and `flags` contains only Win32 file attribute bits.
    let success = unsafe { SetFileAttributesW(wide.as_ptr(), flags) } != 0;
    if success {
        Ok(())
    } else {
        Err(io::Error::last_os_error().to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_attributes(path: &Path, attributes: GuestFileAttributes) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let mut permissions = metadata.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(attributes.contains(GuestFileAttributes::READ_ONLY));
    fs::set_permissions(path, permissions).map_err(|error| error.to_string())?;

    if attributes.contains(GuestFileAttributes::HIDDEN)
        || attributes.contains(GuestFileAttributes::SYSTEM)
    {
        return Err("현재 플랫폼의 일반 파일 권한에는 HIDDEN/SYSTEM 속성이 없습니다".to_string());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn apply_timestamps(path: &Path, times: &GuestFileTimes) -> Result<(), String> {
    use std::ptr::null;

    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, SetFileTime, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, OPEN_EXISTING,
        },
    };

    if times.created.is_none() && times.modified.is_none() && times.accessed.is_none() {
        return Ok(());
    }
    let wide = wide_path(path)?;
    // SAFETY: the path buffer is valid for this call; the handle is closed on
    // every path after CreateFileW succeeds.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_WRITE_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error().to_string());
    }

    let creation = times.created.map(system_time_to_filetime).transpose()?;
    let accessed = times.accessed.map(system_time_to_filetime).transpose()?;
    let modified = times.modified.map(system_time_to_filetime).transpose()?;
    // SAFETY: optional FILETIME pointers are either null or point to local
    // values that remain alive until SetFileTime returns.
    let success = unsafe {
        SetFileTime(
            handle,
            creation
                .as_ref()
                .map_or(null(), |value| value as *const FILETIME),
            accessed
                .as_ref()
                .map_or(null(), |value| value as *const FILETIME),
            modified
                .as_ref()
                .map_or(null(), |value| value as *const FILETIME),
        )
    } != 0;
    if !success {
        let error = io::Error::last_os_error().to_string();
        // SAFETY: `handle` was returned by CreateFileW and is closed exactly once.
        unsafe { CloseHandle(handle) };
        return Err(error);
    }

    // SAFETY: `handle` was returned by CreateFileW and is closed exactly once.
    if unsafe { CloseHandle(handle) } == 0 {
        Err(io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_timestamps(path: &Path, times: &GuestFileTimes) -> Result<(), String> {
    if times.created.is_some() {
        return Err("현재 플랫폼은 파일 생성 시간 설정을 지원하지 않습니다".to_string());
    }
    if times.modified.is_none() && times.accessed.is_none() {
        return Ok(());
    }

    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let accessed = times
        .accessed
        .map(filetime::FileTime::from_system_time)
        .unwrap_or_else(|| filetime::FileTime::from_last_access_time(&metadata));
    let modified = times
        .modified
        .map(filetime::FileTime::from_system_time)
        .unwrap_or_else(|| filetime::FileTime::from_last_modification_time(&metadata));
    filetime::set_file_times(path, accessed, modified).map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn system_time_to_filetime(
    time: SystemTime,
) -> Result<windows_sys::Win32::Foundation::FILETIME, String> {
    let filetime = filetime::FileTime::from_system_time(time);
    let seconds = u64::try_from(filetime.seconds())
        .map_err(|_| "Windows 파일 시간 범위를 벗어났습니다".to_string())?;
    let intervals = seconds
        .checked_mul(10_000_000)
        .and_then(|value| value.checked_add((filetime.nanoseconds() / 100) as u64))
        .ok_or_else(|| "Windows 파일 시간 계산이 오버플로되었습니다".to_string())?;
    Ok(windows_sys::Win32::Foundation::FILETIME {
        dwLowDateTime: intervals as u32,
        dwHighDateTime: (intervals >> 32) as u32,
    })
}

#[cfg(target_os = "windows")]
fn wide_path(path: &Path) -> Result<Vec<u16>, String> {
    use std::os::windows::ffi::OsStrExt;

    if path.as_os_str().encode_wide().any(|unit| unit == 0) {
        return Err("경로에 null 문자가 포함되어 있습니다".to_string());
    }
    Ok(path.as_os_str().encode_wide().chain(Some(0)).collect())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;
    use crate::virtual_disk::{GuestFileKind, GuestPath};

    fn entry(attributes: GuestFileAttributes, times: GuestFileTimes) -> GuestFileEntry {
        GuestFileEntry {
            path: GuestPath::new("metadata.txt").unwrap(),
            kind: GuestFileKind::File,
            size_bytes: 0,
            attributes,
            times,
        }
    }

    fn temp_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gpui-convenience-tools-vde011-{name}-{}",
            std::process::id()
        ));
        fs::write(&path, b"metadata").unwrap();
        path
    }

    #[test]
    fn applies_access_and_modification_times() {
        let path = temp_file("times");
        let timestamp = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        let failures = apply_metadata(
            &path,
            &entry(
                GuestFileAttributes::default(),
                GuestFileTimes {
                    created: None,
                    modified: Some(timestamp),
                    accessed: Some(timestamp),
                },
            ),
            MetadataPolicy {
                apply_attributes: false,
                apply_timestamps: true,
            },
        );

        assert!(failures.is_empty(), "metadata failures: {failures:?}");
        let metadata = fs::metadata(&path).unwrap();
        assert!(
            metadata
                .modified()
                .unwrap()
                .duration_since(timestamp)
                .unwrap_or_else(|error| error.duration())
                <= Duration::from_secs(1)
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn applies_read_only_attribute() {
        let path = temp_file("readonly");
        let failures = apply_metadata(
            &path,
            &entry(GuestFileAttributes::READ_ONLY, GuestFileTimes::default()),
            MetadataPolicy {
                apply_attributes: true,
                apply_timestamps: false,
            },
        );

        assert!(failures.is_empty(), "metadata failures: {failures:?}");
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        apply_metadata(
            &path,
            &entry(GuestFileAttributes::default(), GuestFileTimes::default()),
            MetadataPolicy {
                apply_attributes: true,
                apply_timestamps: false,
            },
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn records_failures_without_turning_metadata_errors_into_copy_errors() {
        let path = std::env::temp_dir().join("gpui-convenience-tools-vde011-missing");
        let failures = apply_metadata(
            &path,
            &entry(GuestFileAttributes::default(), GuestFileTimes::default()),
            MetadataPolicy::default(),
        );

        assert_eq!(failures.len(), 1);
        assert!(failures
            .iter()
            .all(|failure| failure.path == "metadata.txt"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn applies_creation_time_on_windows() {
        let path = temp_file("creation-windows");
        let timestamp = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        let failures = apply_metadata(
            &path,
            &entry(
                GuestFileAttributes::default(),
                GuestFileTimes {
                    created: Some(timestamp),
                    ..GuestFileTimes::default()
                },
            ),
            MetadataPolicy {
                apply_attributes: false,
                apply_timestamps: true,
            },
        );

        assert!(failures.is_empty(), "metadata failures: {failures:?}");
        assert!(
            fs::metadata(&path)
                .unwrap()
                .created()
                .unwrap()
                .duration_since(timestamp)
                .unwrap_or_else(|error| error.duration())
                <= Duration::from_secs(1)
        );
        fs::remove_file(path).unwrap();
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn records_unrepresentable_creation_time() {
        let path = temp_file("creation");
        let failures = apply_metadata(
            &path,
            &entry(
                GuestFileAttributes::default(),
                GuestFileTimes {
                    created: Some(UNIX_EPOCH),
                    ..GuestFileTimes::default()
                },
            ),
            MetadataPolicy {
                apply_attributes: false,
                apply_timestamps: true,
            },
        );

        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].operation, MetadataOperation::Timestamps);
        fs::remove_file(path).unwrap();
    }
}
