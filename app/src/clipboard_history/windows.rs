//! OS 경계의 공통 제한. 테스트 빌드에서는 네이티브 접근을 컴파일하지 않는다.
use super::{Candidate, CaptureResult, MAX_IMAGE_BYTES, MAX_TEXT_BYTES};
use crate::config::ClipboardConfig;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Allowed(u32),
    Sensitive,
    Unavailable,
}

#[derive(Clone, Copy)]
pub(super) enum SnapshotKind {
    Utf16,
    Png,
}

/// 잠금 상태의 메타데이터만 먼저 조사하고, 허용 크기 안에서만 본문을 복사한다.
pub(super) trait SnapshotSource {
    fn probe(&self) -> Probe;
    fn sequence(&self) -> u32;
    fn metadata(
        &mut self,
        config: &ClipboardConfig,
    ) -> Result<(SnapshotKind, usize), CaptureResult>;
    fn copy(&self, bytes: usize) -> Result<Vec<u8>, CaptureResult>;
}

pub(super) fn read_bounded(
    source: &mut impl SnapshotSource,
    config: &ClipboardConfig,
    expected: u32,
) -> Result<Candidate, CaptureResult> {
    match source.probe() {
        Probe::Sensitive => return Err(CaptureResult::Sensitive),
        Probe::Unavailable => return Err(CaptureResult::Unavailable),
        Probe::Allowed(sequence) if sequence != expected => return Err(CaptureResult::Changed),
        _ => {}
    }
    let (kind, length) = source.metadata(config)?;
    let (enabled, maximum) = match kind {
        SnapshotKind::Utf16 => (config.collect_text, (MAX_TEXT_BYTES + 1) * 2),
        SnapshotKind::Png => (config.collect_images, MAX_IMAGE_BYTES),
    };
    if !enabled {
        return Err(CaptureResult::Disabled);
    }
    if length == 0 || length > maximum {
        return Err(CaptureResult::TooLarge);
    }
    if source.sequence() != expected || source.probe() != Probe::Allowed(expected) {
        return Err(CaptureResult::Changed);
    }
    let bytes = source.copy(length)?;
    if source.sequence() != expected || source.probe() != Probe::Allowed(expected) {
        return Err(CaptureResult::Changed);
    }
    match kind {
        SnapshotKind::Png => Ok(Candidate::Png(bytes)),
        SnapshotKind::Utf16 => {
            if bytes.len() % 2 != 0 {
                return Err(CaptureResult::Unavailable);
            }
            let words: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            let end = words
                .iter()
                .position(|word| *word == 0)
                .ok_or(CaptureResult::Unavailable)?;
            let text = String::from_utf16(&words[..end]).map_err(|_| CaptureResult::Unavailable)?;
            if text.len() > MAX_TEXT_BYTES {
                return Err(CaptureResult::TooLarge);
            }
            Ok(Candidate::Text(text))
        }
    }
}

#[cfg(all(target_os = "windows", not(test)))]
mod native;
#[cfg(all(target_os = "windows", not(test)))]
pub use native::{probe, sequence, snapshot};

#[cfg(not(target_os = "windows"))]
#[cfg(not(test))]
pub fn snapshot(_: &ClipboardConfig, _: u32) -> Result<Candidate, CaptureResult> {
    Err(CaptureResult::Unavailable)
}

#[cfg(any(not(target_os = "windows"), test))]
pub fn probe() -> Probe {
    #[cfg(not(test))]
    {
        Probe::Unavailable
    }
    #[cfg(test)]
    {
        TEST_PROBE.with(std::cell::Cell::get)
    }
}

#[cfg(any(not(target_os = "windows"), test))]
pub fn sequence() -> u32 {
    #[cfg(not(test))]
    {
        0
    }
    #[cfg(test)]
    {
        TEST_SEQUENCE.with(std::cell::Cell::get)
    }
}

#[cfg(test)]
thread_local! {
    static TEST_PROBE: std::cell::Cell<Probe> = const { std::cell::Cell::new(Probe::Allowed(0)) };
    static TEST_SEQUENCE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
pub struct ProbeGuard(Probe, u32);
#[cfg(test)]
pub fn test_probe(probe: Probe) -> ProbeGuard {
    let old = ProbeGuard(TEST_PROBE.with(|value| value.replace(probe)), sequence());
    if let Probe::Allowed(sequence) = probe {
        TEST_SEQUENCE.with(|value| value.set(sequence));
    }
    old
}
#[cfg(test)]
pub fn test_sequence(sequence: u32) {
    TEST_SEQUENCE.with(|value| value.set(sequence));
    TEST_PROBE.with(|value| value.set(Probe::Allowed(sequence)));
}
#[cfg(test)]
impl Drop for ProbeGuard {
    fn drop(&mut self) {
        TEST_PROBE.with(|value| value.set(self.0));
        TEST_SEQUENCE.with(|value| value.set(self.1));
    }
}

#[cfg(test)]
mod tests;
