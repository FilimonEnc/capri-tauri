#![windows_subsystem = "windows"]

use std::env;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

const APP_BYTES: &[u8] = include_bytes!("../../src-tauri/target/release/capri.exe");
const DLL_BYTES: &[u8] = include_bytes!("../../src-tauri/target/release/WebView2Loader.dll");

fn main() {
    let local_app_data = env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let bin_dir = PathBuf::from(local_app_data).join("Capri").join("bin");
    let _ = fs::create_dir_all(&bin_dir);

    let dll_path = bin_dir.join("WebView2Loader.dll");
    let exe_path = bin_dir.join("CapriCore.exe");

    let need_dll = !dll_path.exists()
        || fs::metadata(&dll_path).map(|m| m.len()).unwrap_or(0) != DLL_BYTES.len() as u64;
    if need_dll {
        let _ = fs::write(&dll_path, DLL_BYTES);
    }

    let need_exe = !exe_path.exists()
        || fs::metadata(&exe_path).map(|m| m.len()).unwrap_or(0) != APP_BYTES.len() as u64;
    if need_exe {
        let _ = fs::write(&exe_path, APP_BYTES);
    }

    let mut cmd = Command::new(&exe_path);
    cmd.current_dir(&bin_dir);
    cmd.args(env::args().skip(1));
    cmd.creation_flags(0x00000200 | 0x00000008);
    let _ = cmd.spawn();
}

