//! Тесты решений старта CEF при занятом профиле (`profile_gate.rs`).

use super::*;

fn ms(v: u64) -> Duration {
    Duration::from_millis(v)
}

#[test]
fn free_profile_initializes_without_waiting() {
    assert_eq!(pre_init_step(false, ms(0)), Gate::Init);
    assert_eq!(pre_init_step(false, ms(4_900)), Gate::Init);
}

#[test]
fn busy_profile_waits_with_growing_steps() {
    assert_eq!(pre_init_step(true, ms(0)), Gate::Wait(ms(100)));
    assert_eq!(pre_init_step(true, ms(600)), Gate::Wait(ms(300)));
    assert_eq!(pre_init_step(true, ms(1_600)), Gate::Wait(ms(800)));
    assert_eq!(pre_init_step(true, ms(3_000)), Gate::Wait(ms(1_000)));
}

#[test]
fn wait_never_crosses_the_budget() {
    assert_eq!(pre_init_step(true, ms(4_700)), Gate::Wait(ms(300)));
    assert_eq!(pre_init_step(true, PRE_INIT_BUDGET), Gate::Init);
    assert_eq!(pre_init_step(true, ms(60_000)), Gate::Init);
}

#[test]
fn whole_wait_is_bounded_by_the_budget() {
    let mut waited = Duration::ZERO;
    let mut steps = 0;
    while let Gate::Wait(step) = pre_init_step(true, waited) {
        assert!(step > Duration::ZERO);
        waited += step;
        steps += 1;
    }
    assert_eq!(waited, PRE_INIT_BUDGET);
    assert!(steps < 20, "polls {steps}");
}

#[test]
fn exit_codes_are_classified() {
    assert_eq!(classify(1, 0), Outcome::Live);
    assert_eq!(classify(0, 21), Outcome::ProfileBusy(21));
    assert_eq!(classify(0, 24), Outcome::ProfileBusy(24));
    assert_eq!(classify(0, 38), Outcome::Failed(38));
    assert_eq!(classify(0, 0), Outcome::Failed(0));
}

#[test]
fn busy_profile_waits_until_free_then_relaunches() {
    let busy = Outcome::ProfileBusy(24);
    assert_eq!(recovery_step(busy, false, false, 0, false), Recovery::Wait);
    assert_eq!(
        recovery_step(busy, true, false, 0, false),
        Recovery::Relaunch
    );
    assert_eq!(
        recovery_step(busy, true, false, 1, false),
        Recovery::Relaunch
    );
}

#[test]
fn auto_relaunch_is_limited_and_respects_unsaved_edits() {
    let busy = Outcome::ProfileBusy(21);
    assert_eq!(
        recovery_step(busy, true, false, MAX_AUTO_RELAUNCHES, false),
        Recovery::AskUser
    );
    assert_eq!(recovery_step(busy, true, true, 0, false), Recovery::AskUser);
    assert_eq!(recovery_step(busy, false, true, 0, true), Recovery::AskUser);
}

#[test]
fn other_failures_never_relaunch_automatically() {
    let failed = Outcome::Failed(38);
    assert_eq!(
        recovery_step(failed, true, false, 0, false),
        Recovery::AskUser
    );
    assert_eq!(
        recovery_step(failed, true, false, 0, true),
        Recovery::Relaunch
    );
}

#[test]
fn manual_restart_ignores_the_limit_and_the_lock() {
    let busy = Outcome::ProfileBusy(24);
    assert_eq!(
        recovery_step(busy, false, false, MAX_AUTO_RELAUNCHES, true),
        Recovery::Relaunch
    );
}

#[test]
fn successor_keeps_user_args_and_replaces_our_flags() {
    let args: Vec<String> = [
        "C:\\work",
        "--native-acceptance",
        "--kamin-relaunch-after=11",
        "--kamin-relaunch-count=1",
    ]
    .map(String::from)
    .to_vec();
    let next = successor_args(&args, 4242, 2);
    assert_eq!(
        next,
        [
            "C:\\work",
            "--native-acceptance",
            "--kamin-relaunch-after=4242",
            "--kamin-relaunch-count=2",
        ]
    );
    assert_eq!(flag_value(&next, AFTER_FLAG), Some(4242));
    assert_eq!(flag_value(&next, COUNT_FLAG), Some(2));
}

#[test]
fn missing_or_bad_flags_read_as_none() {
    let args: Vec<String> = ["--kamin-relaunch-count=x"].map(String::from).to_vec();
    assert_eq!(flag_value(&args, COUNT_FLAG), None);
    assert_eq!(flag_value(&args, AFTER_FLAG), None);
}
