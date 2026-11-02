fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../src-tauri/icons/icon.ico");
        let _ = res.compile();
    }
}
