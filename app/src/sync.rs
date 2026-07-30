//! 폴더 동기화 엔진.
//!
//! 원본 폴더의 파일을 대상 폴더로 주기적으로 복사한다. 숨김·시스템 속성을 포함한
//! 모든 파일을 대상으로 하며, 복사할 수 없는 파일은 건너뛰되 사유를 기록해
//! UI(토스트/로그)로 전달한다.
//!
//! 동기화 판정은 `(크기, 수정 시각)` 비교다. 원본이 더 최신이거나 크기가 다르면 복사한다.
//! 내용 해시 비교는 하지 않는다(대용량 폴더에서 비용이 크기 때문).

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

use crate::config::SyncJob;

/// 파일 하나를 처리하기 직전에 보고되는 진행 상황.
///
/// 경로는 소유하지 않고 빌려준다. 호출자가 필요할 때만 복사하도록 해서
/// 파일 수천 개짜리 동기화에서 불필요한 할당이 생기지 않게 한다.
#[derive(Clone, Copy, Debug)]
pub struct SyncProgress<'a> {
    /// 원본 기준 상대 경로.
    pub current_path: &'a str,
    pub copied: usize,
    pub skipped: usize,
    pub failed: usize,
}

/// 진행 보고와 중지 요청을 엔진에 연결하는 제어 핸들.
///
/// 엔진을 UI에 의존시키지 않기 위해 채널이 아니라 콜백과 플래그만 받는다.
/// 보고 빈도 조절(throttling)은 호출자 몫이다 — 엔진은 파일마다 그대로 보고한다.
#[derive(Default)]
pub struct SyncControl<'a> {
    cancel: Option<&'a AtomicBool>,
    on_progress: Option<&'a mut dyn FnMut(SyncProgress<'_>)>,
}

impl<'a> SyncControl<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// 중지 플래그를 연결한다. 엔진은 파일·디렉터리 단위로 확인한다.
    pub fn cancel_flag(mut self, flag: &'a AtomicBool) -> Self {
        self.cancel = Some(flag);
        self
    }

    /// 진행 상황 콜백을 연결한다.
    pub fn on_progress(mut self, reporter: &'a mut dyn FnMut(SyncProgress<'_>)) -> Self {
        self.on_progress = Some(reporter);
        self
    }

    fn is_cancelled(&self) -> bool {
        self.cancel
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    }

    fn report(&mut self, current_path: &str, outcome: &SyncOutcome) {
        let Some(reporter) = self.on_progress.as_mut() else {
            return;
        };
        reporter(SyncProgress {
            current_path,
            copied: outcome.copied,
            skipped: outcome.skipped,
            failed: outcome.failures.len() + outcome.truncated_failures,
        });
    }
}

/// 한 파일에 대한 동기화 실패 기록.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncFailure {
    /// 원본 기준 상대 경로(표시용).
    pub path: String,
    /// 사람이 읽을 수 있는 실패 사유.
    pub reason: String,
}

impl SyncFailure {
    /// 알림 억제(suppression)에 사용할 키. 같은 파일의 같은 사유는 한 번만 알린다.
    pub fn key(&self) -> String {
        format!("{}|{}", self.path, self.reason)
    }
}

/// 동기화 1회 실행 결과.
#[derive(Clone, Debug, Default)]
pub struct SyncOutcome {
    pub copied: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub failures: Vec<SyncFailure>,
    /// 실패가 너무 많아 목록에 담지 못한 개수.
    pub truncated_failures: usize,
    /// 사용자 중지 요청으로 중간에 끝났는지 여부.
    pub cancelled: bool,
}

/// UI 목록이 무한정 길어지지 않도록 실패 기록 상한을 둔다.
const MAX_FAILURES: usize = 100;

impl SyncOutcome {
    fn fail(&mut self, path: impl Into<String>, reason: impl Into<String>) {
        if self.failures.len() >= MAX_FAILURES {
            self.truncated_failures += 1;
            return;
        }
        self.failures.push(SyncFailure {
            path: path.into(),
            reason: reason.into(),
        });
    }

    pub fn has_failures(&self) -> bool {
        !self.failures.is_empty() || self.truncated_failures > 0
    }

    /// 로그 한 줄 요약.
    pub fn summary(&self) -> String {
        let mut s = format!("복사 {}건, 건너뜀 {}건", self.copied, self.skipped);
        if self.deleted > 0 {
            s.push_str(&format!(", 삭제 {}건", self.deleted));
        }
        let failed = self.failures.len() + self.truncated_failures;
        if failed > 0 {
            s.push_str(&format!(", 실패 {failed}건"));
        }
        if self.cancelled {
            s.push_str(" — 중지됨");
        }
        s
    }
}

/// 동기화 작업 하나를 1회 실행한다.
///
/// 제어가 필요 없으면 `SyncControl::new()`를 그대로 넘긴다.
pub fn run_sync_job_with_control(job: &SyncJob, control: &mut SyncControl<'_>) -> SyncOutcome {
    let mut outcome = SyncOutcome::default();

    let source = PathBuf::from(job.source.trim());
    let target = PathBuf::from(job.target.trim());

    if job.source.trim().is_empty() || job.target.trim().is_empty() {
        outcome.fail("(설정)", "원본 또는 대상 폴더가 비어 있습니다.");
        return outcome;
    }

    if !source.is_dir() {
        outcome.fail(
            job.source.clone(),
            "원본 폴더가 존재하지 않거나 폴더가 아닙니다.",
        );
        return outcome;
    }

    // 대상이 원본 안에 있으면 무한 복사가 발생하므로 차단한다.
    if is_inside(&target, &source) {
        outcome.fail(
            job.target.clone(),
            "대상 폴더가 원본 폴더 내부에 있어 동기화할 수 없습니다.",
        );
        return outcome;
    }

    if let Err(err) = fs::create_dir_all(&target) {
        outcome.fail(job.target.clone(), describe_io_error(&err));
        return outcome;
    }

    sync_dir(&source, &target, &source, job, control, &mut outcome);
    outcome
}

/// `source` 하위를 재귀적으로 순회하며 `target`에 반영한다.
///
/// `root`는 실패 메시지에 표시할 상대 경로 계산 기준이다.
fn sync_dir(
    source: &Path,
    target: &Path,
    root: &Path,
    job: &SyncJob,
    control: &mut SyncControl<'_>,
    outcome: &mut SyncOutcome,
) {
    let entries = match fs::read_dir(source) {
        Ok(entries) => entries,
        Err(err) => {
            outcome.fail(relative_label(source, root), describe_io_error(&err));
            return;
        }
    };

    // mirror_deletes 처리를 위해 원본에 존재하는 이름을 모아둔다.
    let mut seen_names = BTreeSet::<std::ffi::OsString>::new();

    for entry in entries {
        // 중지 요청은 파일 단위로 확인한다. 이미 복사한 파일은 되돌리지 않고 그대로 둔다.
        if control.is_cancelled() {
            outcome.cancelled = true;
            return;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                outcome.fail(relative_label(source, root), describe_io_error(&err));
                continue;
            }
        };

        let src_path = entry.path();
        let name = entry.file_name();
        seen_names.insert(name.clone());
        let dst_path = target.join(&name);

        // symlink_metadata: 심볼릭 링크를 따라가지 않고 링크 자체를 본다.
        let meta = match fs::symlink_metadata(&src_path) {
            Ok(meta) => meta,
            Err(err) => {
                outcome.fail(relative_label(&src_path, root), describe_io_error(&err));
                continue;
            }
        };

        if !job.include_hidden && has_hidden_or_system_attribute(&src_path, &meta) {
            outcome.skipped += 1;
            continue;
        }

        if meta.file_type().is_symlink() {
            // 링크를 그대로 복제하려면 권한과 대상 종류 판별이 필요하다.
            // 현재는 건너뛰고 사유를 남긴다.
            outcome.fail(
                relative_label(&src_path, root),
                "심볼릭 링크/정션은 복사 대상에서 제외됩니다.",
            );
            continue;
        }

        if meta.is_dir() {
            if let Err(err) = fs::create_dir_all(&dst_path) {
                outcome.fail(relative_label(&dst_path, root), describe_io_error(&err));
                continue;
            }
            sync_dir(&src_path, &dst_path, root, job, control, outcome);
            if outcome.cancelled {
                return;
            }
            continue;
        }

        // 건너뛰는 파일도 보고해야 대용량 폴더에서 화면이 멈춘 것처럼 보이지 않는다.
        control.report(&relative_label(&src_path, root), outcome);

        match needs_copy(&dst_path, &meta) {
            Ok(false) => {
                outcome.skipped += 1;
            }
            Ok(true) => match copy_file(&src_path, &dst_path) {
                Ok(()) => outcome.copied += 1,
                Err(err) => {
                    outcome.fail(relative_label(&src_path, root), err);
                }
            },
            Err(err) => {
                outcome.fail(relative_label(&dst_path, root), describe_io_error(&err));
            }
        }
    }

    // 중지된 순회는 `seen_names`가 불완전할 수 있다. 그 상태로 미러 삭제를 돌리면
    // 아직 확인하지 않은 원본 파일의 대상본을 지우게 되므로 실행하지 않는다.
    if job.mirror_deletes && !outcome.cancelled {
        mirror_deletes(target, root, &seen_names, outcome);
    }
}

/// 원본에 없는 대상 항목을 삭제한다.
fn mirror_deletes(
    target: &Path,
    root: &Path,
    seen_names: &BTreeSet<std::ffi::OsString>,
    outcome: &mut SyncOutcome,
) {
    let entries = match fs::read_dir(target) {
        Ok(entries) => entries,
        Err(err) => {
            outcome.fail(relative_label(target, root), describe_io_error(&err));
            return;
        }
    };

    for entry in entries.flatten() {
        if seen_names.contains(&entry.file_name()) {
            continue;
        }

        let path = entry.path();
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            clear_readonly(&path);
            fs::remove_file(&path)
        };

        match result {
            Ok(()) => outcome.deleted += 1,
            Err(err) => outcome.fail(path.display().to_string(), describe_io_error(&err)),
        }
    }
}

/// 원본이 대상보다 새롭거나 크기가 다르면 복사가 필요하다.
fn needs_copy(dst: &Path, src_meta: &fs::Metadata) -> std::io::Result<bool> {
    let dst_meta = match fs::metadata(dst) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(err) => return Err(err),
    };

    if dst_meta.len() != src_meta.len() {
        return Ok(true);
    }

    let src_time = src_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let dst_time = dst_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    // 파일 시스템 간 타임스탬프 정밀도 차이를 고려해 2초 여유를 둔다(FAT 계열 대응).
    Ok(match src_time.duration_since(dst_time) {
        Ok(diff) => diff.as_secs() > 2,
        Err(_) => false,
    })
}

/// 파일 하나를 복사한다. 대상이 읽기 전용이면 속성을 해제한 뒤 덮어쓴다.
fn copy_file(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        clear_readonly(dst);
    }

    match fs::copy(src, dst) {
        Ok(_) => Ok(()),
        Err(err) => Err(describe_io_error(&err)),
    }
}

/// 대상 파일의 읽기 전용 속성을 해제한다(실패는 무시하고 복사에서 사유가 드러나게 한다).
fn clear_readonly(path: &Path) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    let mut perms = meta.permissions();
    if perms.readonly() {
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = fs::set_permissions(path, perms);
    }
}

/// 숨김 속성 여부.
#[cfg(target_os = "windows")]
fn has_hidden_or_system_attribute(_path: &Path, meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(not(target_os = "windows"))]
fn has_hidden_or_system_attribute(path: &Path, _meta: &fs::Metadata) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// `candidate`가 `base` 내부(또는 동일 경로)인지 검사한다.
fn is_inside(candidate: &Path, base: &Path) -> bool {
    let candidate = fs::canonicalize(candidate).unwrap_or_else(|_| candidate.to_path_buf());
    let base = fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    candidate.starts_with(&base)
}

/// 루트 기준 상대 경로 문자열. 상대화에 실패하면 전체 경로를 쓴다.
fn relative_label(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

/// io::Error를 사용자에게 보여줄 한국어 사유로 변환한다.
///
/// Windows 오류 코드는 메시지만으로는 원인 파악이 어려워 대표적인 것들을 매핑한다.
fn describe_io_error(err: &std::io::Error) -> String {
    use std::io::ErrorKind;

    let base = match err.kind() {
        ErrorKind::PermissionDenied => "접근이 거부되었습니다(권한 부족).",
        ErrorKind::NotFound => "파일 또는 경로를 찾을 수 없습니다.",
        ErrorKind::AlreadyExists => "대상에 같은 이름의 항목이 이미 있습니다.",
        _ => "",
    };

    // Windows 전용 코드 보강
    let os_specific = err.raw_os_error().and_then(|code| match code {
        5 => Some("접근이 거부되었습니다(관리자 권한이 필요할 수 있습니다)."),
        32 => Some("다른 프로세스가 파일을 사용 중입니다(공유 위반)."),
        33 => Some("파일 일부가 잠겨 있습니다."),
        112 => Some("대상 디스크 공간이 부족합니다."),
        206 => Some("경로가 너무 깁니다(260자 제한)."),
        1920 => Some("시스템이 파일에 접근할 수 없습니다."),
        _ => None,
    });

    if let Some(msg) = os_specific {
        return format!("{msg} (code {})", err.raw_os_error().unwrap_or(0));
    }

    if !base.is_empty() {
        return base.to_string();
    }

    format!("{err}")
}

#[cfg(test)]
mod tests {
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
}
