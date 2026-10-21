//! OS 본문 읽기·PNG 처리·검증·디스크 I/O를 단일 작업으로 분리한다.
use super::*;

pub(super) enum ClipboardJob {
    Load(HistoryStore, ClipboardConfig),
    Prune(Option<HistoryStore>, History),
    #[cfg(not(test))]
    Read {
        store: Option<HistoryStore>,
        history: History,
        config: ClipboardConfig,
        sequence: u32,
        previous_fingerprint: Option<String>,
    },
    Capture {
        store: Option<HistoryStore>,
        history: History,
        config: ClipboardConfig,
        candidate: Candidate,
        previous_fingerprint: Option<String>,
    },
}

struct Reply {
    history: History,
    image: Option<Vec<u8>>,
    captured: Option<CaptureResult>,
    fingerprint: Option<String>,
}

impl ClipboardJob {
    fn run(self) -> Result<Reply, &'static str> {
        match self {
            #[cfg(not(test))]
            Self::Read {
                store,
                history,
                config,
                sequence,
                previous_fingerprint,
            } => match history::windows::snapshot(&config, sequence) {
                Ok(candidate) => Self::Capture {
                    store,
                    history,
                    config,
                    candidate,
                    previous_fingerprint,
                }
                .run(),
                Err(result) => Ok(Reply {
                    history,
                    image: None,
                    captured: Some(result),
                    fingerprint: None,
                }),
            },
            Self::Load(store, config) => {
                let history = store.load(&config, now())?;
                // 로드 시 제거한 만료 데이터의 디스크 정리도 worker에서 실행한다.
                store.save(&history, None)?;
                Ok(Reply {
                    history,
                    image: None,
                    captured: None,
                    fingerprint: None,
                })
            }
            Self::Prune(store, history) => {
                if let Some(store) = store {
                    store.save(&history, None)?;
                }
                Ok(Reply {
                    history,
                    image: None,
                    captured: None,
                    fingerprint: None,
                })
            }
            Self::Capture {
                store,
                mut history,
                config,
                candidate,
                previous_fingerprint,
            } => {
                // UI에서 검사한 뒤 worker 실행까지 만료 경계를 넘을 수 있다.
                // 수집 거부·중복·fingerprint 조기 반환과 무관하게 만료 정리를 저장한다.
                let captured_at = now();
                let pruned = history.prune(&config, captured_at);
                let fingerprint = match &candidate {
                    Candidate::Text(text) => format!("text:{}", history::digest(text.as_bytes())),
                    Candidate::Png(bytes) => format!("png:{}", history::digest(bytes)),
                };
                if previous_fingerprint.as_ref() == Some(&fingerprint) {
                    if pruned {
                        if let Some(store) = &store {
                            store.save(&history, None)?;
                        }
                    }
                    return Ok(Reply {
                        history,
                        image: None,
                        captured: None,
                        fingerprint: Some(fingerprint),
                    });
                }
                let (result, image) = history.capture(candidate, false, &config, captured_at);
                if pruned || result == CaptureResult::Added {
                    if let Some(store) = store {
                        store.save(&history, image.as_deref())?;
                    }
                }
                Ok(Reply {
                    history,
                    image,
                    captured: Some(result),
                    fingerprint: Some(fingerprint),
                })
            }
        }
    }
}

impl AppRoot {
    pub(super) fn clipboard_job_pending(&mut self, cx: &mut Context<Self>) -> bool {
        if self.clipboard.busy {
            self.clipboard.status = "이력 처리 중입니다. 완료 후 다시 시도하세요.".into();
            cx.notify();
        }
        self.clipboard.busy
    }

    pub(super) fn begin_clipboard_job(&mut self, job: ClipboardJob, cx: &mut Context<Self>) {
        // 큐를 만들지 않는다. worker가 느리거나 멈추더라도 pending snapshot은 최대 하나다.
        if self.clipboard.busy {
            return;
        }
        self.clipboard.busy = true;
        let task = cx.background_executor().spawn(async move { job.run() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.clipboard.busy = false;
                // 긴 작업 직후 즉시 재수집하지 않고 완료 시점부터 750ms를 센다.
                this.clipboard.last_poll = Some(Instant::now());
                match result {
                    Ok(reply) => {
                        this.clipboard.history = reply.history;
                        if let Some(fingerprint) = reply.fingerprint {
                            this.clipboard.last_fingerprint = Some(fingerprint);
                        }
                        if let Some(image) = reply.image {
                            // 정규 PNG의 해시는 worker에서 이미 계산했다. UI에서 다시 해시하지 않는다.
                            if let Some(hash) = this.clipboard.history.entries.iter().find_map(|entry| {
                                match &entry.content {
                                    Content::Png { digest, .. } => Some(digest.clone()),
                                    _ => None,
                                }
                            }) {
                                this.clipboard.image_cache.insert(hash, image);
                            }
                        }
                        if let Some(result) = reply.captured {
                            if result == CaptureResult::Unavailable {
                                // 같은 내용의 일시 잠금 실패는 다음 tick에서 재시도할 수 있다.
                                this.clipboard.last_sequence = None;
                            }
                            this.clipboard.failed = false;
                            this.clipboard.status = result.message().into();
                        }
                        this.trim_clipboard_cache();
                    }
                    Err(error) => {
                        this.clipboard.blocked = true;
                        this.clipboard.last_fingerprint = None;
                        this.clipboard.last_sequence = None;
                        this.clipboard_failure(&format!("{error} · 안전을 위해 수집 중지. 권한 확인 후 로컬 초기화로 재개할 수 있습니다."));
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_background_duplicate_and_fingerprint_skip_persist_expired_png_cleanup() {
        // 일반 Duplicate와 fingerprint 조기 반환을 각각 실제 격리 index/PNG로 검증한다.
        for cached_fingerprint in [false, true] {
            let fixture = history::tests::Fixture::new();
            let store = HistoryStore::new(&fixture.path);
            let config = ClipboardConfig {
                collect_text: true,
                collect_images: true,
                max_age_hours: 1,
                ..Default::default()
            };
            let worker_time = now();
            let mut history = History::default();
            let (result, image) = history.capture(
                Candidate::Png(history::tests::png()),
                false,
                &config,
                worker_time - 3600,
            );
            assert_eq!(result, CaptureResult::Added);
            let text = "live-duplicate-worker-fixture";
            assert_eq!(
                history
                    .capture(
                        Candidate::Text(text.into()),
                        false,
                        &config,
                        worker_time - 1,
                    )
                    .0,
                CaptureResult::Added,
            );
            // UI 검사 때는 두 항목이 모두 유효하지만 worker 실행 시 PNG만 만료된다.
            assert!(!history.prune(&config, worker_time - 1));
            assert_eq!(history.entries.len(), 2);
            let hash = history
                .entries
                .iter()
                .find_map(|entry| match &entry.content {
                    Content::Png { digest, .. } => Some(digest.clone()),
                    _ => None,
                })
                .unwrap();
            let png_path = fixture
                .path
                .join("clipboard-history")
                .join(format!("{hash}.png"));
            store.save(&history, image.as_deref()).unwrap();
            assert!(png_path.exists());
            let reply = ClipboardJob::Capture {
                store: Some(store.clone()),
                history,
                config: config.clone(),
                candidate: Candidate::Text(text.into()),
                previous_fingerprint: cached_fingerprint
                    .then(|| format!("text:{}", history::digest(text.as_bytes()))),
            }
            .run()
            .unwrap();
            assert_eq!(
                reply.captured,
                if cached_fingerprint {
                    None
                } else {
                    Some(CaptureResult::Duplicate)
                },
            );
            assert!(reply.image.is_none());
            assert_eq!(reply.history.entries.len(), 1);
            assert_eq!(reply.history.entries[0].preview(), text);
            assert!(!png_path.exists());
            // load() 자체의 prune으로 잘못된 디스크 index가 가려지지 않도록 원문을 확인한다.
            let index: serde_json::Value = serde_json::from_slice(
                &std::fs::read(fixture.path.join("clipboard-history/index.json")).unwrap(),
            )
            .unwrap();
            let saved = index["history"]["entries"].as_array().unwrap();
            assert_eq!(saved.len(), 1);
            assert_eq!(saved[0]["content"]["text"], text);
            assert!(store.load(&config, now()).unwrap() == reply.history);
        }
    }

    #[test]
    fn clipboard_background_capture_and_prune_use_isolated_store() {
        let fixture = history::tests::Fixture::new();
        let store = HistoryStore::new(&fixture.path);
        let config = ClipboardConfig {
            collect_images: true,
            ..Default::default()
        };
        let reply = ClipboardJob::Capture {
            store: Some(store.clone()),
            history: History::default(),
            config: config.clone(),
            candidate: Candidate::Png(history::tests::png()),
            previous_fingerprint: None,
        }
        .run()
        .unwrap();
        assert_eq!(reply.captured, Some(CaptureResult::Added));
        assert!(reply.image.is_some());
        assert_eq!(store.load(&config, now()).unwrap().entries.len(), 1);
        ClipboardJob::Prune(Some(store.clone()), History::default())
            .run()
            .unwrap();
        assert!(store.load(&config, now()).unwrap().entries.is_empty());
        assert_eq!(
            std::fs::read_dir(fixture.path.join("clipboard-history"))
                .unwrap()
                .count(),
            1
        );
    }
}
