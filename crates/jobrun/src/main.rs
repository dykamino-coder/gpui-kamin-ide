//! `kamin-jobrun <program> [arguments]` runs one command inside its own
//! Windows Job Object and exits with the command's exit code.
//!
//! The Bridge extension host starts a foreground tool command through this
//! runner and cancels it by terminating the runner. The runner holds the only
//! handle of the job, so its death closes the job and Windows ends every
//! process in it. That includes MSYS children whose Windows parent has already
//! exited, which a walk over parent PIDs (`taskkill /T`) cannot find.
//!
//! When the command finishes by itself, the runner first removes the
//! kill-on-close limit: a process the command left running on purpose keeps
//! running, as it did before.
//!
//! `kamin-jobrun --probe` only sets the job up and exits 0, so that the caller
//! can check once that jobs work here before it relies on them.

#![cfg_attr(
    not(windows),
    allow(dead_code, reason = "the runner exists for Windows only")
)]

#[cfg(windows)]
mod job;
mod tail;

/// The job could not be set up; the command was not started.
const EXIT_JOB_UNAVAILABLE: i32 = 0xE0;
/// The command line names no program.
const EXIT_USAGE: i32 = 0xE1;
/// The program could not be started.
const EXIT_NOT_STARTED: i32 = 127;

fn main() {
    std::process::exit(run());
}

#[cfg(windows)]
fn run() -> i32 {
    use std::ffi::OsString;
    use std::io::Write as _;
    use std::os::windows::ffi::OsStringExt as _;
    use std::os::windows::process::CommandExt as _;
    use windows::Win32::System::Environment::GetCommandLineW;

    // SAFETY: the pointer is the process command line; it is valid and
    // NUL-terminated for the whole life of the process.
    let raw = unsafe { GetCommandLineW() };
    // SAFETY: as above; the slice ends at the terminating NUL.
    let line = unsafe { raw.as_wide() };
    let Some((program, arguments)) = tail::command(line) else {
        return EXIT_USAGE;
    };
    let job = match job::Job::enter() {
        Ok(job) => job,
        Err(err) => {
            let _ = writeln!(
                std::io::stderr(),
                "kamin-jobrun: job object unavailable: {err}"
            );
            return EXIT_JOB_UNAVAILABLE;
        }
    };
    let program = OsString::from_wide(&program);
    if program == "--probe" && arguments.is_empty() {
        return 0;
    }
    // The arguments are passed on exactly as they were received: no parsing
    // and no second round of quoting between the caller and the command.
    let status = std::process::Command::new(&program)
        .raw_arg(OsString::from_wide(arguments))
        .status();
    match status {
        Ok(status) => {
            job.keep_survivors();
            status.code().unwrap_or(1)
        }
        Err(err) => {
            let _ = writeln!(
                std::io::stderr(),
                "kamin-jobrun: cannot start {}: {err}",
                program.to_string_lossy()
            );
            EXIT_NOT_STARTED
        }
    }
}

#[cfg(not(windows))]
fn run() -> i32 {
    EXIT_USAGE
}
