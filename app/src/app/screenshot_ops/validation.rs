//! Release validation용 명시적 합성 PNG 읽기 경계.
//! 부모 검증 시작 처리에서 seed_screenshot_validation_preview(window, cx)를 한 번 호출한다.
//! 실제 화면·클립보드·설정 저장·자동 저장에는 연결하지 않는다.
use super::*;
use image::{codecs::png::PngDecoder, ImageDecoder};
use std::{
    ffi::OsString,
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

const PNG_ENV: &str = "GPUI_CONVENIENCE_TOOLS_VALIDATION_SCREENSHOT_PNG";
const DATA_ENV: &str = "GPUI_CONVENIENCE_TOOLS_DATA_DIR";
const MAX_ENCODED_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SIDE: u32 = 4096;
const MAX_FIXTURE_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = MAX_FIXTURE_PIXELS * 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
enum ValidationError {
    #[error("합성 preview에는 PNG와 DATA_DIR의 명시적 절대 경로가 모두 필요합니다.")]
    Gate,
    #[error("합성 preview PNG는 명시한 DATA_DIR 내부의 일반 PNG 파일이어야 합니다.")]
    Path,
    #[error("합성 preview PNG는 16MiB·4096px/변·8M픽셀 한도를 지켜야 합니다.")]
    Limit,
    #[error("합성 preview PNG를 읽거나 디코딩하지 못했습니다. 기존 이미지는 유지합니다.")]
    Decode,
    #[error("합성 preview 메모리를 준비하지 못했습니다. 기존 이미지는 유지합니다.")]
    Allocation,
}

// Debug를 구현하지 않아 실수로 경로를 로그에 포함하지 않는다.
struct ValidationFixture {
    png: PathBuf,
    data_dir: PathBuf,
}

impl ValidationFixture {
    fn from_values(
        png: Option<OsString>,
        data_dir: Option<OsString>,
    ) -> Result<Option<Self>, ValidationError> {
        let Some(png) = png else {
            return Ok(None);
        };
        let data_dir = data_dir.ok_or(ValidationError::Gate)?;
        let fixture = Self {
            png: PathBuf::from(png),
            data_dir: PathBuf::from(data_dir),
        };
        if !fixture.png.is_absolute() || !fixture.data_dir.is_absolute() {
            return Err(ValidationError::Gate);
        }
        Ok(Some(fixture))
    }

    fn load(self) -> Result<Arc<ScreenshotImage>, ValidationError> {
        // 기본 data_dir/AppData 탐색으로 대체하지 않는다. 두 경로 모두 호출자가 명시한다.
        let root = self
            .data_dir
            .canonicalize()
            .map_err(|_| ValidationError::Path)?;
        let path = self.png.canonicalize().map_err(|_| ValidationError::Path)?;
        if !root.is_dir()
            || root.parent().is_none()
            || !path.starts_with(&root)
            || path == root
            || !path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            return Err(ValidationError::Path);
        }
        let file = File::open(&path).map_err(|_| ValidationError::Path)?;
        verify_opened_path(&file, &path, &root)?;
        let metadata = file.metadata().map_err(|_| ValidationError::Path)?;
        if !metadata.is_file() {
            return Err(ValidationError::Path);
        }
        if metadata.len() == 0 || metadata.len() > MAX_ENCODED_BYTES {
            return Err(ValidationError::Limit);
        }
        // 파일이 읽는 도중 커져도 상한+1 바이트까지만 읽는다.
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact((MAX_ENCODED_BYTES + 1) as usize)
            .map_err(|_| ValidationError::Allocation)?;
        file.take(MAX_ENCODED_BYTES + 1)
            .read_to_end(&mut encoded)
            .map_err(|_| ValidationError::Decode)?;
        decode_png(&encoded).map(Arc::new)
    }
}

#[cfg(target_os = "windows")]
fn verify_opened_path(file: &File, _: &Path, root: &Path) -> Result<(), ValidationError> {
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::GetFinalPathNameByHandleW;

    // canonicalize/open 사이 junction 교체가 있어도 실제 열린 핸들의 경로로 다시 제한한다.
    let mut buffer = vec![0u16; 32768];
    // SAFETY: 파일 핸들은 호출 내내 살아 있고 출력 버퍼의 정확한 용량을 전달한다.
    let len = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle() as isize,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            0,
        )
    } as usize;
    if len == 0 || len >= buffer.len() {
        return Err(ValidationError::Path);
    }
    let opened = PathBuf::from(OsString::from_wide(&buffer[..len]));
    if !opened.starts_with(root) || opened == root {
        return Err(ValidationError::Path);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn verify_opened_path(_: &File, path: &Path, root: &Path) -> Result<(), ValidationError> {
    // Windows release 전용 seam. 다른 플랫폼에서도 링크 탈출을 기본적으로 거부한다.
    if !path
        .canonicalize()
        .map_err(|_| ValidationError::Path)?
        .starts_with(root)
    {
        return Err(ValidationError::Path);
    }
    Ok(())
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ValidationError> {
    if width == 0
        || height == 0
        || width > MAX_SIDE
        || height > MAX_SIDE
        || u64::from(width) * u64::from(height) > MAX_FIXTURE_PIXELS
    {
        return Err(ValidationError::Limit);
    }
    Ok(())
}

fn decode_png(encoded: &[u8]) -> Result<ScreenshotImage, ValidationError> {
    if encoded.len() as u64 > MAX_ENCODED_BYTES {
        return Err(ValidationError::Limit);
    }
    // PNG 서명과 첫 IHDR의 크기만 먼저 확인한다. CRC/압축 데이터 검증은 decoder가 한다.
    if encoded.len() < 33
        || &encoded[..8] != b"\x89PNG\r\n\x1a\n"
        || encoded[8..12] != 13u32.to_be_bytes()
        || &encoded[12..16] != b"IHDR"
    {
        return Err(ValidationError::Decode);
    }
    validate_dimensions(
        u32::from_be_bytes(
            encoded[16..20]
                .try_into()
                .map_err(|_| ValidationError::Decode)?,
        ),
        u32::from_be_bytes(
            encoded[20..24]
                .try_into()
                .map_err(|_| ValidationError::Decode)?,
        ),
    )?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    let decoder = PngDecoder::with_limits(Cursor::new(encoded), limits)
        .map_err(|_| ValidationError::Decode)?;
    let (width, height) = decoder.dimensions();
    // 출력 픽셀 및 RGBA 변환 할당 전에 곱셈 한도까지 확인한다.
    validate_dimensions(width, height)?;
    if decoder.total_bytes() > MAX_DECODE_BYTES {
        return Err(ValidationError::Limit);
    }
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| ValidationError::Decode)?
        .into_rgba8();
    ScreenshotImage::new(width, height, image.into_raw()).map_err(|_| ValidationError::Allocation)
}

impl AppRoot {
    /// 부모가 release 검증 시작 시 명시적으로 연결한다. 일반 실행은 PNG env가 없어 no-op.
    /// true는 요청을 소비했음을 뜻하며 비동기 완료/성공을 보장하지 않는다.
    pub(crate) fn seed_screenshot_validation_preview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.screenshot.validation_seed_attempted
            || self.screenshot.busy
            || self.screenshot.image.is_some()
        {
            return false;
        }
        match ValidationFixture::from_values(std::env::var_os(PNG_ENV), std::env::var_os(DATA_ENV))
        {
            Ok(None) => false,
            Ok(Some(fixture)) => {
                self.start_screenshot_validation_preview(fixture, window, cx);
                true
            }
            Err(error) => {
                self.screenshot.validation_seed_attempted = true;
                self.screenshot.failed = true;
                self.screenshot.status = error.to_string();
                cx.notify();
                true
            }
        }
    }

    fn start_screenshot_validation_preview(
        &mut self,
        fixture: ValidationFixture,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.screenshot.validation_seed_attempted = true;
        self.screenshot.busy = true;
        self.screenshot.failed = false;
        self.screenshot.status = "합성 PNG preview 준비 중 · 화면 캡처/자동 저장 없음".into();
        self.screenshot.generation = self.screenshot.generation.wrapping_add(1);
        let generation = self.screenshot.generation;
        let task = cx.background_executor().spawn(async move {
            PreparedScreenshot::new(fixture.load()?).map_err(|_| ValidationError::Allocation)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.screenshot.generation != generation {
                    return;
                }
                this.screenshot.busy = false;
                match result {
                    Ok(prepared) => {
                        this.apply_screenshot(prepared, window, cx);
                        this.screenshot.status =
                            format!("합성 검증 PNG · {}", this.screenshot.status);
                    }
                    Err(error) => {
                        this.screenshot.failed = true;
                        this.screenshot.status = error.to_string();
                    }
                }
                // persist/capture/export/clipboard 경로를 호출하지 않는다.
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
