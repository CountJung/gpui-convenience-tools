//! 폴더 동기화 엔진.
//!
//! 원본 폴더의 파일을 대상 폴더로 주기적으로 복사한다. 숨김·시스템 속성을 포함한
//! 모든 파일을 대상으로 하며, 복사할 수 없는 파일은 건너뛰되 사유를 기록해
//! UI(토스트/로그)로 전달한다.
//!
//! 동기화 판정은 `(크기, 수정 시각)` 비교다. 원본이 더 최신이거나 크기가 다르면 복사한다.
//! 내용 해시 비교는 하지 않는다(대용량 폴더에서 비용이 크기 때문).

use std::{
    cmp::Ordering as CmpOrdering,
    collections::BTreeSet,
    ffi::OsString,
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
    resume: Option<Resume>,
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

    /// 중단된 순회를 이 상대 경로부터 이어서 시작한다.
    ///
    /// 지정한 경로보다 **앞서는** 항목만 건너뛴다. 경로 자체는 다시 처리하는데, 커서가
    /// "처리 직전"에 기록되므로 그 파일이 실제로 끝났는지 알 수 없기 때문이다.
    /// 한 파일을 다시 보는 비용은 (변경이 없으면) 건너뛰기 한 번이라 무시할 수 있다.
    pub fn resume_from(mut self, relative_path: &str) -> Self {
        let components = split_relative(relative_path);
        if !components.is_empty() {
            self.resume = Some(Resume { components });
        }
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

    /// `depth` 깊이에서 만난 `name`을 이어서 시작 지점과 견준다.
    ///
    /// 반환값이 `Less`면 이미 처리한 구간이므로 건너뛴다. `Greater`면 이어서 시작할
    /// 지점을 지났다는 뜻이라 그 자리에서 탐색을 끝낸다.
    fn resume_probe(&mut self, depth: usize, name: &OsString) -> ResumeProbe {
        let Some(resume) = self.resume.as_ref() else {
            return ResumeProbe::Process;
        };
        let Some(expected) = resume.components.get(depth) else {
            // 커서보다 깊은 자리다. 커서 경로 위에 있으므로 정상 처리한다.
            self.resume = None;
            return ResumeProbe::Process;
        };

        match name.cmp(expected) {
            CmpOrdering::Less => ResumeProbe::Skip,
            CmpOrdering::Greater => {
                self.resume = None;
                ResumeProbe::Process
            }
            CmpOrdering::Equal => {
                if depth + 1 >= resume.components.len() {
                    // 커서가 가리키는 항목 자체다. 여기서부터 정상 처리한다.
                    self.resume = None;
                    ResumeProbe::Process
                } else {
                    ResumeProbe::Descend
                }
            }
        }
    }

    /// 이어서 시작하는 순회인지. 미러 삭제를 미루는 판단에 쓴다.
    fn is_resuming(&self) -> bool {
        self.resume.is_some()
    }
}

/// 이어서 시작할 지점을 경로 컴포넌트로 쪼갠 것.
struct Resume {
    components: Vec<OsString>,
}

enum ResumeProbe {
    /// 이미 지나온 구간 — 건너뛴다.
    Skip,
    /// 커서 경로 위의 디렉터리 — 안으로 들어가 계속 찾는다.
    Descend,
    /// 이어서 시작할 지점에 도달했다 — 평소대로 처리한다.
    Process,
}

/// 상대 경로 문자열을 컴포넌트로 쪼갠다.
///
/// config에 저장된 커서는 기록 당시의 구분자를 그대로 갖고 있으므로 `/`와 `\`를 모두 받는다.
fn split_relative(path: &str) -> Vec<OsString> {
    path.split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .map(OsString::from)
        .collect()
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
    /// 이전에 끊긴 지점부터 이어서 실행했는지 여부.
    ///
    /// 이 실행은 원본 전체를 훑지 않았으므로 요약에 그 사실을 밝힌다.
    pub resumed: bool,
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
        if self.resumed {
            s.push_str(" — 이어서 실행");
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

    // 이어서 시작하는 순회는 원본 일부만 훑으므로, 그 사실을 미리 붙잡아 둔다.
    let resumed = control.is_resuming();
    sync_dir(&source, &target, &source, 0, job, control, &mut outcome);
    outcome.resumed = resumed;
    outcome
}

/// `source` 하위를 재귀적으로 순회하며 `target`에 반영한다.
///
/// `root`는 실패 메시지에 표시할 상대 경로 계산 기준, `depth`는 이어서 시작 지점을 찾을 때
/// 견줄 경로 컴포넌트의 자리다.
fn sync_dir(
    source: &Path,
    target: &Path,
    root: &Path,
    depth: usize,
    job: &SyncJob,
    control: &mut SyncControl<'_>,
    outcome: &mut SyncOutcome,
) {
    let read_dir = match fs::read_dir(source) {
        Ok(entries) => entries,
        Err(err) => {
            outcome.fail(relative_label(source, root), describe_io_error(&err));
            return;
        }
    };

    // 이어서 시작하려면 순회 순서가 실행마다 같아야 한다. `read_dir`는 순서를 보장하지
    // 않으므로 이름으로 정렬해 커서 비교가 성립하게 만든다.
    let mut entries: Vec<_> = read_dir.collect();
    entries.sort_by_key(|entry| match entry {
        Ok(entry) => entry.file_name(),
        Err(_) => OsString::new(),
    });

    // mirror_deletes 처리를 위해 원본에 존재하는 이름을 모아둔다.
    let mut seen_names = BTreeSet::<OsString>::new();

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
        // 건너뛰는 항목도 이름은 남긴다. 그래야 이 디렉터리의 미러 삭제가 아직 확인하지
        // 않은 원본 파일의 대상본을 지우지 않는다.
        seen_names.insert(name.clone());
        let dst_path = target.join(&name);

        let mut resume_depth = depth;
        match control.resume_probe(depth, &name) {
            ResumeProbe::Skip => continue,
            ResumeProbe::Descend => resume_depth = depth + 1,
            ResumeProbe::Process => {}
        }

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
            sync_dir(&src_path, &dst_path, root, resume_depth, job, control, outcome);
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
    //
    // 이어서 시작한 순회는 이 디렉터리의 이름을 모두 모았으므로(건너뛴 항목도 넣는다)
    // 여기서는 안전하다. 다만 통째로 건너뛴 하위 디렉터리 안쪽은 이번에 확인하지 못했고,
    // 그건 삭제가 다음 실행으로 미뤄질 뿐 잘못 지우는 문제가 아니다.
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
mod tests;
