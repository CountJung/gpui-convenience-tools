//! 동기화 경로의 링크 판정과 대상 폴더 안전 경계.

use std::{
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
};

pub(super) fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT (정션 포함)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub(super) fn resolve_inside(link: &Path, source_root: &Path) -> Result<PathBuf, String> {
    let resolved =
        fs::canonicalize(link).map_err(|err| format!("링크 대상을 읽을 수 없습니다: {err}"))?;
    if !resolved.starts_with(source_root) {
        return Err("원본 폴더 밖을 가리키는 링크는 처리하지 않습니다.".to_string());
    }
    Ok(resolved)
}

/// 아직 생성되지 않은 대상도 가장 가까운 기존 조상까지 정규화해 원본 내부 생성을 막는다.
pub(super) fn destination_is_inside_source(candidate: &Path, source: &Path) -> bool {
    let Ok(source) = fs::canonicalize(source) else {
        return false;
    };
    let mut missing = Vec::<OsString>::new();
    let mut ancestor = candidate;
    loop {
        if let Ok(mut resolved) = fs::canonicalize(ancestor) {
            for component in missing.iter().rev() {
                resolved.push(component);
            }
            return resolved.starts_with(&source);
        }
        let Some(component) = ancestor.file_name() else {
            return false;
        };
        missing.push(component.to_os_string());
        let Some(parent) = ancestor.parent() else {
            return false;
        };
        ancestor = parent;
    }
}

/// 이미 있는 대상 경로의 어느 조각도 링크·정션이면 쓰기를 거부한다.
pub(super) fn reject_destination_reparse(path: &Path, target_root: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(target_root)
        .map_err(|_| "대상 폴더 밖의 경로에는 쓸 수 없습니다.".to_string())?;
    // 대상 루트가 아직 없어도 상위 정션을 통과할 수 있으므로 먼저 전부 확인한다.
    for ancestor in target_root
        .ancestors()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        if !ancestor.as_os_str().is_empty() {
            check_link(ancestor)?;
        }
    }
    let mut cursor = target_root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => cursor.push(part),
            _ => return Err("대상 상대 경로가 안전하지 않습니다.".to_string()),
        }
        check_link(&cursor)?;
    }
    Ok(())
}

fn check_link(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if is_link(&meta) => Err(format!(
            "대상 경로에 링크·정션이 있어 쓰기를 중단했습니다: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("대상 경로를 확인할 수 없습니다: {err}")),
    }
}

/// 원본 내부 링크를 대상 트리 내부의 상대 링크로 재생성한다.
/// 기존에 동일한 링크가 있으면 건너뛰며, 다른 항목은 보존하고 실패로 보고한다.
pub(super) fn recreate(
    source_link: &Path,
    destination_link: &Path,
    source_root: &Path,
    target_root: &Path,
) -> Result<bool, String> {
    let resolved = resolve_inside(source_link, source_root)?;
    let within_source = resolved
        .strip_prefix(source_root)
        .map_err(|_| "원본 밖의 링크는 재생성할 수 없습니다.".to_string())?;
    let mapped_target = target_root.join(within_source);
    let parent = destination_link
        .parent()
        .ok_or_else(|| "대상 링크의 상위 폴더가 없습니다.".to_string())?;
    reject_destination_reparse(parent, target_root)?;
    let relative_target = relative_between(parent, &mapped_target)?;

    match fs::symlink_metadata(destination_link) {
        Ok(meta) if is_link(&meta) => {
            let existing = fs::read_link(destination_link)
                .map_err(|err| format!("기존 대상 링크를 읽을 수 없습니다: {err}"))?;
            if existing == relative_target {
                return Ok(false);
            }
            return Err(
                "대상에 다른 링크가 이미 있습니다. 자동으로 덮어쓰지 않았습니다.".to_string(),
            );
        }
        Ok(_) => {
            return Err(
                "대상에 같은 이름의 일반 파일·폴더가 있어 링크를 만들 수 없습니다.".to_string(),
            )
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("대상 링크 상태를 확인할 수 없습니다: {err}")),
    }

    create_link(&relative_target, destination_link, resolved.is_dir())
        .map_err(|err| describe_link_create_error(&err))?;
    Ok(true)
}

fn describe_link_create_error(err: &std::io::Error) -> String {
    if err.raw_os_error() == Some(1314) {
        "링크 생성 권한이 없습니다(Windows 관리자 권한 또는 개발자 모드 필요, code 1314)."
            .to_string()
    } else {
        format!("링크를 재생성할 수 없습니다: {err}")
    }
}

fn relative_between(from: &Path, to: &Path) -> Result<PathBuf, String> {
    let left: Vec<_> = from.components().collect();
    let right: Vec<_> = to.components().collect();
    let common = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return Err("대상 링크의 드라이브가 달라 상대 경로를 만들 수 없습니다.".to_string());
    }
    let mut result = PathBuf::new();
    for _ in common..left.len() {
        result.push("..");
    }
    for component in &right[common..] {
        result.push(component.as_os_str());
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    Ok(result)
}

#[cfg(windows)]
fn create_link(target: &Path, link: &Path, directory: bool) -> std::io::Result<()> {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    if directory {
        symlink_dir(target, link)
    } else {
        symlink_file(target, link)
    }
}

#[cfg(unix)]
fn create_link(target: &Path, link: &Path, _directory: bool) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privilege_failure_explains_admin_or_developer_mode() {
        let reason = describe_link_create_error(&std::io::Error::from_raw_os_error(1314));
        assert!(reason.contains("관리자 권한 또는 개발자 모드"));
        assert!(reason.contains("1314"));
    }
}
