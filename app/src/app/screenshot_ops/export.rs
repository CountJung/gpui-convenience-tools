use super::*;
use crate::screenshot_tool::{default_save_folder, save_png_new};
use gpui::{ClipboardEntry, ClipboardItem, Image, ImageFormat, PathPromptOptions};
use std::path::PathBuf;

impl AppRoot {
    fn screenshot_folder(&self) -> PathBuf {
        if self.screenshot.config.save_folder.trim().is_empty() {
            default_save_folder()
        } else {
            PathBuf::from(&self.screenshot.config.save_folder)
        }
    }

    pub(crate) fn pick_screenshot_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("스크린샷 기본 저장 폴더".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let response = receiver.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match response {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            this.screenshot.config.save_folder =
                                path.to_string_lossy().into_owned();
                            if let Some(input) = &this.screenshot.folder_input {
                                input.update(cx, |input, cx| {
                                    input.set_value(
                                        this.screenshot.config.save_folder.clone(),
                                        window,
                                        cx,
                                    )
                                });
                            }
                            this.persist_screenshot_config(cx);
                        }
                    }
                    Ok(Ok(None)) => {
                        this.screenshot.status = "폴더 선택 취소 · 설정 변경 없음".into();
                        this.screenshot.failed = false;
                    }
                    _ => this.screenshot_error(ScreenshotError::SaveFailed),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn save_screenshot_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screenshot.busy {
            return;
        }
        let image = match self.selected_screenshot() {
            Ok(image) => image,
            Err(error) => {
                self.screenshot_error(error);
                cx.notify();
                return;
            }
        };
        let receiver = cx.prompt_for_new_path(&self.screenshot_folder(), Some("Screenshot.png"));
        self.screenshot.busy = true;
        let generation = self.screenshot.generation;
        cx.spawn_in(window, async move |this, cx| {
            let response = receiver.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.screenshot.generation != generation {
                    return;
                }
                this.screenshot.busy = false;
                match response {
                    Ok(Ok(Some(path))) => this.write_screenshot(image, path, window, cx),
                    Ok(Ok(None)) => {
                        this.screenshot.status =
                            "PNG 저장 취소 · 이미지 유지 · 파일 변경 없음".into();
                        this.screenshot.failed = false;
                    }
                    _ => this.screenshot_error(ScreenshotError::SaveFailed),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn save_screenshot_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screenshot.busy {
            return;
        }
        // 자동 저장만은 명시적으로 지정한 폴더를 요구한다. 잘못된 기본 경로로 유출하지 않는다.
        if self.screenshot.config.auto_save && self.screenshot.config.save_folder.trim().is_empty()
        {
            self.screenshot_error(ScreenshotError::SaveFailed);
            cx.notify();
            return;
        }
        match self.selected_screenshot() {
            Ok(image) => self.write_screenshot(
                image,
                self.screenshot_folder().join("Screenshot.png"),
                window,
                cx,
            ),
            Err(error) => {
                self.screenshot_error(error);
                cx.notify();
            }
        }
    }

    fn write_screenshot(
        &mut self,
        image: SelectedScreenshot,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            self.screenshot_error(ScreenshotError::SaveFailed);
            cx.notify();
            return;
        }
        self.screenshot.busy = true;
        self.screenshot.failed = false;
        self.screenshot.status = "PNG 저장 중… · 기존 파일은 덮어쓰지 않습니다.".into();
        let generation = self.screenshot.generation;
        let task = cx.background_executor().spawn(async move {
            let image = image.materialize()?;
            save_png_new(&image, &path)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.screenshot.generation != generation {
                    return;
                }
                this.screenshot.busy = false;
                match result {
                    Ok(path) => {
                        this.screenshot.status = format!(
                            "PNG 저장 완료 · {} · 기존 파일 덮어쓰기 없음",
                            path.display()
                        );
                        this.screenshot.failed = false;
                    }
                    Err(error) => this.screenshot_error(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn copy_screenshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screenshot.busy {
            return;
        }
        let image = match self.selected_screenshot() {
            Ok(image) => image,
            Err(error) => {
                self.screenshot_error(error);
                cx.notify();
                return;
            }
        };
        let generation = self.screenshot.generation;
        self.screenshot.busy = true;
        self.screenshot.failed = false;
        self.screenshot.status = "클립보드용 PNG 준비 중…".into();
        let task = cx.background_executor().spawn(async move {
            let image = image.materialize()?;
            let item = ClipboardItem::new_image(&Image::from_bytes(ImageFormat::Png, image.png()?));
            Ok::<_, ScreenshotError>((image, item))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let response = this.update_in(cx, |this, _, cx| {
                if this.screenshot.generation != generation {
                    return None;
                }
                match result {
                    Ok((image, item)) => {
                        // TestAppContext는 mock clipboard를 사용하므로 실제 OS 클립보드와 분리된다.
                        // GPUI Windows 구현은 여기서 PNG를 재인코딩하므로 이 OS 경계의 UI 지연은 남는다.
                        cx.write_to_clipboard(item);
                        Some((image, cx.read_from_clipboard()))
                    }
                    Err(error) => {
                        this.screenshot.busy = false;
                        this.screenshot_error(error);
                        cx.notify();
                        None
                    }
                }
            });
            let Ok(Some((image, copied))) = response else {
                return;
            };
            // 압축 바이트 비교는 서로 다른 정상 PNG를 실패로 오판한다. 디코딩·픽셀 비교도 UI에서 제외한다.
            let confirmed = cx
                .background_executor()
                .spawn(async move {
                    copied.is_some_and(|item| {
                        item.entries().iter().any(|entry| {
                            matches!(entry, ClipboardEntry::Image(copied)
                        if copied.format == ImageFormat::Png && image.matches_png(copied.bytes()))
                        })
                    })
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if this.screenshot.generation != generation {
                    return;
                }
                this.screenshot.busy = false;
                if confirmed {
                    this.screenshot.failed = false;
                    this.screenshot.status =
                        "클립보드 PNG 이미지 전송 확인 · 파일은 저장하지 않았습니다.".into();
                } else {
                    this.screenshot_error(ScreenshotError::ClipboardFailed);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
