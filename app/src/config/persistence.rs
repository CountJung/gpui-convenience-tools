//! 사용자 설정의 단일 저장 경로. 구형 파일은 읽기만 하고 새 값은 실행파일 옆에 저장한다.

use anyhow::{anyhow, Context, Result};
use gpui_component::theme::ThemeMode;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
};

use super::{data_dir, AppConfig, DATA_DIR_ENV, LEGACY_CONFIG_FILE_NAME, SETTINGS_FILE_NAME};

static CONFIG_IO_LOCK: Mutex<()> = Mutex::new(());
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 사용자가 직접 확인·수정할 수 있는 공개 설정파일 경로.
pub fn config_path() -> PathBuf {
    if has_data_dir_override() {
        return data_dir().join(SETTINGS_FILE_NAME);
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join(SETTINGS_FILE_NAME)))
        .unwrap_or_else(|| data_dir().join(SETTINGS_FILE_NAME))
}

pub fn fallback_config_path() -> PathBuf {
    data_dir().join(LEGACY_CONFIG_FILE_NAME)
}

/// 새 설정파일이 있으면 구형 파일의 수정시각과 관계없이 항상 새 설정을 읽는다.
pub fn effective_config_path() -> PathBuf {
    let primary = config_path();
    preferred_config_path(primary, fallback_config_path())
}

fn preferred_config_path(primary: PathBuf, fallback: PathBuf) -> PathBuf {
    if primary.exists() || !fallback.exists() {
        primary
    } else {
        fallback
    }
}

fn has_data_dir_override() -> bool {
    std::env::var_os(DATA_DIR_ENV).is_some_and(|value| !value.is_empty())
}

pub fn open_config_file() -> Result<()> {
    let path = effective_config_path();
    #[cfg(target_os = "windows")]
    Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()?;
    #[cfg(target_os = "macos")]
    Command::new("open").arg(&path).spawn()?;
    #[cfg(all(unix, not(target_os = "macos")))]
    Command::new("xdg-open").arg(&path).spawn()?;
    Ok(())
}

pub fn load_config() -> Result<Option<AppConfig>> {
    let _guard = CONFIG_IO_LOCK
        .lock()
        .map_err(|_| anyhow!("설정 잠금 실패"))?;
    load_config_unlocked()
}

fn load_config_unlocked() -> Result<Option<AppConfig>> {
    let path = effective_config_path();
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path)
        .with_context(|| format!("설정 파일 읽기 실패: {}", path.display()))?;
    decode_config(&data, &path, path == fallback_config_path()).map(Some)
}

fn decode_config(data: &str, path: &Path, legacy: bool) -> Result<AppConfig> {
    match serde_json::from_str::<AppConfig>(data) {
        Ok(config) => Ok(config),
        Err(original_error) if legacy => {
            // 구버전 파일의 맨 끝에 불필요한 닫는 중괄호만 붙은 경우만 읽기 복구한다.
            // 원본은 수정하지 않으며 다음 저장에서 새 settings.json으로 이관된다.
            let mut stream = serde_json::Deserializer::from_str(data).into_iter::<AppConfig>();
            if let Some(Ok(config)) = stream.next() {
                let tail = &data[stream.byte_offset()..];
                if tail.chars().any(|ch| ch == '}')
                    && tail.chars().all(|ch| ch.is_ascii_whitespace() || ch == '}')
                {
                    log::warn!(
                        "구형 설정의 끝에 불필요한 괄호가 있어 읽기 복구했습니다. 원본은 유지합니다: {}",
                        path.display()
                    );
                    return Ok(config);
                }
            }
            Err(anyhow!(
                "설정 JSON 오류: {} ({original_error}); 원본을 보존하고 저장을 중단합니다",
                path.display()
            ))
        }
        Err(error) => Err(anyhow!(
            "설정 JSON 오류: {} ({error}); 기존 설정을 덮어쓰지 않습니다",
            path.display()
        )),
    }
}

#[cfg(test)]
pub fn save_config(config: &AppConfig) -> Result<()> {
    let _guard = CONFIG_IO_LOCK
        .lock()
        .map_err(|_| anyhow!("설정 잠금 실패"))?;
    save_config_unlocked(config)
}

fn save_config_unlocked(config: &AppConfig) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    let primary = config_path();
    match write_config_file(&primary, &json) {
        Ok(()) => Ok(()),
        Err(primary_error) if !has_data_dir_override() && !primary.exists() => {
            let fallback = fallback_config_path();
            if fallback.exists() {
                // 복구해 읽은 손상 구형 파일이라도 원본을 덮어쓰지 않는다.
                let existing = fs::read_to_string(&fallback)?;
                serde_json::from_str::<AppConfig>(&existing).with_context(|| {
                    format!(
                        "기본 설정 저장 실패: {primary_error}; 구형 설정 {}은 손상되어 원본을 보존합니다",
                        fallback.display()
                    )
                })?;
            }
            log::warn!("실행파일 옆 설정 저장 실패, AppData 대체 경로 사용: {primary_error}");
            write_config_file(&fallback, &json)
        }
        Err(error) => Err(error),
    }
}

fn write_config_file(path: &Path, json: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .ok_or_else(|| anyhow!("설정 파일 이름이 없습니다"))?;
    let temporary = path.with_file_name(format!(
        "{}.tmp-{}-{}",
        name.to_string_lossy(),
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);
        replace_config_file(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("설정 저장 실패: {}", path.display()))
}

#[cfg(target_os = "windows")]
fn replace_config_file(temporary: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

    if !destination.exists() {
        fs::rename(temporary, destination)?;
        return Ok(());
    }
    let old = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let new = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: 두 UTF-16 경로는 호출 기간 살아 있고, 동일 폴더의 완성된 임시 파일로 교체한다.
    if unsafe {
        ReplaceFileW(
            old.as_ptr(),
            new.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_config_file(temporary: &Path, destination: &Path) -> Result<()> {
    fs::rename(temporary, destination)?;
    Ok(())
}

/// 다른 설정을 보존하며 사용자 설정 하나를 변경한다. 프로세스 내부의 동시 쓰기는 직렬화한다.
pub fn update_config(edit: impl FnOnce(&mut AppConfig)) -> Result<AppConfig> {
    let _guard = CONFIG_IO_LOCK
        .lock()
        .map_err(|_| anyhow!("설정 잠금 실패"))?;
    let mut config = load_config_unlocked()?.unwrap_or_default();
    edit(&mut config);
    save_config_unlocked(&config)?;
    Ok(config)
}

pub fn save_theme_selection(mode: ThemeMode, theme_name: &str) -> Result<()> {
    update_config(|config| {
        config.theme_mode = Some(mode);
        match mode {
            ThemeMode::Light => config.light_theme_name = Some(theme_name.to_string()),
            ThemeMode::Dark => config.dark_theme_name = Some(theme_name.to_string()),
        }
    })?;
    Ok(())
}

pub fn save_theme_mode(mode: ThemeMode) -> Result<()> {
    update_config(|config| config.theme_mode = Some(mode))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_settings_keep_priority_even_if_legacy_is_newer() {
        let root = std::env::temp_dir().join(format!(
            "gct-preferred-config-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let primary = root.join("settings.json");
        let legacy = root.join("config.json");
        fs::write(&legacy, "legacy").unwrap();
        assert_eq!(
            preferred_config_path(primary.clone(), legacy.clone()),
            legacy
        );
        fs::write(&primary, "primary").unwrap();
        fs::write(&legacy, "newer legacy").unwrap();
        assert_eq!(
            preferred_config_path(primary.clone(), legacy.clone()),
            primary
        );
        fs::remove_file(primary).unwrap();
        fs::remove_file(legacy).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
