//! 동기화 엔진 단위 테스트.
//!
//! 엔진 본문(`sync/mod.rs`)과 분리해 두 파일 모두 크기 경고 구간 아래로 유지한다.

use super::*;

/// 제어 없이 1회 실행하는 테스트 단축 호출.
fn run_sync_job(job: &SyncJob) -> SyncOutcome {
    run_sync_job_with_control(job, &mut SyncControl::new())
}

fn temp_dir(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "gct-sync-test-{name}-{}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn job(source: &Path, target: &Path) -> SyncJob {
    SyncJob {
        source: source.display().to_string(),
        target: target.display().to_string(),
        ..SyncJob::default()
    }
}

#[test]
fn copies_nested_files_and_skips_unchanged_on_second_run() {
    let root = temp_dir("basic");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::write(src.join("a.txt"), b"hello").unwrap();
    fs::write(src.join("nested/b.txt"), b"world").unwrap();

    let job = job(&src, &dst);

    let first = run_sync_job(&job);
    assert_eq!(first.copied, 2, "failures: {:?}", first.failures);
    assert_eq!(fs::read(dst.join("a.txt")).unwrap(), b"hello");
    assert_eq!(fs::read(dst.join("nested/b.txt")).unwrap(), b"world");

    // 두 번째 실행은 변경이 없으므로 모두 건너뛴다.
    let second = run_sync_job(&job);
    assert_eq!(second.copied, 0, "failures: {:?}", second.failures);
    assert_eq!(second.skipped, 2);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn recopies_when_size_differs() {
    let root = temp_dir("resize");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.txt"), b"one").unwrap();

    let job = job(&src, &dst);
    run_sync_job(&job);

    fs::write(src.join("a.txt"), b"a much longer content").unwrap();
    let outcome = run_sync_job(&job);
    assert_eq!(outcome.copied, 1);
    assert_eq!(
        fs::read(dst.join("a.txt")).unwrap(),
        b"a much longer content"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn mirror_deletes_removes_extra_target_files() {
    let root = temp_dir("mirror");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    fs::write(src.join("keep.txt"), b"keep").unwrap();
    fs::write(dst.join("stale.txt"), b"stale").unwrap();

    let mut job = job(&src, &dst);
    job.mirror_deletes = true;

    let outcome = run_sync_job(&job);
    assert_eq!(outcome.deleted, 1, "failures: {:?}", outcome.failures);
    assert!(!dst.join("stale.txt").exists());
    assert!(dst.join("keep.txt").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn rejects_target_inside_source() {
    let root = temp_dir("nested-target");
    let src = root.join("src");
    let dst = src.join("inner");
    fs::create_dir_all(&dst).unwrap();

    let outcome = run_sync_job(&job(&src, &dst));
    assert!(outcome.has_failures());
    assert!(outcome.failures[0].reason.contains("내부"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_source_reports_failure() {
    let root = temp_dir("missing");
    let outcome = run_sync_job(&job(&root.join("nope"), &root.join("dst")));
    assert!(outcome.has_failures());

    let _ = fs::remove_dir_all(&root);
}

#[cfg(target_os = "windows")]
#[test]
fn locked_source_reports_windows_sharing_violation_code_32() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    let root = temp_dir("sharing-violation");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    let source_file = src.join("open.xlsx");
    fs::write(&source_file, b"locked workbook").unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&source_file)
        .expect("open source without sharing");

    let blocked = run_sync_job(&job(&src, &dst));
    assert_eq!(blocked.copied, 0);
    assert!(
        blocked
            .failures
            .iter()
            .any(|failure| failure.reason.contains("공유 위반")
                && failure.reason.contains("(code 32)")),
        "locked source should expose the actionable Windows reason: {:?}",
        blocked.failures
    );

    drop(lock);
    let recovered = run_sync_job(&job(&src, &dst));
    assert_eq!(recovered.copied, 1, "failures: {:?}", recovered.failures);
    assert_eq!(fs::read(dst.join("open.xlsx")).unwrap(), b"locked workbook");

    let _ = fs::remove_dir_all(&root);
}

#[cfg(target_os = "windows")]
#[test]
fn hidden_and_system_file_is_copied_when_included_and_skipped_when_disabled() {
    use std::os::windows::fs::MetadataExt;
    use std::process::Command;

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;

    let root = temp_dir("hidden-system");
    let src = root.join("src");
    let included_dst = root.join("included");
    let excluded_dst = root.join("excluded");
    fs::create_dir_all(&src).unwrap();
    let source_file = src.join("protected.dat");
    fs::write(&source_file, b"hidden-system-content").unwrap();

    let status = Command::new("attrib")
        .args(["+h", "+s"])
        .arg(&source_file)
        .status()
        .expect("run attrib");
    assert!(
        status.success(),
        "attrib should set hidden and system flags"
    );
    let attributes = fs::metadata(&source_file).unwrap().file_attributes();
    assert_ne!(attributes & FILE_ATTRIBUTE_HIDDEN, 0);
    assert_ne!(attributes & FILE_ATTRIBUTE_SYSTEM, 0);

    let included = run_sync_job(&job(&src, &included_dst));
    assert_eq!(included.copied, 1, "failures: {:?}", included.failures);
    assert_eq!(
        fs::read(included_dst.join("protected.dat")).unwrap(),
        b"hidden-system-content"
    );

    let mut excluded_job = job(&src, &excluded_dst);
    excluded_job.include_hidden = false;
    let excluded = run_sync_job(&excluded_job);
    assert_eq!(excluded.copied, 0, "failures: {:?}", excluded.failures);
    assert_eq!(excluded.skipped, 1);
    assert!(!excluded_dst.join("protected.dat").exists());

    let _ = Command::new("attrib")
        .args(["-h", "-s"])
        .arg(&source_file)
        .status();
    let _ = fs::remove_dir_all(&root);
}

#[cfg(target_os = "windows")]
#[test]
fn path_over_260_characters_is_copied_or_reports_code_206() {
    use std::os::windows::ffi::OsStrExt;

    let root = temp_dir("long-path");
    let src = root.join("src");
    let dst = root.join("dst");
    let mut relative = PathBuf::new();
    while src.join(&relative).as_os_str().encode_wide().count() <= 280 {
        relative.push("segment-0123456789abcdef0123456789");
    }
    let deep_source = src.join(&relative);
    fs::create_dir_all(&deep_source).expect("create source path longer than 260 chars");
    let source_file = deep_source.join("long-path.txt");
    fs::write(&source_file, b"long path content").expect("write long path source");
    assert!(
        source_file.as_os_str().encode_wide().count() > 260,
        "test path should exceed the legacy MAX_PATH limit"
    );

    let outcome = run_sync_job(&job(&src, &dst));
    if outcome.copied == 1 {
        assert_eq!(
            fs::read(dst.join(&relative).join("long-path.txt")).unwrap(),
            b"long path content"
        );
        println!("260자 초과 경로가 현재 Windows/Rust 환경에서 정상 복사됨");
    } else {
        assert!(
            outcome
                .failures
                .iter()
                .any(|failure| failure.reason.contains("(code 206)")),
            "long-path failure should retain the mapped Windows reason: {:?}",
            outcome.failures
        );
        println!("260자 초과 경로가 code 206으로 차단됨");
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn progress_reports_each_file_with_running_counters() {
    let root = temp_dir("progress");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::write(src.join("a.txt"), b"one").unwrap();
    fs::write(src.join("nested/b.txt"), b"two").unwrap();

    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut reporter = |progress: SyncProgress<'_>| {
        seen.push((progress.current_path.to_string(), progress.copied));
    };
    let mut control = SyncControl::new().on_progress(&mut reporter);
    let outcome = run_sync_job_with_control(&job(&src, &dst), &mut control);
    drop(control);

    assert_eq!(outcome.copied, 2, "failures: {:?}", outcome.failures);
    let paths: Vec<&str> = seen.iter().map(|(path, _)| path.as_str()).collect();
    assert!(
        paths.contains(&"a.txt"),
        "progress should name the file being processed: {paths:?}"
    );
    assert!(
        paths.iter().any(|path| path.ends_with("b.txt")),
        "nested files should also be reported: {paths:?}"
    );
    // 보고는 처리 '직전'이므로 첫 보고의 복사 수는 아직 0이다.
    assert_eq!(seen[0].1, 0, "progress is reported before the copy: {seen:?}");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cancel_flag_stops_traversal_and_marks_outcome_cancelled() {
    let root = temp_dir("cancel");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    for index in 0..50 {
        fs::write(src.join(format!("file-{index:02}.txt")), b"payload").unwrap();
    }

    // 첫 파일을 처리하기 직전에 중지를 요청한다.
    let cancel = AtomicBool::new(false);
    let mut reporter = |_: SyncProgress<'_>| cancel.store(true, Ordering::Relaxed);
    let mut control = SyncControl::new()
        .cancel_flag(&cancel)
        .on_progress(&mut reporter);
    let outcome = run_sync_job_with_control(&job(&src, &dst), &mut control);
    drop(control);

    assert!(outcome.cancelled, "cancel flag should stop the traversal");
    assert!(
        outcome.copied < 50,
        "cancellation should leave files unprocessed, copied={}",
        outcome.copied
    );
    assert!(
        outcome.summary().contains("중지됨"),
        "summary should surface the cancellation: {}",
        outcome.summary()
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cancelled_run_does_not_mirror_delete_unvisited_targets() {
    let root = temp_dir("cancel-mirror");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    for index in 0..20 {
        fs::write(src.join(format!("keep-{index:02}.txt")), b"payload").unwrap();
    }
    // 원본에도 있는 파일이므로 정상 실행이었다면 절대 삭제되지 않아야 한다.
    fs::write(dst.join("keep-19.txt"), b"stale").unwrap();

    let cancel = AtomicBool::new(false);
    let mut reporter = |_: SyncProgress<'_>| cancel.store(true, Ordering::Relaxed);
    let mut control = SyncControl::new()
        .cancel_flag(&cancel)
        .on_progress(&mut reporter);
    let mut cancelled_job = job(&src, &dst);
    cancelled_job.mirror_deletes = true;
    let outcome = run_sync_job_with_control(&cancelled_job, &mut control);
    drop(control);

    assert!(outcome.cancelled);
    assert_eq!(
        outcome.deleted, 0,
        "an interrupted scan must not delete targets it never verified"
    );
    assert!(dst.join("keep-19.txt").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn windows_long_path_error_code_has_actionable_reason() {
    let error = std::io::Error::from_raw_os_error(206);
    let reason = describe_io_error(&error);
    assert!(reason.contains("경로가 너무 깁니다"));
    assert!(reason.contains("(code 206)"));
}

#[test]
#[ignore = "manual performance validation with 3,000 real files"]
fn syncs_three_thousand_files_and_reports_elapsed_time() {
    use std::time::Instant;

    const FILE_COUNT: usize = 3_000;
    let root = temp_dir("performance-3000");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    for index in 0..FILE_COUNT {
        fs::write(
            src.join(format!("file-{index:04}.txt")),
            format!("payload-{index}"),
        )
        .unwrap();
    }

    let started = Instant::now();
    let outcome = run_sync_job(&job(&src, &dst));
    let elapsed = started.elapsed();
    assert_eq!(
        outcome.copied, FILE_COUNT,
        "failures: {:?}",
        outcome.failures
    );
    assert!(!outcome.has_failures());
    println!(
        "3,000개 파일 1회 동기화: {:.3}초 ({:.0} files/sec)",
        elapsed.as_secs_f64(),
        FILE_COUNT as f64 / elapsed.as_secs_f64()
    );

    let _ = fs::remove_dir_all(&root);
}

// ─────────────────────────────────────────────
// 이어서 동기화
// ─────────────────────────────────────────────

/// 중지된 실행이 이어서 시작할 지점을 남기고, 다음 실행이 앞 구간을 다시 훑지 않는다.
#[test]
fn resuming_skips_everything_before_the_cursor() {
    let root = temp_dir("resume-flat");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fs::write(src.join(name), name.as_bytes()).unwrap();
    }

    let job = job(&src, &dst);
    let mut control = SyncControl::new().resume_from("c.txt");
    let outcome = run_sync_job_with_control(&job, &mut control);

    assert_eq!(
        outcome.copied, 2,
        "커서 이후(c·d)만 복사해야 한다: {:?}",
        outcome.failures
    );
    assert!(outcome.resumed, "이어서 실행했다는 사실이 결과에 남아야 한다");
    assert!(!dst.join("a.txt").exists(), "커서 앞 구간은 손대지 않는다");
    assert!(!dst.join("b.txt").exists());
    assert!(dst.join("c.txt").exists(), "커서가 가리킨 항목부터 처리한다");
    assert!(dst.join("d.txt").exists());

    let _ = fs::remove_dir_all(&root);
}

/// 커서가 하위 폴더를 가리키면 그 경로를 따라 내려가 이어서 시작한다.
#[test]
fn resuming_walks_into_the_directory_the_cursor_points_at() {
    let root = temp_dir("resume-nested");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(src.join("alpha")).unwrap();
    fs::create_dir_all(src.join("beta")).unwrap();
    fs::write(src.join("alpha/1.txt"), b"1").unwrap();
    fs::write(src.join("alpha/2.txt"), b"2").unwrap();
    fs::write(src.join("beta/3.txt"), b"3").unwrap();

    let job = job(&src, &dst);
    // 구분자는 기록 당시 형태 그대로 들어올 수 있으므로 `/`도 받아야 한다.
    let mut control = SyncControl::new().resume_from("alpha/2.txt");
    let outcome = run_sync_job_with_control(&job, &mut control);

    assert_eq!(outcome.copied, 2, "failures: {:?}", outcome.failures);
    assert!(
        !dst.join("alpha/1.txt").exists(),
        "커서 앞의 형제 파일은 건너뛴다"
    );
    assert!(dst.join("alpha/2.txt").exists());
    assert!(
        dst.join("beta/3.txt").exists(),
        "커서 폴더를 지난 뒤에는 평소대로 이어서 돈다"
    );

    let _ = fs::remove_dir_all(&root);
}

/// 이어서 실행하더라도 미러 삭제가 아직 확인하지 않은 원본의 대상본을 지우면 안 된다.
#[test]
fn resuming_never_mirror_deletes_files_it_has_not_examined() {
    let root = temp_dir("resume-mirror");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    for name in ["a.txt", "b.txt", "c.txt"] {
        fs::write(src.join(name), name.as_bytes()).unwrap();
        fs::write(dst.join(name), name.as_bytes()).unwrap();
    }
    // 원본에 없는 항목만 삭제 대상이다.
    fs::write(dst.join("stale.txt"), b"stale").unwrap();

    let mut job = job(&src, &dst);
    job.mirror_deletes = true;
    let mut control = SyncControl::new().resume_from("c.txt");
    let outcome = run_sync_job_with_control(&job, &mut control);

    assert!(
        dst.join("a.txt").exists() && dst.join("b.txt").exists(),
        "건너뛴 구간의 대상 파일이 삭제되면 데이터가 사라진다"
    );
    assert!(
        !dst.join("stale.txt").exists(),
        "원본에 없는 항목은 그대로 삭제된다"
    );
    assert_eq!(outcome.deleted, 1);

    let _ = fs::remove_dir_all(&root);
}

/// 커서가 없으면 예전과 똑같이 전체를 훑는다.
#[test]
fn a_run_without_a_cursor_still_walks_everything() {
    let root = temp_dir("resume-none");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(&src).unwrap();
    for name in ["a.txt", "b.txt"] {
        fs::write(src.join(name), name.as_bytes()).unwrap();
    }

    let outcome = run_sync_job(&job(&src, &dst));
    assert_eq!(outcome.copied, 2);
    assert!(!outcome.resumed);

    let _ = fs::remove_dir_all(&root);
}
