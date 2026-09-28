//! 원본 폴더에서 제외 대상을 고르기 위한 한 단계 탐색.
//! 숨김·시스템 항목도 열거하고, 링크·정션 안쪽으로는 들어가지 않는다.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use super::{has_hidden_or_system_attribute, links};

#[derive(Clone, Debug)]
pub struct BrowseEntry {
    pub name: String,
    pub relative_path: String,
    pub is_directory: bool,
    pub is_link: bool,
    pub is_hidden_or_system: bool,
}

pub fn list(source: &Path, relative: &Path) -> Result<Vec<BrowseEntry>, String> {
    if !relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err("원본 내부의 폴더만 탐색할 수 있습니다.".to_string());
    }
    let root =
        fs::canonicalize(source).map_err(|err| format!("원본 폴더를 열 수 없습니다: {err}"))?;
    let directory = source.join(relative);
    let resolved = fs::canonicalize(&directory)
        .map_err(|err| format!("선택한 폴더를 열 수 없습니다: {err}"))?;
    if !resolved.starts_with(&root) {
        return Err("원본 폴더 밖의 경로는 탐색할 수 없습니다.".to_string());
    }
    // 일반 폴더를 지나갈 때도 이미 있는 정션·심볼릭 링크를 통과하지 않는다.
    let mut cursor = source.to_path_buf();
    for component in relative.components() {
        cursor.push(component.as_os_str());
        let meta = fs::symlink_metadata(&cursor).map_err(|err| err.to_string())?;
        if links::is_link(&meta) {
            return Err("링크·정션 내부는 탐색하지 않습니다.".to_string());
        }
    }

    let mut result = Vec::new();
    let items =
        fs::read_dir(&directory).map_err(|err| format!("폴더 목록을 읽을 수 없습니다: {err}"))?;
    for item in items {
        let item = item.map_err(|err| format!("폴더 항목을 읽을 수 없습니다: {err}"))?;
        let name = item.file_name().to_string_lossy().into_owned();
        let path = item.path();
        let meta = fs::symlink_metadata(&path)
            .map_err(|err| format!("{name} 속성을 읽을 수 없습니다: {err}"))?;
        result.push(BrowseEntry {
            name,
            relative_path: path
                .strip_prefix(source)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
            is_directory: meta.is_dir(),
            is_link: links::is_link(&meta),
            is_hidden_or_system: has_hidden_or_system_attribute(&path, &meta),
        });
    }
    result.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(result)
}

pub fn child_relative(parent: &Path, name: &str) -> Option<PathBuf> {
    let name_path = Path::new(name);
    if name_path.components().count() != 1
        || !matches!(name_path.components().next(), Some(Component::Normal(_)))
    {
        return None;
    }
    Some(parent.join(name))
}
