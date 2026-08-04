//! Бутстрап дочерних процессов CEF (renderer, gpu, utility, network).
//!
//! Здесь НИЧЕГО, кроме передачи управления libcef: ни probe, ни окна, ни
//! kamin-host. Главный процесс кладёт копию этого exe в `web\kaminide-gpui.exe`
//! (см. `crates/shell/src/web/process.rs`, там же — зачем именно так).

#![windows_subsystem = "windows"]

use cef::rc::*;
use cef::*;

wrap_app! {
    struct WebHelperApp;
    impl App {}
}

fn main() {
    // Рукопожатие версий обязательно любому процессу CEF: без него libcef
    // обрывает работу с «CefApp called with invalid version -1».
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = cef::args::Args::new();
    let mut app = WebHelperApp::new();
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    std::process::exit(code.max(0));
}
