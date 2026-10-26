//! Самостоятельный стенд фаз движка: одно offscreen-окно, без WPT-обвязки.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::disallowed_methods,
    reason = "CLI owns environment configuration and machine-readable process output"
)]

#[path = "engine_bench/args.rs"]
mod args;
#[cfg(windows)]
#[path = "engine_bench/host.rs"]
mod host;
#[path = "engine_bench/report.rs"]
mod report;
#[path = "engine_bench/resources.rs"]
mod resources;
#[cfg(windows)]
#[path = "engine_bench/run.rs"]
mod run;
#[path = "engine_bench/stylesheet.rs"]
mod stylesheet;
#[cfg(windows)]
#[path = "engine_bench/view.rs"]
mod view;

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
    if cfg!(feature = "wpt-harness") {
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
