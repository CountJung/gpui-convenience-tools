//! 실제 VM 없이 앱의 VBoxManage 프로세스 경계를 검증하는 테스트 대역.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::{env, fs::OpenOptions, io::Write};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let trace = env::var("GPUI_O5_PROBE_TRACE").expect("isolated trace path");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(trace)
        .unwrap();
    writeln!(file, "{}", args.join(" ")).unwrap();

    if env::var("GPUI_O5_PROBE_MODE").as_deref() == Ok("QueryFailure") {
        eprintln!("O5_PROBE_QUERY_FAILURE");
        std::process::exit(7);
    }
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["list", "runningvms"] => {
            if env::var("GPUI_O5_PROBE_MODE").as_deref() == Ok("Running") {
                println!("\"O5 fixture VM\" {{00000000-0000-0000-0000-000000000005}}");
            }
        }
        ["showvminfo", "00000000-0000-0000-0000-000000000005", "--machinereadable"] => {
            let path = env::var("GPUI_O5_PROBE_VDI").expect("isolated disk path");
            println!("VMState=\"running\"\nSATA-0-0=\"{path}\"");
        }
        _ => std::process::exit(9),
    }
}
