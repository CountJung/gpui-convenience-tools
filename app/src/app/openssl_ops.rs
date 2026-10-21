//! PCF 호환 변환의 입력·실행·설정 저장. 작업 텍스트는 메모리에만 둔다.
use gpui::{AppContext, Context, Entity, ScrollHandle, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};

use super::AppRoot;
use crate::config::{update_config, OpenSslConfig};

#[derive(Default)]
pub(crate) struct OpenSslState {
    pub config: OpenSslConfig,
    pub passphrase: Option<Entity<InputState>>,
    pub input: Option<Entity<InputState>>,
    input_subscription: Option<Subscription>,
    pub output: Option<Entity<InputState>>,
    pub status: String,
    pub failed: bool,
    pub save_error: bool,
    pub scroll: ScrollHandle,
}

impl OpenSslState {
    pub fn from_config(config: OpenSslConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }
}

impl AppRoot {
    pub(crate) fn ensure_openssl_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.openssl.input.is_some() {
            return;
        }
        if self.openssl.passphrase.is_none() {
            let passphrase = cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .default_value(self.openssl.config.passphrase.clone())
                    .validate(|text, _| text.len() <= 1024)
            });
            let pass_sub = cx.subscribe_in(
                &passphrase,
                window,
                |this: &mut Self, input, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::Change) {
                        this.openssl.config.passphrase = input.read(cx).value().to_string();
                        this.clear_openssl_result(window, cx);
                        this.persist_openssl_config(cx);
                    }
                },
            );
            self.subscriptions.push(pass_sub);
            self.openssl.passphrase = Some(passphrase);
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(5)
                .placeholder("평문 또는 Salted__ 형식의 Base64 암호문을 입력하세요")
        });
        let input_sub = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.clear_openssl_result(window, cx);
                }
            },
        );
        self.openssl.input_subscription = Some(input_sub);
        self.openssl.input = Some(input);
        self.clear_openssl_result(window, cx);
    }

    fn clear_openssl_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // set_value는 undo 이력을 지우지 않는다. 위젯을 교체해 과거 결과 복원을 막는다.
        self.openssl.output = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(5)
                .placeholder("변환 결과 · 선택 후 Ctrl+C로 복사")
        }));
        self.openssl.status.clear();
        self.openssl.failed = false;
        cx.notify();
    }

    pub(crate) fn persist_openssl_config(&mut self, cx: &mut Context<Self>) {
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            cx.notify();
            return;
        }
        let config = self.openssl.config.clone();
        self.openssl.save_error = update_config(|cfg| cfg.openssl = config).is_err();
        if self.openssl.save_error {
            // 사용자 입력값이나 암호 문구는 로그에 기록하지 않는다.
            log::error!("OpenSSL 설정 저장 실패");
        }
        cx.notify();
    }

    pub(crate) fn set_openssl_newline(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.openssl.config.append_newline = enabled;
        self.clear_openssl_result(window, cx);
        self.persist_openssl_config(cx);
    }

    pub(crate) fn run_openssl(
        &mut self,
        encode: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self
            .openssl
            .input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        self.clear_openssl_result(window, cx);
        let passphrase = &self.openssl.config.passphrase;
        let result = if encode {
            crate::openssl_codec::encrypt(&text, passphrase, self.openssl.config.append_newline)
                .map(|text| {
                    (
                        text,
                        "인코딩 완료 · SHA-256 · 매번 새 salt 사용".to_string(),
                    )
                })
        } else {
            crate::openssl_codec::decrypt(&text, passphrase).map(|(text, digest)| {
                (
                    text,
                    format!("디코딩 완료 · {digest} · 끝 CR/LF 제거(PCF 호환)"),
                )
            })
        };
        match result {
            Ok((text, status)) => {
                if let Some(output) = &self.openssl.output {
                    output.update(cx, |state, cx| state.set_value(text, window, cx));
                }
                self.openssl.status = status;
            }
            Err(error) => {
                self.openssl.failed = true;
                self.openssl.status = error.to_string();
            }
        }
        cx.notify();
    }

    pub(crate) fn clear_openssl_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.openssl.input_subscription = None;
        self.openssl.input = None;
        self.ensure_openssl_inputs(window, cx);
    }
}
