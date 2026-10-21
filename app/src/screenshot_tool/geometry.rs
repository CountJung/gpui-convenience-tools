use super::{PixelRect, ScreenshotError};

/// 실제 이미지 표시 영역(논리 px). 컨테이너 여백과 DPI 배율을 픽셀 좌표에 섞지 않는다.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PreviewRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl PreviewRect {
    pub fn fit(self, image_width: u32, image_height: u32) -> Option<Self> {
        if image_width == 0
            || image_height == 0
            || self.width <= 0.0
            || self.height <= 0.0
            || ![self.left, self.top, self.width, self.height]
                .iter()
                .all(|v| v.is_finite())
        {
            return None;
        }
        let scale = (self.width / image_width as f32).min(self.height / image_height as f32);
        let width = image_width as f32 * scale;
        let height = image_height as f32 * scale;
        Some(Self {
            left: self.left + (self.width - width) / 2.0,
            top: self.top + (self.height - height) / 2.0,
            width,
            height,
        })
    }

    pub fn contains(self, point: (f32, f32)) -> bool {
        point.0 >= self.left
            && point.0 <= self.left + self.width
            && point.1 >= self.top
            && point.1 <= self.top + self.height
    }

    pub fn selection(
        self,
        start: (f32, f32),
        end: (f32, f32),
        width: u32,
        height: u32,
    ) -> Result<PixelRect, ScreenshotError> {
        if width == 0
            || height == 0
            || self.width <= 0.0
            || self.height <= 0.0
            || ![
                self.left,
                self.top,
                self.width,
                self.height,
                start.0,
                start.1,
                end.0,
                end.1,
            ]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(ScreenshotError::EmptySelection);
        }
        // f32의 0.6 * 200 같은 중간 오차가 ceil에서 1px을 더 선택하지 않게 한다.
        let x = |v: f32| {
            ((f64::from(v) - f64::from(self.left)) / f64::from(self.width) * f64::from(width))
                .clamp(0.0, f64::from(width))
        };
        let y = |v: f32| {
            ((f64::from(v) - f64::from(self.top)) / f64::from(self.height) * f64::from(height))
                .clamp(0.0, f64::from(height))
        };
        if start.0 == end.0 || start.1 == end.1 {
            return Err(ScreenshotError::EmptySelection);
        }
        let left = x(start.0.min(end.0)).floor() as u32;
        let top = y(start.1.min(end.1)).floor() as u32;
        let right = x(start.0.max(end.0)).ceil() as u32;
        let bottom = y(start.1.max(end.1)).ceil() as u32;
        if right <= left || bottom <= top {
            return Err(ScreenshotError::EmptySelection);
        }
        Ok(PixelRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }
}
