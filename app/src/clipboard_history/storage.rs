//! 전용 폴더 안의 index + 해시 이름 PNG만 소유한다. 손상 원본은 명시 초기화 전 보존한다.
use super::{digest, Content, History, MAX_IMAGE_BYTES, MAX_TEXT_BYTES};
use crate::config::ClipboardConfig;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u32,
    history: History,
}

#[derive(Clone)]
pub struct HistoryStore {
    root: PathBuf,
}

impl HistoryStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("clipboard-history"),
        }
    }

    pub fn load(&self, config: &ClipboardConfig, now: u64) -> Result<History, &'static str> {
        let path = self.root.join("index.json");
        if !path.exists() {
            return Ok(History::default());
        }
        let bytes = bounded_read(&path, 100 * 1024 * 1024)?;
        let envelope: Envelope = serde_json::from_slice(&bytes)
            .map_err(|_| "이력 파일 손상 · 원본 보존 중. 초기화 전까지 수집이 중지됩니다.")?;
        if envelope.version != 1 || envelope.history.entries.len() > 500 {
            return Err("지원하지 않는 이력 형식 · 원본을 보존합니다.");
        }
        let mut history = envelope.history;
        let mut ids = std::collections::HashSet::new();
        for entry in &history.entries {
            if entry.id == 0 || !ids.insert(entry.id) || entry.id > history.next_id {
                return Err("이력 식별자 손상 · 원본을 보존합니다.");
            }
            match &entry.content {
                Content::Text { text } if text.len() > MAX_TEXT_BYTES => {
                    return Err("이력 텍스트 한도 초과 · 원본을 보존합니다.")
                }
                Content::Png {
                    digest,
                    width,
                    height,
                    bytes,
                } => {
                    if !valid_digest(digest) || *bytes > MAX_IMAGE_BYTES as u64 || *bytes == 0 {
                        return Err("이미지 이력 형식 손상 · 원본을 보존합니다.");
                    }
                    let png = self.read_png(digest)?;
                    let (_, actual_width, actual_height) = super::normalize_png(&png)?;
                    if png.len() as u64 != *bytes
                        || actual_width != *width
                        || actual_height != *height
                    {
                        return Err("이미지 이력 검증 실패 · 원본을 보존합니다.");
                    }
                }
                _ => {}
            }
        }
        history.prune(config, now);
        Ok(history)
    }

    pub fn read_png(&self, hash: &str) -> Result<Vec<u8>, &'static str> {
        if !valid_digest(hash) {
            return Err("잘못된 이미지 식별자");
        }
        let bytes = bounded_read(
            &self.root.join(format!("{hash}.png")),
            MAX_IMAGE_BYTES as u64,
        )?;
        if digest(&bytes) != hash {
            return Err("이미지 무결성 검증 실패 · 원본을 보존합니다.");
        }
        Ok(bytes)
    }

    /// 이미지를 먼저 완성하고 index를 원자 교체한다. index 실패 시 기존 이력은 보존한다.
    pub fn save(&self, history: &History, new_image: Option<&[u8]>) -> Result<(), &'static str> {
        let index = self.root.join("index.json");
        if index.exists() {
            let bytes = bounded_read(&index, 100 * 1024 * 1024)?;
            let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|_| {
                "이력 파일 손상 · 원본을 보존합니다. 초기화 전까지 저장할 수 없습니다."
            })?;
            if envelope.version != 1 {
                return Err("지원하지 않는 이력 형식 · 원본을 보존합니다.");
            }
        }
        self.save_unchecked(history, new_image)
    }

    fn save_unchecked(
        &self,
        history: &History,
        new_image: Option<&[u8]>,
    ) -> Result<(), &'static str> {
        fs::create_dir_all(&self.root)
            .map_err(|_| "이력 폴더 생성 실패 · 쓰기 권한을 확인하세요.")?;
        if let Some(bytes) = new_image {
            let hash = digest(bytes);
            let path = self.root.join(format!("{hash}.png"));
            if !path.exists() {
                atomic_write(&path, bytes)?;
            } else if self.read_png(&hash)? != bytes {
                return Err("이미지 저장 충돌 · 원본을 보존합니다.");
            }
        }
        let envelope = Envelope {
            version: 1,
            history: history.clone(),
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|_| "이력 직렬화 실패")?;
        atomic_write(&self.root.join("index.json"), &bytes)?;
        self.remove_unreferenced(history)
    }

    fn remove_unreferenced(&self, history: &History) -> Result<(), &'static str> {
        for entry in fs::read_dir(&self.root).map_err(|_| "보관 폴더 조회 실패")? {
            let entry = entry.map_err(|_| "보관 파일 조회 실패")?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(hash) = name.strip_suffix(".png").filter(|hash| valid_digest(hash)) else {
                continue;
            };
            let referenced = history.entries.iter().any(
                |entry| matches!(&entry.content, Content::Png { digest, .. } if digest == hash),
            );
            if !referenced {
                fs::remove_file(entry.path())
                    .map_err(|_| "만료 이미지 정리 실패 · 쓰기 권한을 확인하세요.")?;
            }
        }
        Ok(())
    }

    pub fn clear(&self) -> Result<(), &'static str> {
        self.save_unchecked(&History::default(), None)
    }
}

fn valid_digest(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch))
}

fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>, &'static str> {
    let file = fs::File::open(path).map_err(|_| "보관 파일 읽기 실패 · 접근 권한을 확인하세요.")?;
    if file
        .metadata()
        .map_err(|_| "보관 파일 정보 조회 실패")?
        .len()
        > limit
    {
        return Err("보관 파일 크기 한도 초과 · 원본을 보존합니다.");
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "보관 파일 읽기 실패")?;
    if bytes.len() as u64 > limit {
        return Err("보관 파일 크기 한도 초과");
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    // 고정 임시 이름은 create_new로 잠근다. 다른 프로세스의 임시 원본을 덮어쓰지 않는다.
    let temporary = path.with_extension(format!("{}.pending", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| "임시 이력 생성 실패 · 다른 실행 또는 쓰기 권한을 확인하세요.")?;
    let result = (|| {
        file.write_all(bytes).map_err(|_| "이력 쓰기 실패")?;
        file.sync_all().map_err(|_| "이력 디스크 반영 실패")?;
        drop(file);
        replace(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(target_os = "windows")]
fn replace(temporary: &Path, path: &Path) -> Result<(), &'static str> {
    use std::os::windows::ffi::OsStrExt;
    if !path.exists() {
        return fs::rename(temporary, path).map_err(|_| "이력 파일 이동 실패");
    }
    let old: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let new: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: 호출 동안 살아 있는 NUL 종료 UTF-16 경로, 동일 폴더에서 완성된 파일만 교체한다.
    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            old.as_ptr(),
            new.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result == 0 {
        Err("이력 원자 교체 실패 · 기존 이력을 보존합니다.")
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
fn replace(temporary: &Path, path: &Path) -> Result<(), &'static str> {
    fs::rename(temporary, path).map_err(|_| "이력 원자 교체 실패")
}
