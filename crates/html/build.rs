//! Ограничения тестов, зависящие от уровня оптимизации крейта.

#[allow(
    clippy::print_stdout,
    reason = "Cargo build scripts communicate configuration through stdout directives"
)]
#[allow(
    clippy::disallowed_methods,
    reason = "Cargo supplies this crate's optimization level to its build script"
)]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(kamin_unoptimized)");
    if std::env::var("OPT_LEVEL").as_deref() == Ok("0") {
        println!("cargo:rustc-cfg=kamin_unoptimized");
    }
}
