//! 이력의 GPUI 연결. 검색·선택·내용은 config에 넣지 않으며 기본 테스트는 메모리만 쓴다.
use super::AppRoot;
use crate::{
    clipboard_history::{
        self as history, Candidate, CaptureResult, Content, History, HistoryStore,
    },
    config::{update_config, ClipboardConfig},
};
#[cfg(test)]
use gpui::ClipboardEntry;
use gpui::{
    AppContext, ClipboardItem, Context, Entity, Image, ImageFormat, ScrollHandle, Subscription,
    Window,
};
use gpui_component::input::{InputEvent, InputState};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod background;
#[cfg(test)]
mod tests;
use background::ClipboardJob;

#[derive(Default)]
pub(crate) struct ClipboardState {
    pub config: ClipboardConfig,
    pub history: History,
    pub save_error: bool,
    pub status: String,
    pub failed: bool,
    pub scroll: ScrollHandle,
    pub search: Option<Entity<InputState>>,
    pub exclusions: Option<Entity<InputState>>,
    pub limits: Option<Entity<InputState>>,
    pub query: String,
    pub selected: Option<u64>,
    pub selected_image: Option<Arc<Image>>,
    pub confirm_clear: bool,
    pub busy: bool,
    subscriptions: Vec<Subscription>,
    store: Option<HistoryStore>,
    loaded: bool,
    blocked: bool,
    last_poll: Option<Instant>,
    last_sequence: Option<u32>,
    last_fingerprint: Option<String>,
    image_cache: HashMap<String, Vec<u8>>,
}

impl ClipboardState {
    pub fn from_config(config: ClipboardConfig) -> Self {
        Self {
            config: config.normalized(),
            ..Default::default()
        }
    }

    pub fn visible_entries(&self) -> impl Iterator<Item = &history::Entry> {
        self.history
            .entries
            .iter()
            .filter(|entry| entry.matches(&self.query))
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl AppRoot {
    fn load_clipboard_history(&mut self, cx: &mut Context<Self>) {
        if self.clipboard.loaded {
            return;
        }
        self.clipboard.loaded = true;
        #[cfg(not(test))]
        {
            self.clipboard.store = Some(HistoryStore::new(&crate::config::data_dir()));
        }
        if let Some(store) = self.clipboard.store.clone() {
            self.begin_clipboard_job(ClipboardJob::Load(store, self.clipboard.config.clone()), cx);
        }
    }

    fn clipboard_failure(&mut self, message: &str) {
        self.clipboard.failed = true;
        self.clipboard.status = message.to_string();
        // 경로·본문·제외 문구·검색어·OS 오류 원문은 기록하지 않는다.
        log::error!("클립보드 이력 작업 실패");
    }

    fn persist_clipboard_history(&mut self, new_image: Option<&[u8]>) -> bool {
        if self.clipboard.blocked {
            return false;
        }
        if let Some(store) = &self.clipboard.store {
            if let Err(error) = store.save(&self.clipboard.history, new_image) {
                // 실패한 정리/디스크 쓰기 뒤 추가 수집으로 실효 보관 용량을 늘리지 않는다.
                self.clipboard.blocked = true;
                self.clipboard_failure(&format!("{error} · 안전을 위해 수집 중지. 권한 확인 후 로컬 초기화로 재개할 수 있습니다."));
                return false;
            }
        }
        true
    }

    pub(crate) fn ensure_clipboard_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_clipboard_history(cx);
        if self.clipboard.search.is_some() {
            return;
        }
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("최근 항목 검색 · 저장하지 않음")
                .validate(|text, _| text.len() <= 1024)
        });
        let subscription = cx.subscribe_in(
            &search,
            window,
            |this: &mut Self, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.clipboard.query = input.read(cx).value().to_string();
                    this.clipboard.selected = None;
                    this.clipboard.selected_image = None;
                    this.clipboard.confirm_clear = false;
                    cx.notify();
                }
            },
        );
        self.clipboard.subscriptions.push(subscription);
        self.clipboard.search = Some(search);
        let exclusions = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(2)
                .placeholder("제외 문구 · 한 줄에 하나 · 대소문자 구분 없음")
                .default_value(self.clipboard.config.excluded_phrases.join("\n"))
        });
        self.clipboard.exclusions = Some(exclusions);
        let config = &self.clipboard.config;
        let limits = format!(
            "{} {} {}",
            config.max_entries,
            config.max_age_hours,
            config.max_bytes / 1024
        );
        self.clipboard.limits = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(limits)
                .placeholder("항목 수 / 보관 시간 / 총 KiB (공백 구분)")
                .validate(|text, _| text.len() <= 80)
        }));
    }

    pub(crate) fn persist_clipboard_config(&mut self, cx: &mut Context<Self>) {
        self.clipboard.config = self.clipboard.config.clone().normalized();
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            cx.notify();
            return;
        }
        let config = self.clipboard.config.clone();
        self.clipboard.save_error = update_config(|cfg| cfg.clipboard = config).is_err();
        if self.clipboard.save_error {
            log::error!("클립보드 설정 저장 실패");
        }
        cx.notify();
    }

    pub(crate) fn set_clipboard_collection(
        &mut self,
        images: bool,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if images {
            self.clipboard.config.collect_images = enabled;
        } else {
            self.clipboard.config.collect_text = enabled;
        }
        // 사용자가 opt-in하기 전에 있던 현재 클립보드는 다음 tick에서 수집할 수 있다.
        self.clipboard.last_fingerprint = None;
        self.clipboard.last_sequence = None;
        self.clipboard.confirm_clear = false;
        self.persist_clipboard_config(cx);
    }

    pub(crate) fn set_clipboard_sensitive(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.clipboard_job_pending(cx) {
            return;
        }
        self.clipboard.config.exclude_sensitive = enabled;
        self.persist_clipboard_config(cx);
    }

    pub(crate) fn apply_clipboard_policy(&mut self, cx: &mut Context<Self>) {
        if self.clipboard_job_pending(cx) {
            return;
        }
        let text = self
            .clipboard
            .limits
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let values = text
            .split_whitespace()
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>();
        let Ok(values) = values else {
            self.clipboard_failure("한도는 항목 수·보관 시간·총 KiB의 정수 3개를 입력하세요.");
            cx.notify();
            return;
        };
        if values.len() != 3
            || !(1..=500).contains(&values[0])
            || !(1..=720).contains(&values[1])
            || !(64..=16384).contains(&values[2])
        {
            self.clipboard_failure(
                "허용 한도: 1~500개 · 1~720시간 · 64~16384 KiB. 고정 항목에도 적용됩니다.",
            );
            cx.notify();
            return;
        }
        self.clipboard.config.max_entries = values[0] as usize;
        self.clipboard.config.max_age_hours = values[1];
        self.clipboard.config.max_bytes = values[2] * 1024;
        self.clipboard.config.excluded_phrases = self
            .clipboard
            .exclusions
            .as_ref()
            .map(|input| input.read(cx).value().lines().map(str::to_string).collect())
            .unwrap_or_default();
        self.clipboard.failed = false;
        self.clipboard.status =
            "보관 정책을 적용했습니다. 고정 항목도 기간·총 한도의 적용을 받습니다.".into();
        self.clipboard.history.prune(&self.clipboard.config, now());
        self.trim_clipboard_cache();
        self.persist_clipboard_history(None);
        self.persist_clipboard_config(cx);
    }

    fn trim_clipboard_cache(&mut self) {
        let history = &self.clipboard.history;
        self.clipboard.image_cache.retain(|hash, _| {
            history.entries.iter().any(
                |entry| matches!(&entry.content, Content::Png { digest, .. } if digest == hash),
            )
        });
        if !history
            .entries
            .iter()
            .any(|entry| Some(entry.id) == self.clipboard.selected)
        {
            self.clipboard.selected = None;
            self.clipboard.selected_image = None;
        }
    }

    /// 메인 UI 타이머에서 호출한다. 패널을 열지 않아도 복원된 opt-in 정책을 사용한다.
    pub(crate) fn tick_clipboard(&mut self, cx: &mut Context<Self>) {
        // 느린 디스크/이미지 작업이 끝나기 전에는 새 작업·클립보드 스냅샷을 쌓지 않는다.
        if self.clipboard.busy {
            return;
        }
        if self
            .clipboard
            .last_poll
            .is_some_and(|last| last.elapsed() < Duration::from_millis(750))
        {
            return;
        }
        self.clipboard.last_poll = Some(Instant::now());
        self.load_clipboard_history(cx);
        if self.clipboard.blocked || self.clipboard.busy {
            return;
        }
        let mut pruned = self.clipboard.history.clone();
        if pruned.prune(&self.clipboard.config, now()) {
            self.begin_clipboard_job(
                ClipboardJob::Prune(self.clipboard.store.clone(), pruned),
                cx,
            );
            return;
        }
        if !self.clipboard.config.enabled() {
            return;
        }
        let sequence = match history::windows::probe() {
            history::windows::Probe::Allowed(sequence) => sequence,
            history::windows::Probe::Sensitive => {
                self.clipboard.status = CaptureResult::Sensitive.message().into();
                self.clipboard.last_fingerprint = None;
                // 암호창에서 포커스를 옮긴 뒤 같은 스냅샷이 뒤늦게 수집되는 것을 막는다.
                self.clipboard.last_sequence = Some(history::windows::sequence());
                cx.notify();
                return;
            }
            history::windows::Probe::Unavailable => {
                self.clipboard.status =
                    "클립보드 접근 또는 OS 민감 마커 확인 불가 · 안전을 위해 수집하지 않습니다."
                        .into();
                cx.notify();
                return;
            }
        };
        if sequence != 0 && self.clipboard.last_sequence == Some(sequence) {
            return;
        }
        self.clipboard.last_sequence = Some(sequence);
        #[cfg(not(test))]
        self.begin_clipboard_job(
            ClipboardJob::Read {
                store: self.clipboard.store.clone(),
                history: self.clipboard.history.clone(),
                config: self.clipboard.config.clone(),
                sequence,
                previous_fingerprint: self.clipboard.last_fingerprint.clone(),
            },
            cx,
        );
        // 테스트는 GPUI TestPlatform 메모리만 사용하며 실제 OS 코드는 컴파일하지 않는다.
        #[cfg(test)]
        self.queue_mock_clipboard_snapshot(sequence, cx);
    }

    #[cfg(test)]
    fn queue_mock_clipboard_snapshot(&mut self, sequence: u32, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        // GPUI가 읽는 동안 원본/보호 마커가 바뀌었으면 해당 스냅샷을 저장하지 않는다.
        if history::windows::sequence() != sequence {
            return;
        }
        if history::windows::probe() != history::windows::Probe::Allowed(sequence) {
            return;
        }
        self.clipboard.last_sequence = Some(sequence);
        let candidate = match item.entries().first() {
            Some(ClipboardEntry::String(string)) if self.clipboard.config.collect_text => {
                if string.text().len() > history::MAX_TEXT_BYTES {
                    self.clipboard.status = CaptureResult::TooLarge.message().into();
                    cx.notify();
                    return;
                }
                Candidate::Text(string.text().to_string())
            }
            Some(ClipboardEntry::Image(image))
                if self.clipboard.config.collect_images && image.format == ImageFormat::Png =>
            {
                if image.bytes.len() > history::MAX_IMAGE_BYTES {
                    self.clipboard.status = CaptureResult::TooLarge.message().into();
                    cx.notify();
                    return;
                }
                Candidate::Png(image.bytes.clone())
            }
            Some(ClipboardEntry::Image(_)) if self.clipboard.config.collect_images => {
                self.clipboard.status = CaptureResult::InvalidImage.message().into();
                cx.notify();
                return;
            }
            _ => return,
        };
        self.begin_clipboard_job(
            ClipboardJob::Capture {
                store: self.clipboard.store.clone(),
                history: self.clipboard.history.clone(),
                config: self.clipboard.config.clone(),
                candidate,
                previous_fingerprint: self.clipboard.last_fingerprint.clone(),
            },
            cx,
        );
    }

    // 합성 fixture 시드 전용. 제품의 자동 수집은 background::ClipboardJob만 사용한다.
    #[cfg(test)]
    fn capture_clipboard_candidate(&mut self, candidate: Candidate, cx: &mut Context<Self>) {
        let previous = self.clipboard.history.clone();
        let (result, image) =
            self.clipboard
                .history
                .capture(candidate, false, &self.clipboard.config, now());
        self.clipboard.failed = false;
        self.clipboard.status = result.message().into();
        if result == CaptureResult::Added {
            if self.persist_clipboard_history(image.as_deref()) {
                if let Some(image) = image {
                    self.clipboard
                        .image_cache
                        .insert(history::digest(&image), image);
                }
            } else {
                self.clipboard.history = previous;
                self.clipboard.last_fingerprint = None;
                self.clipboard.last_sequence = None;
            }
            self.trim_clipboard_cache();
        }
        cx.notify();
    }

    pub(crate) fn select_clipboard_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        self.clipboard.selected_image = None;
        let image_hash = self
            .clipboard
            .history
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| match &entry.content {
                Content::Png { digest, .. } => Some(digest.clone()),
                _ => None,
            });
        if let Some(hash) = image_hash {
            match self.read_clipboard_png(&hash) {
                Ok(bytes) => {
                    self.clipboard.selected_image =
                        Some(Arc::new(Image::from_bytes(ImageFormat::Png, bytes)))
                }
                Err(error) => self.clipboard_failure(error),
            }
        }
        self.clipboard.selected = Some(id);
        self.clipboard.confirm_clear = false;
        cx.notify();
    }

    pub(crate) fn pin_clipboard_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.clipboard_job_pending(cx) {
            return;
        }
        let previous = self.clipboard.history.clone();
        if let Some(entry) = self
            .clipboard
            .history
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
        {
            entry.pinned = !entry.pinned;
        }
        if !self.persist_clipboard_history(None) {
            self.clipboard.history = previous;
        }
        cx.notify();
    }

    pub(crate) fn delete_clipboard_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.clipboard_job_pending(cx) {
            return;
        }
        let previous = self.clipboard.history.clone();
        self.clipboard
            .history
            .entries
            .retain(|entry| entry.id != id);
        if !self.persist_clipboard_history(None) {
            self.clipboard.history = previous;
        }
        self.trim_clipboard_cache();
        cx.notify();
    }

    pub(crate) fn clear_clipboard_history(&mut self, cx: &mut Context<Self>) {
        if self.clipboard_job_pending(cx) {
            return;
        }
        if !self.clipboard.confirm_clear {
            self.clipboard.confirm_clear = true;
            cx.notify();
            return;
        }
        if let Some(store) = &self.clipboard.store {
            if let Err(error) = store.clear() {
                self.clipboard_failure(error);
                cx.notify();
                return;
            }
        }
        self.clipboard.history = History::default();
        self.clipboard.image_cache.clear();
        self.clipboard.selected = None;
        self.clipboard.selected_image = None;
        self.clipboard.blocked = false;
        self.clipboard.failed = false;
        self.clipboard.confirm_clear = false;
        self.clipboard.status =
            "로컬 이력을 초기화했습니다. 시스템 클립보드는 변경하지 않았습니다.".into();
        // 현재 OS 스냅샷은 초기화 대상이 아니다. 이 번호 이후의 변경부터 수집한다.
        self.clipboard.last_sequence = Some(history::windows::sequence());
        // 번호가 없는 mock에서도 직전 명시 복사와 같은 본문을 다시 수집하지 않는다.
        cx.notify();
    }

    pub(crate) fn copy_clipboard_entry(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.clipboard_job_pending(cx) {
            return;
        }
        let mut pruned = self.clipboard.history.clone();
        if pruned.prune(&self.clipboard.config, now()) {
            // 메모리만 지우면 다음 tick이 만료를 놓친다. 성공 시에만 worker 결과로 교체한다.
            self.begin_clipboard_job(
                ClipboardJob::Prune(self.clipboard.store.clone(), pruned),
                cx,
            );
            self.clipboard.status = "보관 만료 항목 정리 중 · 완료 후 다시 복사하세요.".into();
            return;
        }
        let Some(entry) = self
            .clipboard
            .history
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
        else {
            self.clipboard_failure("선택한 항목이 없거나 보관 기간이 만료되었습니다.");
            cx.notify();
            return;
        };
        let item = match &entry.content {
            Content::Text { text } => ClipboardItem::new_string(text.clone()),
            Content::Png { digest, .. } => {
                let png = self.read_clipboard_png(digest);
                let png = match png {
                    Ok(png) => png,
                    Err(error) => {
                        self.clipboard_failure(error);
                        cx.notify();
                        return;
                    }
                };
                ClipboardItem::new_image(&Image::from_bytes(ImageFormat::Png, png))
            }
        };
        cx.write_to_clipboard(item);
        // GPUI 0.2.2의 쓰기 API는 성공 여부를 반환하지 않으므로 읽어서 결과를 확인한다.
        #[cfg(test)]
        let success = match (cx.read_from_clipboard(), &entry.content) {
            (Some(actual), Content::Text { text }) => actual.text().as_deref() == Some(text),
            (Some(actual), Content::Png { digest, .. }) => {
                matches!(actual.entries().first(), Some(ClipboardEntry::Image(image)) if history::digest(&image.bytes) == *digest)
            }
            _ => false,
        };
        #[cfg(not(test))]
        let success = {
            // 명시 복사의 확인도 GPUI의 무제한 읽기를 피하고 동일한 bounded 경계를 사용한다.
            let config = ClipboardConfig {
                collect_text: true,
                collect_images: true,
                ..Default::default()
            };
            match (
                history::windows::snapshot(&config, history::windows::sequence()),
                &entry.content,
            ) {
                (Ok(Candidate::Text(actual)), Content::Text { text }) => actual == *text,
                (Ok(Candidate::Png(actual)), Content::Png { digest, .. }) => {
                    history::digest(&actual) == *digest
                }
                _ => false,
            }
        };
        if success {
            self.clipboard.last_sequence = Some(history::windows::sequence());
            self.clipboard.last_fingerprint = Some(match &entry.content {
                Content::Text { text } => format!("text:{}", history::digest(text.as_bytes())),
                Content::Png { digest, .. } => format!("png:{digest}"),
            });
        }
        self.clipboard.failed = !success;
        self.clipboard.status = if success {
            "선택한 항목을 다시 복사했습니다."
        } else {
            "다시 복사 결과를 확인할 수 없습니다. 다른 앱의 클립보드 점유를 확인하세요."
        }
        .into();
        cx.notify();
    }

    fn read_clipboard_png(&self, digest: &str) -> Result<Vec<u8>, &'static str> {
        self.clipboard
            .image_cache
            .get(digest)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| {
                self.clipboard
                    .store
                    .as_ref()
                    .ok_or("이미지 저장소가 없습니다.")
                    .and_then(|store| store.read_png(digest))
            })
    }
}
