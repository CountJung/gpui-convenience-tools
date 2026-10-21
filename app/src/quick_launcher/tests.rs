use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "gpui-launcher-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }
    fn file(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, b"fixture").unwrap();
        path
    }
    fn entry(&self, name: &str, kind: LauncherKind) -> LauncherEntry {
        LauncherEntry {
            name: "fixture".into(),
            path: self.file(name).to_string_lossy().into_owned(),
            trusted: true,
            kind,
            args: vec![],
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Delete only our uniquely-created temporary fixture directory, never application data.
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn launcher_args_are_json_argv_not_shell_commands() {
    let args = parse_args(
        r#"["a b","& whoami","; echo nope","$(calc)","quote\"inside","", "back\\slash"]"#,
    )
    .unwrap();
    assert_eq!(args[0], "a b");
    assert_eq!(args[3], "$(calc)");
    for invalid in ["cmd /c calc", "{}", "[1]", r#"["\u0000"]"#] {
        assert_eq!(parse_args(invalid), Err(LauncherError::InvalidArguments));
    }
    assert!(parse_args(&serde_json::to_string(&vec!["x"; 65]).unwrap()).is_err());
    assert!(parse_args(&serde_json::to_string(&vec!["x".repeat(16385)]).unwrap()).is_err());
}

#[test]
fn launcher_rejects_untrusted_relative_remote_and_unsupported_targets() {
    let fixture = Fixture::new();
    let mut entry = fixture.entry("fixture.exe", LauncherKind::Application);
    assert!(validate_entry(&entry).is_ok());
    entry.trusted = false;
    assert_eq!(validate_entry(&entry), Err(LauncherError::Untrusted));
    entry.trusted = true;
    for path in [
        "fixture.exe",
        r"\\server\share\fixture.exe",
        r"\\?\C:\fixture.exe",
        "https://example.com",
    ] {
        entry.path = path.into();
        assert_eq!(validate_entry(&entry), Err(LauncherError::InvalidPath));
    }
    for filename in [
        "script.cmd",
        "script.bat",
        "shortcut.lnk",
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "mshta.exe",
        "wscript.exe",
    ] {
        let entry = fixture.entry(filename, LauncherKind::Application);
        assert_eq!(
            validate_entry(&entry),
            Err(LauncherError::UnsupportedTarget)
        );
    }
    let mut folder = LauncherEntry {
        trusted: true,
        kind: LauncherKind::Folder,
        path: fixture.root.to_string_lossy().into_owned(),
        ..Default::default()
    };
    assert!(validate_entry(&folder).is_ok());
    folder.args.push("/select".into());
    assert_eq!(validate_entry(&folder), Err(LauncherError::FolderArguments));
}

#[test]
fn launcher_script_requires_explicit_confirmation_and_fixed_file_interpreter() {
    let fixture = Fixture::new();
    let mut script = fixture.entry("safe fixture.ps1", LauncherKind::Script);
    script.args = vec!["hello world".into(), "& literal".into()];
    assert!(matches!(
        plan(&script, false),
        Err(LauncherError::ConfirmationRequired)
    ));
    #[cfg(windows)]
    {
        let plan = plan(&script, true).unwrap();
        assert_eq!(plan.program, platform::powershell().unwrap());
        assert_eq!(plan.args[3], "-File");
        assert!(plan.args[4].ends_with("safe fixture.ps1"));
        assert_eq!(&plan.args[5..], &script.args);
        assert!(!plan
            .args
            .iter()
            .any(|arg| arg == "-Command" || arg == "-ExecutionPolicy"));
    }
    fs::remove_file(&script.path).unwrap();
    assert!(matches!(
        plan(&script, true),
        Err(LauncherError::InvalidPath)
    ));
}

#[cfg(windows)]
#[test]
fn launcher_script_fixture_runs_only_after_confirmation_or_reports_policy_block() {
    let fixture = Fixture::new();
    let script = fixture.root.join("harmless fixture.ps1");
    let marker = fixture.root.join("script-marker.log");
    let diagnostics = fixture.root.join("script-stderr.log");
    // Self-contained ASCII source: no profiles, UI, network, or user-file access.
    fs::write(
        &script,
        r#"param([string]$Marker)
$ErrorActionPreference = 'Stop'
$payload = @{ marker = 'launcher-script-fixture-v1'; args = @($args) } | ConvertTo-Json -Compress
[System.IO.File]::WriteAllText($Marker, $payload, [System.Text.UTF8Encoding]::new($false))
"#,
    )
    .unwrap();
    let expected_args = ["space value", "& literal", "$(not-executed)"];
    let mut entry = LauncherEntry {
        name: "Harmless script fixture".into(),
        path: script.to_str().unwrap().to_owned(),
        kind: LauncherKind::Script,
        trusted: true,
        args: vec![marker.to_str().unwrap().to_owned()],
    };
    entry.args.extend(expected_args.map(str::to_owned));

    // Exercise the same plan -> spawn boundary, not just the denial enum.
    assert!(matches!(
        plan(&entry, false).and_then(|execution| execution.spawn()),
        Err(LauncherError::ConfirmationRequired)
    ));
    assert!(!marker.exists(), "denied launch must not create a marker");

    let execution = plan(&entry, true).unwrap();
    assert_eq!(execution.program, platform::powershell().unwrap());
    assert_eq!(execution.args[3], "-File");
    assert!(!execution
        .args
        .iter()
        .any(|arg| arg == "-Command" || arg == "-ExecutionPolicy"));
    // Retain production CREATE_NO_WINDOW/null stdin/stdout. Capture stderr only inside our
    // fixture to identify a policy denial without printing paths or changing machine policy.
    let mut child = execution
        .command()
        .current_dir(&fixture.root)
        .stderr(Stdio::from(fs::File::create(&diagnostics).unwrap()))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("own PowerShell fixture timed out and was terminated");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if !status.success() {
        let stderr = fs::read(&diagnostics).unwrap();
        let stderr = String::from_utf8_lossy(&stderr);
        assert!(
            !marker.exists()
                && stderr.contains("PSSecurityException")
                && stderr.contains("UnauthorizedAccess"),
            "script failed without a recognized execution-policy denial (exit {:?})",
            status.code()
        );
        println!("POLICY_BLOCKED: unsigned fixture denied; marker absent; policy left unchanged");
        return;
    }
    let actual: serde_json::Value =
        serde_json::from_slice(&fs::read(&marker).expect("successful fixture must write marker"))
            .unwrap();
    assert_eq!(actual["marker"], "launcher-script-fixture-v1");
    assert_eq!(actual["args"], serde_json::json!(expected_args));
    println!("EXECUTED: confirmed -File fixture wrote its marker with exact literal arguments");
}

#[cfg(windows)]
#[test]
fn launcher_drive_roots_allow_folder_registration_and_execution_plan() {
    // Syntax checks do not require either sample drive to exist.
    for root in [r"C:\", r"D:\", "C:/", "D:/"] {
        assert!(
            local_absolute(Path::new(root)),
            "drive root must be absolute"
        );
    }
    for relative in ["C:", "D:folder", r"\"] {
        assert!(!local_absolute(Path::new(relative)));
    }

    // Read only an existing root's metadata. Never enumerate, write to, or open the drive.
    let root = std::env::temp_dir()
        .ancestors()
        .last()
        .unwrap()
        .to_path_buf();
    let mut entry = LauncherEntry {
        name: "Drive root fixture".into(),
        path: root.to_str().unwrap().to_owned(),
        kind: LauncherKind::Folder,
        trusted: true,
        ..Default::default()
    };
    let registered = validate_entry(&entry).expect("folder registration must accept drive root");
    assert_eq!(registered.to_str().unwrap().len(), 3);
    entry.path = registered.to_str().unwrap().to_owned();
    let execution = plan(&entry, false).expect("folder plan must accept registered drive root");
    assert_eq!(execution.program, platform::explorer().unwrap());
    assert_eq!(execution.args, vec![entry.path.clone()]);
    assert_eq!(execution.command().get_args().count(), 1);

    entry.trusted = false;
    assert_eq!(validate_entry(&entry), Err(LauncherError::Untrusted));
    entry.trusted = true;
    entry.args.push("/select".into());
    assert_eq!(validate_entry(&entry), Err(LauncherError::FolderArguments));
}

#[test]
fn launcher_search_matches_names_paths_case_insensitively() {
    let entries = vec![
        LauncherEntry {
            name: "Editor".into(),
            path: r"C:\Tools\Code.exe".into(),
            ..Default::default()
        },
        LauncherEntry {
            name: "Logs".into(),
            path: r"C:\Reports".into(),
            ..Default::default()
        },
    ];
    assert_eq!(matching_indices(&entries, " EDIT "), vec![0]);
    assert_eq!(matching_indices(&entries, "reports"), vec![1]);
    assert_eq!(matching_indices(&entries, ""), vec![0, 1]);
    assert!(matching_indices(&entries, "missing").is_empty());
}

#[test]
fn launcher_runs_only_its_harmless_fixture_with_literal_arguments() {
    let fixture = Fixture::new();
    let source = fixture.root.join("fixture.rs");
    let output = fixture.root.join("argv.log");
    let executable = fixture.root.join("fixture.exe");
    fs::write(
        &source,
        r#"fn main() {
        let mut args = std::env::args().skip(1);
        let output = args.next().expect("output");
        std::fs::write(output, args.collect::<Vec<_>>().join("\n")).unwrap();
    }"#,
    )
    .unwrap();
    let status = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("--crate-name")
        .arg("launcher_fixture")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    let literal = [
        "space value",
        "& echo injection",
        "$(calc)",
        "a\"b",
        "",
        "path\\tail",
    ];
    let mut args = vec![output.to_string_lossy().into_owned()];
    args.extend(literal.iter().map(|s| s.to_string()));
    let entry = LauncherEntry {
        name: "Harmless fixture".into(),
        path: executable.to_string_lossy().into_owned(),
        args,
        trusted: true,
        ..Default::default()
    };
    let plan = plan(&entry, false).unwrap();
    assert_eq!(
        plan.command().get_args().collect::<Vec<_>>(),
        entry
            .args
            .iter()
            .map(std::ffi::OsStr::new)
            .collect::<Vec<_>>()
    );
    let mut child = plan.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("own fixture timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(fs::read_to_string(output).unwrap(), literal.join("\n"));
}
