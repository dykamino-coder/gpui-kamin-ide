//! Файловая матрица writer: выполняется и отдельно rustc --test на Windows.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "kamin-diag-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn log(&self, limit: u64) -> Log {
        Log::new(self.0.join("diag.log"), limit)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn long_run_rotates_whole_utf8_records_and_bounds_all_generations() {
    let fixture = Fixture::new();
    let log = fixture.log(32);
    for i in 0..100 {
        log.append(&format!("{i:03} кадр")).unwrap();
        let mut total = 0;
        for path in [&log.path, &log.backup(1), &log.backup(2)] {
            let bytes = fs::read(path).unwrap_or_default();
            assert!(bytes.len() <= 32);
            assert!(bytes.is_empty() || bytes.ends_with(b"\n"));
            let text = std::str::from_utf8(&bytes).unwrap();
            assert!(text.lines().all(|line| line.ends_with(" кадр")));
            total += bytes.len();
        }
        assert!(total <= 96);
    }
    assert!(fs::read_to_string(&log.path).unwrap().contains("099 кадр"));
}

#[test]
fn exact_boundary_rotates_on_next_record_and_restart_keeps_history() {
    let fixture = Fixture::new();
    let log = fixture.log(8);
    log.append("1234567").unwrap();
    assert!(!log.backup(1).exists());
    let restarted = fixture.log(8);
    restarted.append("boot").unwrap();
    assert_eq!(fs::read_to_string(log.backup(1)).unwrap(), "1234567\n");
    assert_eq!(fs::read_to_string(&log.path).unwrap(), "boot\n");
}

#[test]
fn restart_preserves_recent_complete_legacy_records_in_bounded_memory() {
    let fixture = Fixture::new();
    let log = fixture.log(32);
    fs::write(
        &log.path,
        format!("{}recent кадр\npartial", "old строка\n".repeat(100)),
    )
    .unwrap();
    log.append("boot").unwrap();
    let kept = fs::read_to_string(&log.path).unwrap();
    assert!(kept.contains("recent кадр\n"));
    assert!(!kept.contains("partial"));
    assert!(kept.ends_with("boot\n"));
    assert!(kept.len() <= 32);
}

#[test]
fn oversized_or_multiline_record_does_not_modify_history() {
    let fixture = Fixture::new();
    let log = fixture.log(8);
    log.append("ok").unwrap();
    for record in ["12345678", "one\ntwo", "one\rtwo"] {
        assert_eq!(
            log.append(record).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    assert_eq!(fs::read_to_string(&log.path).unwrap(), "ok\n");
}

#[test]
fn failed_rotation_does_not_append_past_limit_and_can_retry() {
    let fixture = Fixture::new();
    let log = fixture.log(8);
    log.append("1234567").unwrap();
    fs::create_dir(log.backup(2)).unwrap();
    assert!(log.append("next").is_err());
    assert_eq!(fs::read_to_string(&log.path).unwrap(), "1234567\n");
    fs::remove_dir(log.backup(2)).unwrap();
    log.append("next").unwrap();
    assert_eq!(fs::read_to_string(log.backup(1)).unwrap(), "1234567\n");
}

#[test]
fn failed_open_is_retryable() {
    let fixture = Fixture::new();
    let log = Log::new(fixture.0.join("missing/diag.log"), 32);
    assert!(log.append("first").is_err());
    fs::create_dir(fixture.0.join("missing")).unwrap();
    log.append("retry").unwrap();
    assert_eq!(fs::read_to_string(log.path).unwrap(), "retry\n");
}

#[test]
fn production_handle_can_roll_back_a_partially_written_record() {
    let fixture = Fixture::new();
    let log = fixture.log(32);
    log.append("complete").unwrap();
    let (mut file, before) = log.open_append().unwrap();
    file.write_all("torn кад".as_bytes()).unwrap();
    file.set_len(before).unwrap();
    drop(file);
    log.append("retry").unwrap();
    assert_eq!(fs::read_to_string(&log.path).unwrap(), "complete\nretry\n");
}

#[test]
fn production_limit_is_enforced_without_recreating_writer() {
    let fixture = Fixture::new();
    let limit = 5 * 1024 * 1024;
    let log = fixture.log(limit);
    let line = "x".repeat(32 * 1024 - 1);
    for _ in 0..700 {
        log.append(&line).unwrap();
    }
    let total: u64 = [&log.path, &log.backup(1), &log.backup(2)]
        .iter()
        .map(|path| length(path).unwrap())
        .sum();
    assert!(total <= 3 * limit);
    assert_eq!(length(&log.backup(1)).unwrap(), limit);
    assert_eq!(length(&log.backup(2)).unwrap(), limit);
}

#[cfg(windows)]
#[test]
fn windows_sharing_violation_keeps_history_and_retries_after_unlock() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let log = fixture.log(8);
    log.append("1234567").unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&log.path)
        .unwrap();
    assert!(log.append("next").is_err());
    drop(locked);
    assert_eq!(fs::read_to_string(&log.path).unwrap(), "1234567\n");
    log.append("next").unwrap();
    assert_eq!(fs::read_to_string(log.backup(1)).unwrap(), "1234567\n");
}
