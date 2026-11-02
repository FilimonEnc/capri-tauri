fn main() {
    let mut res = tauri_winres::WindowsResource::new();
    res.set_icon("../src-tauri/icons/icon.ico");
    let _ = res.compile();
}
