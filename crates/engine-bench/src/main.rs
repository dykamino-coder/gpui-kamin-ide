//! Самостоятельный стенд фаз движка: одно offscreen-окно, без WPT-обвязки.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::disallowed_methods,
    reason = "CLI owns environment configuration and machine-readable process output"
)]

#[cfg(windows)]
mod accounting;
mod allocations;
mod args;
mod diagnostics;
mod edit;
#[cfg(windows)]
mod edits;
#[cfg(windows)]
mod frames;
#[cfg(windows)]
mod gpu;
#[cfg(windows)]
mod host;
#[cfg(windows)]
mod memory;
#[cfg(windows)]
mod pixels;
#[cfg(windows)]
mod process;
mod report;
mod resources;
#[cfg(windows)]
mod run;
mod stylesheet;
#[cfg(all(windows, feature = "work-counters"))]
mod usage;
#[cfg(windows)]
mod view;
#[cfg(windows)]
mod work;

// Тот же аллокатор, что у обычного приложения, без тестовой платформы GPUI.
#[global_allocator]
#[cfg(feature = "allocation-audit")]
static ALLOC: allocations::Counted = allocations::Counted;
#[global_allocator]
#[cfg(not(feature = "allocation-audit"))]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    diagnostics::init();
    if let Err(error) = entry() {
        eprintln!("engine_bench: {error}");
        std::process::exit(1);
    }
}

fn entry() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    if std::env::args().skip(1).eq(["--list-gpus"]) {
        return gpu::catalog();
    }
    let Some(args) = args::Args::parse(std::env::args().skip(1))? else {
        println!("{}", args::USAGE);
        return Ok(());
    };
    if args.trace_frames {
        diagnostics::trace_frames();
    }
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
