//! The runner against real process trees (INC-2026-0096): stopping the runner
//! must end everything the command started, and a command that finishes by
//! itself must keep what it left running.

#![cfg(windows)]

use std::io::Write as _;
use std::os::windows::process::CommandExt as _;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
};

const RUNNER: &str = env!("CARGO_BIN_EXE_kamin-jobrun");
const GIT_BASH: &str = r"C:\Program Files\Git\bin\bash.exe";
/// The bound the Bridge promises for a stopped command.
const STOP_BOUND: Duration = Duration::from_secs(1);

/// A number that appears on the command line of this test's processes only.
fn marker(test: u32) -> u32 {
    300_000 + (std::process::id() % 50_000) * 10 + test
}

fn runner(program: &str, raw_arguments: &str) -> Child {
    Command::new(RUNNER)
        .arg(program)
        .raw_arg(raw_arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start the runner")
}

/// PIDs of the processes with this image name whose command line has the marker.
fn find(image: &str, marker: u32) -> Vec<u32> {
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"Name='{image}'\" | \
         Where-Object {{ $_.CommandLine -like '*{marker}*' }} | \
         ForEach-Object {{ $_.ProcessId }}"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .expect("list processes");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .filter_map(|pid| pid.parse().ok())
        .collect()
}

fn wait_for(image: &str, marker: u32, count: usize) -> Vec<u32> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let found = find(image, marker);
        if found.len() >= count {
            return found;
        }
        assert!(
            Instant::now() < deadline,
            "{image} with {marker}: {found:?}, expected {count}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A process kept open so that its end can be awaited without polling.
struct Watched(HANDLE);

impl Watched {
    fn open(pid: u32) -> Self {
        // SAFETY: plain Win32 call; the handle is closed in `Drop`.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, false, pid) };
        Self(handle.expect("open the process"))
    }

    fn ended_within(&self, limit: Duration) -> bool {
        let millis = u32::try_from(limit.as_millis()).unwrap_or(u32::MAX);
        // SAFETY: the handle is open and has SYNCHRONIZE access.
        unsafe { WaitForSingleObject(self.0, millis) == WAIT_OBJECT_0 }
    }

    fn end(&self) {
        // SAFETY: the handle is open and has TERMINATE access.
        let _ = unsafe { TerminateProcess(self.0, 1) };
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        // A test that failed half-way must not leave its processes behind.
        self.end();
        // SAFETY: the handle is open and is not used after this.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn stop_and_expect_all_gone(mut runner: Child, watched: &[Watched]) {
    let started = Instant::now();
    runner.kill().expect("stop the runner");
    for process in watched {
        let left = STOP_BOUND.saturating_sub(started.elapsed());
        assert!(
            process.ended_within(left),
            "a process outlived the runner by {STOP_BOUND:?}"
        );
    }
    let _ = runner.wait();
}

#[test]
fn stopping_the_runner_ends_a_child_and_an_orphan() {
    let marker = marker(1);
    // The inner `cmd` starts a ping and exits at once: that ping has no live
    // parent. The outer `cmd` then waits on a second ping.
    let runner = runner(
        "cmd",
        &format!(
            "/d /c \"cmd /d /c start /b ping -n 600 -w {marker} 127.0.0.1 >nul \
             & ping -n 600 -w {marker} 127.0.0.1 >nul\""
        ),
    );
    let watched: Vec<_> = wait_for("PING.EXE", marker, 2)
        .into_iter()
        .map(Watched::open)
        .collect();
    stop_and_expect_all_gone(runner, &watched);
}

#[test]
fn stopping_the_runner_ends_msys_children() {
    if !std::path::Path::new(GIT_BASH).exists() {
        let _ = writeln!(std::io::stderr(), "skipped: {GIT_BASH} is not installed");
        return;
    }
    let marker = marker(2);
    // MSYS starts programs through an intermediate process that exits, so a
    // background `sleep` is not reachable from the shell by parent PID. The
    // shell itself stays in `wait`, so the command is still running.
    let runner = runner(
        GIT_BASH,
        &format!("-c \"sleep {marker} & sleep {marker} & sleep {marker} & wait\""),
    );
    let watched: Vec<_> = wait_for("sleep.exe", marker, 3)
        .into_iter()
        .map(Watched::open)
        .collect();
    stop_and_expect_all_gone(runner, &watched);
}

#[test]
fn a_finished_command_keeps_its_exit_code_and_what_it_left_running() {
    let marker = marker(3);
    let mut runner = runner(
        "cmd",
        &format!("/d /c \"start /b ping -n 600 -w {marker} 127.0.0.1 >nul & exit 7\""),
    );
    let status = runner.wait().expect("wait for the runner");
    assert_eq!(status.code(), Some(7));
    let left: Vec<_> = wait_for("PING.EXE", marker, 1)
        .into_iter()
        .map(Watched::open)
        .collect();
    assert!(
        !left[0].ended_within(Duration::from_millis(500)),
        "the left process was ended"
    );
}

#[test]
fn reports_setup_and_start_results_by_exit_code() {
    let code = |arguments: &[&str]| {
        Command::new(RUNNER)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("run the runner")
            .code()
    };
    assert_eq!(code(&["--probe"]), Some(0));
    assert_eq!(code(&[]), Some(0xE1));
    assert_eq!(code(&["kamin-jobrun-no-such-program"]), Some(127));
}
