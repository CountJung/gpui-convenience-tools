use super::*;
use std::cell::Cell;

struct Source {
    kind: SnapshotKind,
    length: usize,
    bytes: Vec<u8>,
    sequence: Cell<u32>,
    copies: Cell<usize>,
    change_before_copy: bool,
    change_after_copy: bool,
    sensitive_after_copy: bool,
    sensitive: bool,
}
impl Source {
    fn png(length: usize) -> Self {
        Self {
            kind: SnapshotKind::Png,
            length,
            bytes: vec![1, 2, 3],
            sequence: Cell::new(10),
            copies: Cell::new(0),
            change_before_copy: false,
            change_after_copy: false,
            sensitive_after_copy: false,
            sensitive: false,
        }
    }
}
impl SnapshotSource for Source {
    fn probe(&self) -> Probe {
        if self.sensitive || (self.sensitive_after_copy && self.copies.get() > 0) {
            Probe::Sensitive
        } else {
            Probe::Allowed(self.sequence.get())
        }
    }
    fn sequence(&self) -> u32 {
        self.sequence.get()
    }
    fn metadata(&mut self, _: &ClipboardConfig) -> Result<(SnapshotKind, usize), CaptureResult> {
        if self.change_before_copy {
            self.sequence.set(11);
        }
        Ok((self.kind, self.length))
    }
    fn copy(&self, _: usize) -> Result<Vec<u8>, CaptureResult> {
        self.copies.set(self.copies.get() + 1);
        if self.change_after_copy {
            self.sequence.set(11);
        }
        Ok(self.bytes.clone())
    }
}
fn images() -> ClipboardConfig {
    ClipboardConfig {
        collect_images: true,
        ..Default::default()
    }
}

#[test]
fn clipboard_bounded_snapshot_oversize_and_image_off_never_copy_payload() {
    for length in [MAX_IMAGE_BYTES + 1, usize::MAX] {
        let mut source = Source::png(length);
        assert!(matches!(
            read_bounded(&mut source, &images(), 10),
            Err(CaptureResult::TooLarge)
        ));
        assert_eq!(source.copies.get(), 0);
    }
    let mut source = Source::png(usize::MAX);
    let text_only = ClipboardConfig {
        collect_text: true,
        ..Default::default()
    };
    assert!(matches!(
        read_bounded(&mut source, &text_only, 10),
        Err(CaptureResult::Disabled)
    ));
    assert_eq!(source.copies.get(), 0);
}

#[test]
fn clipboard_bounded_snapshot_sequence_races_and_markers_discard_payload() {
    let mut source = Source::png(3);
    source.change_before_copy = true;
    assert!(matches!(
        read_bounded(&mut source, &images(), 10),
        Err(CaptureResult::Changed)
    ));
    assert_eq!(source.copies.get(), 0);
    let mut source = Source::png(3);
    source.change_after_copy = true;
    assert!(matches!(
        read_bounded(&mut source, &images(), 10),
        Err(CaptureResult::Changed)
    ));
    assert_eq!(source.copies.get(), 1);
    let mut source = Source::png(3);
    source.sensitive = true;
    assert!(matches!(
        read_bounded(&mut source, &images(), 10),
        Err(CaptureResult::Sensitive)
    ));
    assert_eq!(source.copies.get(), 0);
    let mut source = Source::png(3);
    source.sensitive_after_copy = true;
    assert!(matches!(
        read_bounded(&mut source, &images(), 10),
        Err(CaptureResult::Changed)
    ));
}

#[test]
fn clipboard_bounded_snapshot_utf16_limits_termination_and_valid_png() {
    let config = ClipboardConfig {
        collect_text: true,
        ..Default::default()
    };
    let mut source = Source::png(6);
    source.kind = SnapshotKind::Utf16;
    source.bytes = vec![b'A', 0, b'B', 0, 0, 0];
    assert!(
        matches!(read_bounded(&mut source, &config, 10), Ok(Candidate::Text(text)) if text == "AB")
    );
    source.length = (MAX_TEXT_BYTES + 1) * 2 + 1;
    source.copies.set(0);
    assert!(matches!(
        read_bounded(&mut source, &config, 10),
        Err(CaptureResult::TooLarge)
    ));
    assert_eq!(source.copies.get(), 0);
    source.length = 2;
    source.bytes = vec![b'A', 0];
    assert!(matches!(
        read_bounded(&mut source, &config, 10),
        Err(CaptureResult::Unavailable)
    ));
    let mut source = Source::png(3);
    assert!(
        matches!(read_bounded(&mut source, &images(), 10), Ok(Candidate::Png(bytes)) if bytes == vec![1, 2, 3])
    );
}
