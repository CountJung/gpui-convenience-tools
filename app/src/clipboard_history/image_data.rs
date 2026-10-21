use super::MAX_IMAGE_BYTES;
use std::io::Cursor;

/// PNG만 허용한다. SVG/외부 참조·압축 폭탄을 열지 않으며 디코더 한도를 먼저 적용한다.
pub fn normalize_png(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), &'static str> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("이미지 크기 한도 초과");
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().map_err(|_| "PNG 디코딩 실패")?;
    let (width, height) = (image.width(), image.height());
    if u64::from(width) * u64::from(height) > 16 * 1024 * 1024 {
        return Err("이미지 픽셀 한도 초과");
    }
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|_| "PNG 변환 실패")?;
    let png = png.into_inner();
    if png.len() > MAX_IMAGE_BYTES {
        return Err("PNG 크기 한도 초과");
    }
    Ok((png, width, height))
}
