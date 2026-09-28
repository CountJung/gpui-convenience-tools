use super::*;

fn fixture(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("gct-links-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let source = root.join("src");
    let target = root.join("dst");
    fs::create_dir_all(&source).unwrap();
    (root, source, target)
}

fn job(source: &Path, target: &Path, mode: SymlinkMode) -> SyncJob {
    SyncJob {
        source: source.display().to_string(),
        target: target.display().to_string(),
        symlink_mode: mode,
        ..SyncJob::default()
    }
}

#[cfg(windows)]
#[test]
fn follows_internal_file_and_directory_links_without_escaping_root() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let (root, src, dst) = fixture("follow");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::write(src.join("nested/data.txt"), b"inside").unwrap();
    symlink_file(Path::new(r"nested\data.txt"), src.join("alias.txt")).unwrap();
    symlink_dir(Path::new("nested"), src.join("alias-dir")).unwrap();
    let configured = job(&src, &dst, SymlinkMode::Follow);
    assert_eq!(count_sync_entries(&configured), Some(3));
    let mut reports = Vec::new();
    let mut reporter = |progress: SyncProgress<'_>| reports.push(progress.current_path.to_string());
    let mut control = SyncControl::new().total(Some(3)).on_progress(&mut reporter);
    let result = run_sync_job_with_control(&configured, &mut control);
    assert!(result.failures.is_empty(), "{:?}", result.failures);
    assert_eq!(reports.len(), 3, "{reports:?}");
    assert_eq!(fs::read(dst.join("alias.txt")).unwrap(), b"inside");
    assert_eq!(fs::read(dst.join("alias-dir/data.txt")).unwrap(), b"inside");
    assert!(!links::is_link(
        &fs::symlink_metadata(dst.join("alias-dir")).unwrap()
    ));
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn rejects_outside_and_cyclic_links_with_path_specific_reasons() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let (root, src, dst) = fixture("unsafe");
    fs::write(root.join("outside.txt"), b"private").unwrap();
    symlink_file(root.join("outside.txt"), src.join("outside.txt")).unwrap();
    symlink_dir(&src, src.join("loop")).unwrap();
    let result = run_sync_job_with_control(
        &job(&src, &dst, SymlinkMode::Follow),
        &mut SyncControl::new(),
    );
    assert_eq!(result.failures.len(), 2, "{:?}", result.failures);
    assert!(result
        .failures
        .iter()
        .any(|f| f.path == "outside.txt" && f.reason.contains("원본 폴더 밖")));
    assert!(result
        .failures
        .iter()
        .any(|f| f.path == "loop" && f.reason.contains("순환 링크")));
    assert!(!dst.join("outside.txt").exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn recreates_internal_links_as_relative_destination_links() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let (root, src, dst) = fixture("recreate");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::write(src.join("nested/data.txt"), b"inside").unwrap();
    symlink_file(Path::new(r"nested\data.txt"), src.join("alias.txt")).unwrap();
    symlink_dir(Path::new("nested"), src.join("alias-dir")).unwrap();
    let configured = job(&src, &dst, SymlinkMode::Recreate);
    let first = run_sync_job_with_control(&configured, &mut SyncControl::new());
    assert!(first.failures.is_empty(), "{:?}", first.failures);
    assert_eq!(
        fs::read_link(dst.join("alias.txt")).unwrap(),
        Path::new(r"nested\data.txt")
    );
    assert_eq!(
        fs::read_link(dst.join("alias-dir")).unwrap(),
        Path::new("nested")
    );
    let second = run_sync_job_with_control(&configured, &mut SyncControl::new());
    assert!(second.failures.is_empty(), "{:?}", second.failures);
    assert_eq!(second.copied, 0);
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn recreate_preserves_colliding_regular_files_and_other_links() {
    use std::os::windows::fs::symlink_file;
    let (root, src, dst) = fixture("collision");
    fs::write(src.join("data.txt"), b"source").unwrap();
    symlink_file(Path::new("data.txt"), src.join("alias-file.txt")).unwrap();
    symlink_file(Path::new("data.txt"), src.join("alias-link.txt")).unwrap();
    fs::create_dir_all(&dst).unwrap();
    fs::write(dst.join("alias-file.txt"), b"preserve").unwrap();
    symlink_file(Path::new("elsewhere.txt"), dst.join("alias-link.txt")).unwrap();

    let result = run_sync_job_with_control(
        &job(&src, &dst, SymlinkMode::Recreate),
        &mut SyncControl::new(),
    );
    assert_eq!(result.failures.len(), 2, "{:?}", result.failures);
    assert!(result
        .failures
        .iter()
        .any(|failure| failure.path == "alias-file.txt"));
    assert!(result
        .failures
        .iter()
        .any(|failure| failure.path == "alias-link.txt"));
    assert_eq!(fs::read(dst.join("alias-file.txt")).unwrap(), b"preserve");
    assert_eq!(
        fs::read_link(dst.join("alias-link.txt")).unwrap(),
        Path::new("elsewhere.txt")
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn does_not_write_through_a_destination_junction() {
    use std::os::windows::fs::symlink_dir;
    let (root, src, dst) = fixture("destination-link");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::create_dir_all(&dst).unwrap();
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(src.join("nested/data.txt"), b"inside").unwrap();
    symlink_dir(&outside, dst.join("nested")).unwrap();
    let result = run_sync_job_with_control(
        &job(&src, &dst, SymlinkMode::Follow),
        &mut SyncControl::new(),
    );
    assert!(result
        .failures
        .iter()
        .any(|f| f.reason.contains("대상 경로에 링크")));
    assert!(!outside.join("data.txt").exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn rejects_a_junction_above_a_new_destination_root() {
    use std::os::windows::fs::symlink_dir;
    let (root, src, _) = fixture("parent-junction");
    fs::write(src.join("data.txt"), b"inside").unwrap();
    let outside = root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    let junction = root.join("alias");
    symlink_dir(&outside, &junction).unwrap();
    let target = junction.join("new-target");
    let result = run_sync_job_with_control(
        &job(&src, &target, SymlinkMode::Skip),
        &mut SyncControl::new(),
    );
    assert!(
        result
            .failures
            .iter()
            .any(|failure| failure.reason.contains("대상 경로에 링크")),
        "{:?}",
        result.failures
    );
    assert!(!outside.join("new-target").exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn rejects_an_uncreated_target_inside_source_with_different_path_case() {
    let (root, src, _) = fixture("nested-case");
    fs::write(src.join("data.txt"), b"inside").unwrap();
    let target = PathBuf::from(src.to_string_lossy().to_uppercase()).join("new-target");
    let result = run_sync_job_with_control(
        &job(&src, &target, SymlinkMode::Skip),
        &mut SyncControl::new(),
    );
    assert!(result
        .failures
        .iter()
        .any(|failure| failure.reason.contains("원본 폴더 내부")));
    assert!(!src.join("new-target").exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn handles_windows_junction_as_a_link_in_follow_and_recreate_modes() {
    let (root, src, dst) = fixture("junction");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::write(src.join("nested/data.txt"), b"inside").unwrap();
    let junction = src.join("junction");
    let created = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&junction)
        .arg(src.join("nested"))
        .output()
        .expect("mklink /J should start");
    assert!(
        created.status.success(),
        "mklink /J: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert!(links::is_link(&fs::symlink_metadata(&junction).unwrap()));

    let followed = run_sync_job_with_control(
        &job(&src, &dst, SymlinkMode::Follow),
        &mut SyncControl::new(),
    );
    assert!(followed.failures.is_empty(), "{:?}", followed.failures);
    assert_eq!(fs::read(dst.join("junction/data.txt")).unwrap(), b"inside");
    let _ = fs::remove_dir_all(&dst);
    let recreated = run_sync_job_with_control(
        &job(&src, &dst, SymlinkMode::Recreate),
        &mut SyncControl::new(),
    );
    assert!(recreated.failures.is_empty(), "{:?}", recreated.failures);
    assert!(links::is_link(
        &fs::symlink_metadata(dst.join("junction")).unwrap()
    ));
    assert_eq!(fs::read(dst.join("junction/data.txt")).unwrap(), b"inside");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn browse_lists_hidden_names_and_rejects_path_escape() {
    let (root, src, _) = fixture("browse");
    fs::create_dir_all(src.join(".secret")).unwrap();
    fs::write(src.join(".secret/data.txt"), b"hidden").unwrap();
    let entries = browse::list(&src, Path::new("")).unwrap();
    assert!(entries
        .iter()
        .any(|e| e.name == ".secret" && e.is_directory));
    assert!(browse::list(&src, Path::new("../outside")).is_err());
    let _ = fs::remove_dir_all(root);
}
