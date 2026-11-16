//! The process never outlives its use: it exits after the idle time and as
//! soon as its stdin closes, which is what the OS does to the pipe when the
//! extension host ends or is killed.

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn spawn(idle_ms: u64) -> Child {
    Command::new(env!("CARGO_BIN_EXE_kamin-search"))
        .args(["--idle-ms", &idle_ms.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("kamin-search starts")
}

/// The exit status within `limit`, killing the process otherwise.
fn exits_within(child: &mut Child, limit: Duration) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Some(status) = child.try_wait().expect("the process can be waited on") {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

#[test]
fn exits_after_the_idle_time_with_stdin_still_open() {
    let mut child = spawn(300);
    let mut stdin = child.stdin.take().expect("piped stdin");
    let dir = std::env::temp_dir();
    let request = format!(
        "{{\"id\":1,\"args\":[\"--files\",\"--null\",\"--no-config\",\"--max-depth\",\"0\",\"--\",{}],\"cwd\":{}}}\n",
        serde_json::to_string(&dir.display().to_string()).expect("json"),
        serde_json::to_string(&dir.display().to_string()).expect("json"),
    );
    stdin
        .write_all(request.as_bytes())
        .expect("request written");
    let mut header = String::new();
    BufReader::new(child.stdout.take().expect("piped stdout"))
        .read_line(&mut header)
        .expect("answer read");
    assert!(header.contains("\"id\":1"), "{header}");
    let status = exits_within(&mut child, Duration::from_secs(10));
    assert!(
        status.is_some_and(|s| s.success()),
        "still running 10 s after a 300 ms idle time: {status:?}"
    );
    drop(stdin);
}

#[test]
fn exits_when_its_host_goes_away() {
    // Ten idle minutes: only the closed pipe can end it here.
    let mut child = spawn(600_000);
    drop(child.stdin.take());
    let status = exits_within(&mut child, Duration::from_secs(10));
    assert!(
        status.is_some_and(|s| s.success()),
        "still running 10 s after stdin closed: {status:?}"
    );
}
