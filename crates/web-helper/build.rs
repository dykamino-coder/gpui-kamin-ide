//! Ресурсы Windows помощника: та же иконка, но СВОЁ описание — по нему
//! диспетчер задач подписывает строки дочерних процессов CEF.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "Cargo build script emits directives and build diagnostics"
)]

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../shell/assets/app/kaminide.ico");
        let mut res = winres::WindowsResource::new();
        res.set_icon("../shell/assets/app/kaminide.ico");
        res.set("FileDescription", "KaminIDE Web");
        res.set("ProductName", "KaminIDE");
        res.compile().expect("ресурсы Windows (иконка помощника)");
    }
}
