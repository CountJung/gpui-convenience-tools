//! 주기 선택 상태와 조작.
//!
//! 광고 차단 스캔 주기와 파일 동기화 감시 주기는 **하나의 프리셋 목록을 공유**한다.
//! 사용자가 한쪽에서 만든 주기를 다른 쪽에서 다시 만들 이유가 없기 때문이다.
//!
//! 렌더는 [`crate::window::interval`]이 담당한다. 이 모듈은 상태와 조작만 소유한다.

use gpui::{AppContext, Context, Entity, SharedString, Window};
use gpui_component::{
    input::InputState,
    select::{SearchableVec, SelectEvent, SelectItem, SelectState},
    IndexPath,
};

use super::AppRoot;
use crate::config::{normalize_interval_presets, update_config};
use crate::util::{format_interval, interval_to_secs, TimeUnit};

/// 주기 드롭다운의 항목. 값은 초 단위이고 표시는 사람이 읽는 표기다.
#[derive(Clone)]
pub(crate) struct IntervalItem {
    secs: u32,
    label: SharedString,
}

impl IntervalItem {
    fn new(secs: u32) -> Self {
        Self {
            secs,
            label: SharedString::from(format_interval(secs)),
        }
    }
}

impl SelectItem for IntervalItem {
    type Value = u32;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.secs
    }
}

pub(crate) type IntervalSelect = Entity<SelectState<SearchableVec<IntervalItem>>>;
pub(crate) type UnitSelect = Entity<SelectState<SearchableVec<&'static str>>>;

/// 주기 선택 UI가 쓰는 상태 묶음.
///
/// `AppRoot`의 필드 수를 더 늘리지 않도록 한 덩어리로 소유한다.
#[derive(Default)]
pub(crate) struct IntervalPicker {
    /// 공유 프리셋(초). 항상 오름차순·중복 없음.
    pub(crate) presets: Vec<u32>,
    pub(crate) scan_select: Option<IntervalSelect>,
    pub(crate) sync_select: Option<IntervalSelect>,
    pub(crate) amount_input: Option<Entity<InputState>>,
    pub(crate) unit_select: Option<UnitSelect>,
    /// 직접 추가가 거부된 사유. 성공하면 지운다.
    pub(crate) error: Option<String>,
}

/// 어떤 주기를 편집하는지.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntervalTarget {
    /// 광고 차단 스캔 주기.
    Scan,
    /// 선택한 파일 동기화 작업의 감시 주기.
    Sync,
}

impl AppRoot {
    /// 현재 선택값을 포함한 드롭다운 항목 목록.
    ///
    /// 저장된 주기가 프리셋에 없을 수도 있으므로(구버전 config, 작업별 개별 설정)
    /// 항상 현재 값을 합쳐서 드롭다운이 빈 채로 보이지 않게 한다.
    pub(crate) fn interval_options(&self, current: u32) -> Vec<u32> {
        let mut options = self.interval_picker.presets.clone();
        options.push(current);
        normalize_interval_presets(&mut options);
        options
    }

    pub(crate) fn interval_value(&self, target: IntervalTarget) -> u32 {
        match target {
            IntervalTarget::Scan => self.scan_interval_secs,
            IntervalTarget::Sync => self
                .selected_sync_job
                .and_then(|ix| self.sync_jobs.get(ix))
                .map(|job| job.interval_secs)
                .unwrap_or_default(),
        }
    }

    /// 주기 드롭다운과 직접 추가 위젯을 준비한다(윈도우가 있어야 만들 수 있어 지연 생성).
    pub(crate) fn ensure_interval_widgets(
        &mut self,
        target: IntervalTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ensure_interval_select(target, window, cx);

        if self.interval_picker.amount_input.is_none() {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("예: 45")
                    .default_value("")
            });
            self.interval_picker.amount_input = Some(input);
        }

        if self.interval_picker.unit_select.is_none() {
            let labels: Vec<&'static str> = TimeUnit::ALL.iter().map(|u| u.label()).collect();
            let unit = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(labels),
                    Some(IndexPath::new(0)),
                    window,
                    cx,
                )
            });
            self.interval_picker.unit_select = Some(unit);
        }
    }

    fn ensure_interval_select(
        &mut self,
        target: IntervalTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing = match target {
            IntervalTarget::Scan => &self.interval_picker.scan_select,
            IntervalTarget::Sync => &self.interval_picker.sync_select,
        };
        if existing.is_some() {
            self.sync_interval_select(target, window, cx);
            return;
        }

        let current = self.interval_value(target);
        let options = self.interval_options(current);
        let selected = options.iter().position(|secs| *secs == current);
        let items: Vec<IntervalItem> = options.into_iter().map(IntervalItem::new).collect();

        let select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(items),
                selected.map(IndexPath::new),
                window,
                cx,
            )
        });

        let subscription = cx.subscribe_in(
            &select,
            window,
            move |this: &mut Self,
                  _select,
                  event: &SelectEvent<SearchableVec<IntervalItem>>,
                  window,
                  cx| {
                let SelectEvent::Confirm(Some(secs)) = event else {
                    return;
                };
                this.apply_interval(target, *secs, window, cx);
            },
        );

        match target {
            IntervalTarget::Scan => self.interval_picker.scan_select = Some(select),
            IntervalTarget::Sync => self.interval_picker.sync_select = Some(select),
        }
        self.subscriptions.push(subscription);
    }

    /// 드롭다운 항목과 선택 위치를 현재 상태에 맞춘다.
    ///
    /// 프리셋이 추가되거나 다른 동기화 작업을 고르면 목록·선택이 함께 바뀌어야 한다.
    pub(crate) fn sync_interval_select(
        &mut self,
        target: IntervalTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let select = match target {
            IntervalTarget::Scan => self.interval_picker.scan_select.clone(),
            IntervalTarget::Sync => self.interval_picker.sync_select.clone(),
        };
        let Some(select) = select else {
            return;
        };

        let current = self.interval_value(target);
        let options = self.interval_options(current);
        let selected = options.iter().position(|secs| *secs == current);
        let items: Vec<IntervalItem> = options.into_iter().map(IntervalItem::new).collect();

        select.update(cx, |state, cx| {
            state.set_items(SearchableVec::new(items), window, cx);
            state.set_selected_index(selected.map(IndexPath::new), window, cx);
        });
    }

    fn apply_interval(
        &mut self,
        target: IntervalTarget,
        secs: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match target {
            IntervalTarget::Scan => self.set_scan_interval(secs, cx),
            IntervalTarget::Sync => {
                self.update_selected_sync_job(window, cx, |job| job.interval_secs = secs)
            }
        }
    }

    fn selected_time_unit(&self, cx: &Context<Self>) -> TimeUnit {
        self.interval_picker
            .unit_select
            .as_ref()
            .and_then(|select| select.read(cx).selected_value().copied())
            .and_then(TimeUnit::from_label)
            .unwrap_or(TimeUnit::Seconds)
    }

    /// 입력한 (값, 단위)를 프리셋 목록에 추가하고 현재 주기로 선택한다.
    pub(crate) fn add_interval_preset(
        &mut self,
        target: IntervalTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let amount = self
            .interval_picker
            .amount_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let unit = self.selected_time_unit(cx);

        let secs = match interval_to_secs(&amount, unit) {
            Ok(secs) => secs,
            Err(reason) => {
                self.interval_picker.error = Some(reason);
                cx.notify();
                return;
            }
        };

        self.interval_picker.error = None;

        let already_known = self.interval_picker.presets.contains(&secs);
        if !already_known {
            self.interval_picker.presets.push(secs);
            normalize_interval_presets(&mut self.interval_picker.presets);
            self.persist_interval_presets();
        }

        if let Some(input) = self.interval_picker.amount_input.as_ref() {
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }

        // 추가한 주기를 바로 쓰는 것이 사용자의 의도다.
        self.apply_interval(target, secs, window, cx);
        self.sync_interval_select(target, window, cx);
        self.push_log(
            "INFO",
            format!("주기 프리셋을 추가했습니다: {}", format_interval(secs)),
        );
        cx.notify();
    }

    /// 프리셋을 목록에서 제거한다. 마지막 하나는 남긴다.
    pub(crate) fn remove_interval_preset(
        &mut self,
        secs: u32,
        target: IntervalTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.interval_picker.presets.len() <= 1 {
            self.interval_picker.error =
                Some("마지막 프리셋은 삭제할 수 없습니다.".to_string());
            cx.notify();
            return;
        }

        self.interval_picker.presets.retain(|value| *value != secs);
        self.interval_picker.error = None;
        self.persist_interval_presets();
        self.sync_interval_select(target, window, cx);
        cx.notify();
    }

    fn persist_interval_presets(&self) {
        let presets = self.interval_picker.presets.clone();

        #[cfg(test)]
        if !self.external_side_effects_enabled {
            return;
        }

        if let Err(err) = update_config(move |cfg| cfg.interval_presets = presets) {
            log::error!("주기 프리셋 저장 실패: {err}");
        }
    }
}
