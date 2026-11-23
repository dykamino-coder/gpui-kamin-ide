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

#[test]
fn selected_checkpoint_defers_the_default_snapshot() {
    let path =
        std::env::temp_dir().join(format!("retention-checkpoint-{}.bin", std::process::id()));
    child("checkpoint_child", Some(path.to_str().unwrap()));
    #[cfg(windows)]
    std::fs::remove_file(path.with_extension("stacks")).unwrap();
    std::fs::remove_file(path).unwrap();
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
        .env_remove(config::AT)
        .env(SLOTS, "16");
    if test == "checkpoint_child" {
        command
            .env(config::AT, "paint@42")
            .env("ENGINE_BENCH_ALLOC_STACKS", "*@0-42")
            .env("ENGINE_BENCH_ALLOC_STACKS_UNTIL", "paint")
            .env(
                "ENGINE_BENCH_ALLOC_STACKS_OUT",
                std::path::Path::new(output.unwrap()).with_extension("stacks"),
            );
    }
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
        checkpoint(42, 0);
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
fn checkpoint_child() {
    if std::env::var_os(CHILD).is_none() {
        return;
    }
    kamin_alloc::configure();
    #[cfg(windows)]
    super::super::probe::init().unwrap();
    init().unwrap();
    let state = state().unwrap();
    let (frame, phase) = state.target.unwrap();
    assert!(snapshot("first-mode").is_none());
    checkpoint(frame - 1, phase);
    assert!(!state.taken.load(Relaxed));
    #[cfg(windows)]
    {
        for _ in 0..frame {
            super::super::probe::frame();
        }
        super::super::probe::phase(phase);
        assert!(!state.taken.load(Relaxed));
        super::super::probe::outside();
    }
    #[cfg(not(windows))]
    checkpoint(frame, phase);
    assert!(state.taken.load(Relaxed));
    let file = std::fs::read(&state.path).unwrap();
    assert!(!file.is_empty());
    checkpoint(frame, phase);
    assert_eq!(std::fs::read(&state.path).unwrap(), file);
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
