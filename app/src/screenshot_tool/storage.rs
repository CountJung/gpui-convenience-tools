use super::{ScreenshotError, ScreenshotImage};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) fn default_save_folder() -> PathBuf {
    dirs::picture_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// create_new로 동시 저장에서도 기존 파일을 보호한다. 폴더는 임의로 만들지 않는다.
pub(crate) fn save_png_new(
    image: &ScreenshotImage,
    requested: &Path,
) -> Result<PathBuf, ScreenshotError> {
    let png = image.png()?;
    let requested = requested.with_extension("png");
    let stem = requested.file_stem().ok_or(ScreenshotError::SaveFailed)?;
    for n in 0..1000 {
        let candidate = if n == 0 {
            requested.clone()
        } else {
            let mut name = stem.to_os_string();
            name.push(format!(" ({n}).png"));
            requested.with_file_name(name)
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                return finish_new_file(file, candidate, |file| {
                    file.write_all(&png).and_then(|_| file.sync_all())
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(ScreenshotError::SaveFailed),
        }
    }
    Err(ScreenshotError::NameExhausted)
}

// create_new 직후에 이 함수가 소유하는 새 파일만 받는다.
fn finish_new_file(
    mut file: File,
    candidate: PathBuf,
    write: impl FnOnce(&mut File) -> std::io::Result<()>,
) -> Result<PathBuf, ScreenshotError> {
    if write(&mut file).is_err() {
        drop(file);
        // 이 함수가 새로 만든 불완전 출력만 정리한다. 정리 실패를 성공처럼 숨기지 않는다.
        return match std::fs::remove_file(&candidate) {
            Ok(()) => Err(ScreenshotError::SaveFailed),
            Err(_) => Err(ScreenshotError::PartialFile),
        };
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_failed_write_removes_only_its_new_partial_file() {
        let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-partial");
        let folder = crate::config::data_dir();
        let existing = folder.join("existing.png");
        std::fs::write(&existing, b"untouched").unwrap();
        let path = folder.join("partial.png");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let result = finish_new_file(file, path.clone(), |file| {
            file.write_all(b"partial PNG fixture")?;
            Err(std::io::Error::other("injected fixture write failure"))
        });
        assert_eq!(result, Err(ScreenshotError::SaveFailed));
        assert!(!path.exists());
        assert_eq!(std::fs::read(existing).unwrap(), b"untouched");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn screenshot_failed_partial_cleanup_is_reported_not_hidden() {
        use std::os::windows::fs::OpenOptionsExt;
        let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-partial-lock");
        let path = crate::config::data_dir().join("partial.png");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        // 이 fixture 파일만 delete 공유를 거부하여 정리 실패를 결정적으로 만든다.
        let blocker = OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(&path)
            .unwrap();
        let result = finish_new_file(file, path.clone(), |file| {
            file.write_all(b"partial PNG fixture")?;
            Err(std::io::Error::other("injected fixture write failure"))
        });
        assert_eq!(result, Err(ScreenshotError::PartialFile));
        assert!(path.exists());
        drop(blocker);
        std::fs::remove_file(path).unwrap();
    }
}
