//! 메모리 이미지, PNG, 미리보기 좌표의 순수 처리. OS 캡처는 platform 하위에 격리한다.
mod geometry;
pub(crate) mod platform;
mod storage;

pub(crate) use geometry::PreviewRect;
pub(crate) use storage::{default_save_folder, save_png_new};

use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

pub(crate) const MAX_PIXELS: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ScreenshotError {
    #[cfg(not(target_os = "windows"))]
    #[error("Windows에서만 수동 화면 캡처를 지원합니다.")]
    Unsupported,
    #[error("캡처 크기가 잘못되었거나 64M 픽셀 한도를 넘었습니다.")]
    InvalidDimensions,
    #[error("이미지 메모리를 확보하지 못했습니다. 캡처 범위를 줄여 주세요.")]
    Allocation,
    #[error("선택 영역이 비어 있습니다. 이미지 안쪽을 드래그해 주세요.")]
    EmptySelection,
    #[error("선택한 모니터를 찾을 수 없습니다. 모니터 목록을 새로 고쳐 주세요.")]
    MonitorUnavailable,
    #[error("모니터 식별자 또는 배치가 변경되었습니다. 목록을 새로 고치고 캡처 대상을 다시 선택해 주세요.")]
    MonitorChanged,
    #[error("화면 캡처에 실패했습니다. 잠금/보호된 화면 또는 원격 세션 상태를 확인해 주세요.")]
    CaptureFailed,
    #[error("DPI 좌표 모드를 설정하지 못했습니다. 캡처를 실행하지 않았습니다.")]
    DpiFailed,
    #[error("PNG 변환에 실패했습니다. 이미지는 메모리에 유지됩니다.")]
    PngFailed,
    #[error("PNG 저장에 실패했습니다. 저장 공간·폴더·쓰기 권한을 확인해 주세요.")]
    SaveFailed,
    #[error(
        "PNG 저장과 불완전 파일 정리에 실패했습니다. 선택한 저장 폴더의 새 파일을 확인해 주세요."
    )]
    PartialFile,
    #[error("같은 이름이 너무 많습니다. 다른 파일명을 선택해 주세요.")]
    NameExhausted,
    #[error("먼저 수동 캡처를 실행해 주세요.")]
    NoImage,
    #[error(
        "클립보드 이미지 전송을 확인하지 못했습니다. 다른 앱의 점유를 해제하고 다시 시도해 주세요."
    )]
    ClipboardFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// 픽셀은 항상 위에서 아래 순서의 RGBA8이다. GDI와 렌더러 경계에서만 BGRA로 바꾼다.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ScreenshotImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

pub(crate) fn byte_len(width: u32, height: u32) -> Result<usize, ScreenshotError> {
    let count = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || count > MAX_PIXELS {
        return Err(ScreenshotError::InvalidDimensions);
    }
    usize::try_from(count * 4).map_err(|_| ScreenshotError::InvalidDimensions)
}

pub(crate) fn zeroed_pixels(width: u32, height: u32) -> Result<Vec<u8>, ScreenshotError> {
    let len = byte_len(width, height)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| ScreenshotError::Allocation)?;
    bytes.resize(len, 0);
    Ok(bytes)
}

impl ScreenshotImage {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, ScreenshotError> {
        if rgba.len() != byte_len(width, height)? {
            return Err(ScreenshotError::InvalidDimensions);
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn crop(&self, rect: PixelRect) -> Result<Self, ScreenshotError> {
        if rect.width == 0 || rect.height == 0 || rect.x >= self.width || rect.y >= self.height {
            return Err(ScreenshotError::EmptySelection);
        }
        let width = rect.width.min(self.width - rect.x);
        let height = rect.height.min(self.height - rect.y);
        let mut bytes = zeroed_pixels(width, height)?;
        for row in 0..height as usize {
            let start = ((rect.y as usize + row) * self.width as usize + rect.x as usize) * 4;
            let out = row * width as usize * 4;
            let len = width as usize * 4;
            bytes[out..out + len].copy_from_slice(&self.rgba[start..start + len]);
        }
        Self::new(width, height, bytes)
    }

    pub fn png(&self) -> Result<Vec<u8>, ScreenshotError> {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                &self.rgba,
                self.width,
                self.height,
                ExtendedColorType::Rgba8,
            )
            .map_err(|_| ScreenshotError::PngFailed)?;
        Ok(png)
    }

    /// PNG 압축 방식이 달라도 전송된 실제 픽셀이 같은지 확인한다. 작업 스레드에서 호출한다.
    pub fn matches_png(&self, png: &[u8]) -> bool {
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(self.width);
        limits.max_image_height = Some(self.height);
        limits.max_alloc = Some(MAX_PIXELS * 4);
        reader.limits(limits);
        reader.decode().is_ok_and(|image| {
            image.width() == self.width
                && image.height() == self.height
                && image.into_rgba8().as_raw() == &self.rgba
        })
    }

    pub fn render_image(&self) -> Result<std::sync::Arc<gpui::RenderImage>, ScreenshotError> {
        let mut bgra = zeroed_pixels(self.width, self.height)?;
        bgra.copy_from_slice(&self.rgba);
        for pixel in bgra.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let buffer = image::RgbaImage::from_raw(self.width, self.height, bgra)
            .ok_or(ScreenshotError::InvalidDimensions)?;
        Ok(std::sync::Arc::new(gpui::RenderImage::new([
            image::Frame::new(buffer),
        ])))
    }
}

#[cfg(test)]
mod tests;
