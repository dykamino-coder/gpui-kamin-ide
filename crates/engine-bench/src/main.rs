//! Самостоятельный стенд фаз движка: одно offscreen-окно, без WPT-обвязки.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::disallowed_methods,
    reason = "CLI owns environment configuration and machine-readable process output"
)]

#[cfg(windows)]
mod accounting;
mod args;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod pixels;
mod report;
mod resources;
#[cfg(windows)]
mod run;
mod stylesheet;
#[cfg(windows)]
mod view;

// Тот же аллокатор, что у обычного приложения, без тестовой платформы GPUI.
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    if let Err(error) = entry() {
        eprintln!("engine_bench: {error}");
        std::process::exit(1);
    }
}

fn entry() -> Result<(), Box<dyn std::error::Error>> {
    let Some(args) = args::Args::parse(std::env::args().skip(1))? else {
        println!("{}", args::USAGE);
        return Ok(());
    };
    if cfg!(debug_assertions) {
        return Err("measurement requires --profile release (debug assertions are enabled)".into());
    }
    if kamin_html::BuildTimings::WPT_HARNESS_ENABLED {
        return Err("measure with engine-bench only; disable wpt-harness".into());
    }
    #[cfg(windows)]
    return host::launch(args);
    #[cfg(not(windows))]
    {
        let _ = args;
        Err("this offscreen GPU/readback host currently requires Windows".into())
    }
}
