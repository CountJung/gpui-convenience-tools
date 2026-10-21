//! 로컬 전용 클립보드 이력. 정책·한도는 UI/OS와 독립적으로 적용한다.
use crate::config::ClipboardConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

mod image_data;
mod storage;
pub(crate) mod windows;
pub use image_data::normalize_png;
pub use storage::HistoryStore;
#[cfg(test)]
pub(crate) mod tests;

pub const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Content {
    Text {
        text: String,
    },
    Png {
        digest: String,
        width: u32,
        height: u32,
        bytes: u64,
    },
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub id: u64,
    pub created_at: u64,
    pub pinned: bool,
    pub content: Content,
}

impl Entry {
    pub fn bytes(&self) -> u64 {
        match &self.content {
            Content::Text { text } => text.len() as u64,
            Content::Png { bytes, .. } => *bytes,
        }
    }

    pub fn preview(&self) -> String {
        match &self.content {
            Content::Text { text } => text
                .chars()
                .take(160)
                .map(|ch| if ch.is_control() { ' ' } else { ch })
                .collect(),
            Content::Png { width, height, .. } => format!("PNG 이미지 · {width} × {height}"),
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        self.preview()
            .to_lowercase()
            .contains(&query.to_lowercase())
            || matches!(&self.content, Content::Text { text } if text.to_lowercase().contains(&query.to_lowercase()))
    }
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct History {
    pub entries: Vec<Entry>,
    pub next_id: u64,
}

pub enum Candidate {
    Text(String),
    Png(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureResult {
    Added,
    Duplicate,
    Disabled,
    Sensitive,
    Excluded,
    TooLarge,
    PinnedLimit,
    InvalidImage,
    Unavailable,
    Changed,
}

impl CaptureResult {
    pub fn message(self) -> &'static str {
        match self {
            Self::Added => "최근 항목을 로컬에 보관했습니다.",
            Self::Duplicate => "이미 보관된 항목입니다.",
            Self::Disabled => "이 유형의 수집은 꺼져 있습니다.",
            Self::Sensitive => "민감한 형식 또는 비밀 패턴이므로 수집하지 않았습니다.",
            Self::Excluded => "제외 문구에 해당하여 수집하지 않았습니다.",
            Self::TooLarge => "항목이 크기 한도를 초과하여 수집하지 않았습니다.",
            Self::PinnedLimit => "고정 항목이 한도를 채워 새 항목을 수집하지 않았습니다.",
            Self::InvalidImage => "지원하지 않거나 손상된 이미지입니다. PNG 이미지만 수집합니다.",
            Self::Unavailable => {
                "클립보드 접근 또는 보호 확인 불가 · 안전을 위해 수집하지 않습니다."
            }
            Self::Changed => "읽는 동안 클립보드가 변경되어 수집하지 않았습니다.",
        }
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sensitive_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "-----begin private key-----",
        "-----begin rsa private key-----",
        "-----begin openssh private key-----",
        "password=",
        "password:",
        "passwd=",
        "authorization: bearer ",
        "aws_secret_access_key",
        "client_secret=",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
        || lower
            .split_whitespace()
            .any(|part| part.starts_with("eyj") && part.matches('.').count() == 2)
}

impl History {
    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(Entry::bytes).sum()
    }

    /// 고정은 개수/용량 퇴출 우선순위만 보호한다. 기간 만료와 한도 변경은 항상 적용한다.
    pub fn prune(&mut self, config: &ClipboardConfig, now: u64) -> bool {
        let config = config.clone().normalized();
        let before = self.entries.len();
        self.entries.retain(|entry| {
            entry.created_at <= now.saturating_add(60)
                && now.saturating_sub(entry.created_at) < config.max_age_hours * 3600
        });
        while self.entries.len() > config.max_entries || self.total_bytes() > config.max_bytes {
            let index = self
                .entries
                .iter()
                .rposition(|entry| !entry.pinned)
                .unwrap_or(self.entries.len() - 1);
            self.entries.remove(index);
        }
        before != self.entries.len()
    }

    /// sensitive_marker는 본문을 읽기 전에 OS 경계에서 확인한 결과다. 테스트는 합성값만 쓴다.
    pub fn capture(
        &mut self,
        candidate: Candidate,
        sensitive_marker: bool,
        config: &ClipboardConfig,
        now: u64,
    ) -> (CaptureResult, Option<Vec<u8>>) {
        if sensitive_marker {
            return (CaptureResult::Sensitive, None);
        }
        let config = config.clone().normalized();
        let (content, image) = match candidate {
            Candidate::Text(text) => {
                if !config.collect_text {
                    return (CaptureResult::Disabled, None);
                }
                if text.is_empty() || text.len() > MAX_TEXT_BYTES {
                    return (CaptureResult::TooLarge, None);
                }
                if config.exclude_sensitive && sensitive_text(&text) {
                    return (CaptureResult::Sensitive, None);
                }
                let lower = text.to_lowercase();
                if config
                    .excluded_phrases
                    .iter()
                    .any(|phrase| lower.contains(&phrase.to_lowercase()))
                {
                    return (CaptureResult::Excluded, None);
                }
                (Content::Text { text }, None)
            }
            Candidate::Png(bytes) => {
                if !config.collect_images {
                    return (CaptureResult::Disabled, None);
                }
                let Ok((bytes, width, height)) = normalize_png(&bytes) else {
                    return (CaptureResult::InvalidImage, None);
                };
                (
                    Content::Png {
                        digest: digest(&bytes),
                        width,
                        height,
                        bytes: bytes.len() as u64,
                    },
                    Some(bytes),
                )
            }
        };
        self.prune(&config, now);
        if self.entries.iter().any(|entry| entry.content == content) {
            return (CaptureResult::Duplicate, None);
        }
        let bytes = match &content {
            Content::Text { text } => text.len() as u64,
            Content::Png { bytes, .. } => *bytes,
        };
        if bytes > config.max_bytes {
            return (CaptureResult::TooLarge, None);
        }
        let pinned_bytes: u64 = self
            .entries
            .iter()
            .filter(|entry| entry.pinned)
            .map(Entry::bytes)
            .sum();
        if self.entries.iter().filter(|entry| entry.pinned).count() >= config.max_entries
            || pinned_bytes + bytes > config.max_bytes
        {
            return (CaptureResult::PinnedLimit, None);
        }
        let mut id = self.next_id.checked_add(1).unwrap_or(1);
        while self.entries.iter().any(|entry| entry.id == id) {
            id = id.checked_add(1).unwrap_or(1);
        }
        self.next_id = self.next_id.max(id);
        self.entries.insert(
            0,
            Entry {
                id,
                created_at: now,
                pinned: false,
                content,
            },
        );
        self.prune(&config, now);
        (CaptureResult::Added, image)
    }
}
