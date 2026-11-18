//! Fresh-process checks avoid mutating the test runner's shared environment.

use super::*;
use std::process::Command;

const CHILD: &str = "ENGINE_BENCH_RETAINERS_TEST_CHILD";

#[test]
fn disabled_startup_and_hooks_do_not_allocate() {
    child("disabled_child", None);
}

#[test]
fn enabled_tracker_records_and_releases_blocks() {
    child("enabled_child", Some("retention-test-output"));
}

#[test]
fn empty_output_value_still_enables_tracking() {
    child("enabled_child", Some(""));
}

fn child(test: &str, output: Option<&str>) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &format!("allocations::retain::tests::{test}"),
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env_remove(OUT)
        .env(SLOTS, "16");
    if let Some(output) = output {
        command.env(OUT, output);
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "child failed: {}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn disabled_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    assert!(std::env::var_os(OUT).is_none());
    kamin_alloc::configure();
    let live = super::super::thread_live();
    let calls = super::super::thread_calls();
    for _ in 0..10 {
        init().unwrap();
        track(std::ptr::dangling(), 16);
        untrack(std::ptr::dangling());
        assert!(snapshot("disabled").is_none());
    }
    assert_eq!(
        super::super::thread_calls(),
        calls,
        "disabled tracker allocated"
    );
    assert_eq!(super::super::thread_live(), live);
    assert!(!enabled());
    assert!(
        STATE.load(Acquire).is_null(),
        "disabled tracker created state"
    );
}

#[test]
fn enabled_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    kamin_alloc::configure();
    init().unwrap();
    let state = state().expect("opt-in creates tracker state");
    assert_eq!(state.table.capacity(), 1 << 16);
    let block = std::hint::black_box(vec![7u8; 256]);
    let pointer = block.as_ptr() as usize;
    let mut found = false;
    state.table.each(|address, bytes| {
        if address == pointer {
            found = bytes == 256;
        }
    });
    assert!(found, "enabled shim missed a live block");
    drop(block);
    let mut retained = false;
    state.table.each(|address, _| {
        retained |= address == pointer;
    });
    assert!(!retained, "freed block remained live");
    assert!(!state.table.overflowed());
}
