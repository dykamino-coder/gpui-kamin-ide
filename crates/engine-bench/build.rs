//! Что это за сборка — в JSON стенда (`build`): имя профиля cargo, уровень
//! оптимизации и переопределения профиля из окружения. Сравнение двух exe с
//! разными features или профилем — не сравнение кода; скрипты его отвергают.

#![allow(
    clippy::disallowed_methods,
    reason = "a build script reads the environment cargo gives it"
)]

use std::{env, path::PathBuf};

/// Настройки профиля, меняющие машинный код; символы и strip его не меняют.
const OVERRIDES: [&str; 5] = ["LTO", "CODEGEN_UNITS", "INCREMENTAL", "OPT_LEVEL", "PANIC"];

fn main() {
    // OUT_DIR = <target>/[<triple>/]<profile>/build/<package>-<hash>/out
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let profile = out
        .ancestors()
        .nth(3)
        .and_then(|dir| dir.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Каталог профиля dev называется debug; переопределения ищем по имени профиля.
    let key = if profile == "debug" {
        "DEV".to_owned()
    } else {
        profile.to_uppercase().replace('-', "_")
    };
    let mut overrides = Vec::new();
    for name in OVERRIDES {
        let variable = format!("CARGO_PROFILE_{key}_{name}");
        println!("cargo:rerun-if-env-changed={variable}");
        if let Ok(value) = env::var(&variable) {
            overrides.push(format!("{}={value}", name.to_lowercase().replace('_', "-")));
        }
    }
    println!("cargo:rustc-env=ENGINE_BENCH_BUILD_PROFILE={profile}");
    println!(
        "cargo:rustc-env=ENGINE_BENCH_BUILD_OPT_LEVEL={}",
        env::var("OPT_LEVEL").unwrap_or_default()
    );
    println!(
        "cargo:rustc-env=ENGINE_BENCH_BUILD_OVERRIDES={}",
        overrides.join(" ")
    );
    println!("cargo:rerun-if-changed=build.rs");
}
