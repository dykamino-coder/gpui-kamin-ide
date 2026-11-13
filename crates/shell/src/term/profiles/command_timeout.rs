//! Bound WSL detection, drain stdout concurrently, and reap timed-out children.
use std::io::{self, Read};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub(super) fn output(command: &mut Command, timeout: Duration) -> io::Result<Vec<u8>> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    collect(&mut child, timeout)
}

fn collect(child: &mut Child, timeout: Duration) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let (send, receive) = std::sync::mpsc::channel();
    let reader = std::thread::Builder::new()
        .name("kamin-wsl-output".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout.read_to_end(&mut bytes).map(|_| bytes);
            let _ = send.send(result);
        });
    if let Err(error) = reader {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(status)) => {
                return Err(io::Error::other(format!(
                    "profile command exited with {status}"
                )));
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(result.err().unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "profile command timed out")
                }));
            }
        }
    }
    // A descendant inheriting stdout must not extend the detection deadline.
    receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "profile command stdout timed out"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "subprocess fixture, invoked only by the timeout test"]
    fn sleeping_child() {
        println!("ready");
        std::thread::sleep(Duration::from_secs(30));
    }

    #[test]
    fn timeout_kills_and_reaps_the_child() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "term::profiles::command_timeout::tests::sleeping_child",
                "--ignored",
                "--nocapture",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        let result = collect(&mut child, Duration::from_millis(200));
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert!(
            child.try_wait().unwrap().is_some(),
            "timed-out child is still running"
        );
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn successful_output_is_collected() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--list"]);
        let stdout = output(&mut command, Duration::from_secs(5)).unwrap();
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("timeout_kills_and_reaps_the_child")
        );
    }
}
