use super::*;
use std::{fs, path::PathBuf};

pub(crate) struct Fixture {
    pub path: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let mut random = [0u8; 12];
        getrandom::getrandom(&mut random).unwrap();
        let path = std::env::temp_dir().join(format!("gpui-clipboard-test-{}", digest(&random)));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self.path.starts_with(std::env::temp_dir()));
        assert!(self
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gpui-clipboard-test-"));
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn config() -> ClipboardConfig {
    ClipboardConfig {
        collect_text: true,
        collect_images: true,
        ..Default::default()
    }
}
fn capture(history: &mut History, text: &str, config: &ClipboardConfig, now: u64) -> CaptureResult {
    history
        .capture(Candidate::Text(text.into()), false, config, now)
        .0
}
pub(crate) fn png() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(2, 3, image::Rgba([10, 20, 30, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn clipboard_default_off_and_sensitive_marker_never_collect() {
    let mut history = History::default();
    assert_eq!(
        capture(&mut history, "fixture", &ClipboardConfig::default(), 100),
        CaptureResult::Disabled
    );
    assert_eq!(
        history
            .capture(
                Candidate::Png(png()),
                false,
                &ClipboardConfig::default(),
                100
            )
            .0,
        CaptureResult::Disabled
    );
    assert_eq!(
        history
            .capture(
                Candidate::Text("unmarked-secret".into()),
                true,
                &config(),
                100
            )
            .0,
        CaptureResult::Sensitive
    );
    assert_eq!(
        history
            .capture(Candidate::Png(png()), true, &config(), 100)
            .0,
        CaptureResult::Sensitive
    );
    assert!(history.entries.is_empty());
}

#[test]
fn clipboard_sensitive_patterns_and_excluded_phrases_are_not_stored() {
    let mut history = History::default();
    let mut config = config();
    config.excluded_phrases = vec!["confidential".into()];
    for text in [
        "-----BEGIN RSA PRIVATE KEY-----",
        "PASSWORD=fixture",
        "Authorization: Bearer fixture",
        "eyJaaa.bbb.ccc",
    ] {
        assert_eq!(
            capture(&mut history, text, &config, 100),
            CaptureResult::Sensitive
        );
    }
    assert_eq!(
        capture(&mut history, "CONFIDENTIAL fixture", &config, 100),
        CaptureResult::Excluded
    );
    assert!(history.entries.is_empty());
    config.exclude_sensitive = false;
    assert_eq!(
        capture(&mut history, "password=synthetic", &config, 100),
        CaptureResult::Added
    );
}

#[test]
fn clipboard_deduplicate_full_text_search_and_pin_priority() {
    let mut history = History::default();
    let config = ClipboardConfig {
        max_entries: 2,
        ..config()
    };
    assert_eq!(
        capture(&mut history, "old", &config, 100),
        CaptureResult::Added
    );
    history.entries[0].pinned = true;
    assert_eq!(
        capture(&mut history, "new", &config, 101),
        CaptureResult::Added
    );
    assert_eq!(
        capture(&mut history, "new", &config, 102),
        CaptureResult::Duplicate
    );
    let long = format!("{}needle", "x".repeat(200));
    assert_eq!(
        capture(&mut history, &long, &config, 103),
        CaptureResult::Added
    );
    assert!(history.entries[0].matches("NEEDLE"));
    assert!(history.entries[1].pinned);
    assert!(!history.entries.iter().any(|entry| entry.preview() == "new"));
    history.entries[0].pinned = true;
    assert_eq!(
        capture(&mut history, "overflow", &config, 104),
        CaptureResult::PinnedLimit
    );
}

#[test]
fn clipboard_count_byte_age_limits_apply_even_to_pins() {
    let config = ClipboardConfig {
        max_entries: 10,
        max_bytes: 64 * 1024,
        max_age_hours: 1,
        ..config()
    };
    let mut history = History::default();
    capture(&mut history, &"a".repeat(40000), &config, 100);
    history.entries[0].pinned = true;
    assert_eq!(
        capture(&mut history, &"b".repeat(40000), &config, 101),
        CaptureResult::PinnedLimit
    );
    assert_eq!(
        capture(&mut history, &"c".repeat(MAX_TEXT_BYTES + 1), &config, 102),
        CaptureResult::TooLarge
    );
    assert!(history.prune(&config, 3700));
    assert!(history.entries.is_empty());
    for i in 0..3 {
        capture(&mut history, &format!("item{i}"), &config, 4000);
    }
    for entry in &mut history.entries {
        entry.pinned = true;
    }
    let smaller = ClipboardConfig {
        max_entries: 1,
        ..config
    };
    history.prune(&smaller, 4001);
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].preview(), "item2");
}

#[test]
fn clipboard_byte_limit_evicts_old_unpinned_and_rejects_future_dates() {
    let config = ClipboardConfig {
        max_bytes: 64 * 1024,
        ..config()
    };
    let mut history = History::default();
    capture(&mut history, &"a".repeat(40000), &config, 100);
    capture(&mut history, &"b".repeat(40000), &config, 101);
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.total_bytes(), 40000);
    history.entries[0].created_at = 999999;
    assert!(history.prune(&config, 102));
    assert!(history.entries.is_empty());
}

#[test]
fn clipboard_png_validation_roundtrip_hash_and_owned_cleanup() {
    let fixture = Fixture::new();
    let store = HistoryStore::new(&fixture.path);
    let mut history = History::default();
    let (result, image) = history.capture(Candidate::Png(png()), false, &config(), 100);
    assert_eq!(result, CaptureResult::Added);
    let image = image.unwrap();
    store.save(&history, Some(&image)).unwrap();
    let restored = store.load(&config(), 101).unwrap();
    assert!(restored == history);
    let Content::Png {
        digest: hash,
        width,
        height,
        ..
    } = &restored.entries[0].content
    else {
        panic!("PNG expected")
    };
    assert_eq!((*width, *height), (2, 3));
    assert_eq!(store.read_png(hash).unwrap(), image);
    let unrelated = fixture
        .path
        .join("clipboard-history")
        .join("user-image.png");
    fs::write(&unrelated, b"user-owned").unwrap();
    store.clear().unwrap();
    assert!(unrelated.exists());
    assert!(!fixture
        .path
        .join("clipboard-history")
        .join(format!("{hash}.png"))
        .exists());
    assert!(store.load(&config(), 102).unwrap().entries.is_empty());
}

#[test]
fn clipboard_corrupt_index_is_preserved_until_explicit_clear() {
    let fixture = Fixture::new();
    let store = HistoryStore::new(&fixture.path);
    let mut history = History::default();
    capture(&mut history, "synthetic", &config(), 100);
    store.save(&history, None).unwrap();
    let index = fixture.path.join("clipboard-history/index.json");
    fs::write(&index, b"{broken").unwrap();
    assert!(store.load(&config(), 101).is_err());
    assert!(store.save(&history, None).is_err());
    assert_eq!(fs::read(&index).unwrap(), b"{broken");
    store.clear().unwrap();
    assert!(store.load(&config(), 101).unwrap().entries.is_empty());
}

#[test]
fn clipboard_corrupt_png_and_traversal_are_rejected_without_source_change() {
    let fixture = Fixture::new();
    let store = HistoryStore::new(&fixture.path);
    let mut history = History::default();
    let (_, image) = history.capture(Candidate::Png(png()), false, &config(), 100);
    store.save(&history, image.as_deref()).unwrap();
    let hash = digest(image.as_ref().unwrap());
    let path = fixture
        .path
        .join("clipboard-history")
        .join(format!("{hash}.png"));
    fs::write(&path, b"corrupt").unwrap();
    assert!(store.load(&config(), 101).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"corrupt");
    assert!(store.read_png("../../outside").is_err());
}

#[test]
fn clipboard_atomic_write_failure_keeps_existing_index() {
    let fixture = Fixture::new();
    let store = HistoryStore::new(&fixture.path);
    let mut history = History::default();
    capture(&mut history, "first", &config(), 100);
    store.save(&history, None).unwrap();
    let index = fixture.path.join("clipboard-history/index.json");
    let before = fs::read(&index).unwrap();
    let pending = index.with_extension(format!("{}.pending", std::process::id()));
    fs::write(&pending, b"another-writer").unwrap();
    capture(&mut history, "second", &config(), 101);
    assert!(store.save(&history, None).is_err());
    assert_eq!(fs::read(&index).unwrap(), before);
    assert_eq!(fs::read(&pending).unwrap(), b"another-writer");
}

#[test]
fn clipboard_invalid_image_and_expired_reload_are_safe() {
    let mut history = History::default();
    assert_eq!(
        history
            .capture(Candidate::Png(b"not png".to_vec()), false, &config(), 100)
            .0,
        CaptureResult::InvalidImage
    );
    let fixture = Fixture::new();
    let store = HistoryStore::new(&fixture.path);
    capture(&mut history, "expired", &config(), 100);
    history.entries[0].pinned = true;
    store.save(&history, None).unwrap();
    assert!(store
        .load(&config(), 100 + 24 * 3600)
        .unwrap()
        .entries
        .is_empty());
}

#[test]
fn clipboard_png_encoded_and_decoded_limits_are_enforced() {
    assert!(normalize_png(&vec![0; MAX_IMAGE_BYTES + 1]).is_err());
    let image = image::RgbaImage::from_pixel(9000, 1, image::Rgba([0, 0, 0, 0]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    assert!(normalize_png(&bytes.into_inner()).is_err());
    let mut history = History::default();
    let mut config = config();
    config.max_bytes = 64 * 1024;
    let image = image::RgbaImage::from_fn(256, 256, |x, y| {
        image::Rgba([
            (x.wrapping_mul(37) ^ y.wrapping_mul(131)) as u8,
            (x.wrapping_mul(53) ^ y.wrapping_mul(71)) as u8,
            (x.wrapping_mul(23) ^ y.wrapping_mul(199)) as u8,
            255,
        ])
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let bytes = bytes.into_inner();
    assert!(bytes.len() > 64 * 1024);
    assert_eq!(
        history
            .capture(Candidate::Png(bytes), false, &config, 100)
            .0,
        CaptureResult::TooLarge
    );
    assert!(history.entries.is_empty());
}

#[test]
fn clipboard_metadata_traversal_and_oversized_image_size_are_rejected() {
    let fixture = Fixture::new();
    let store = HistoryStore::new(&fixture.path);
    let root = fixture.path.join("clipboard-history");
    fs::create_dir(&root).unwrap();
    let outside = fixture.path.join("outside.png");
    fs::write(&outside, b"preserve-outside").unwrap();
    let path = root.join("index.json");
    let mut history = History {
        next_id: 1,
        entries: vec![Entry {
            id: 1,
            created_at: 100,
            pinned: false,
            content: Content::Png {
                digest: "../../outside".into(),
                width: 2,
                height: 3,
                bytes: 10,
            },
        }],
    };
    for malicious in ["../../outside".to_string(), "a".repeat(64)] {
        if let Content::Png { digest, bytes, .. } = &mut history.entries[0].content {
            *digest = malicious;
            *bytes = u64::MAX;
        }
        let json = serde_json::json!({"version":1,"history":history});
        let source = serde_json::to_vec(&json).unwrap();
        fs::write(&path, &source).unwrap();
        assert!(store.load(&config(), 101).is_err());
        assert_eq!(fs::read(&path).unwrap(), source);
        assert_eq!(fs::read(&outside).unwrap(), b"preserve-outside");
    }
}
