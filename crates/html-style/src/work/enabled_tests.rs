//! Изоляция сессий и восстановление фаз наблюдения.

use super::*;
#[test]
fn boundary_observer_sees_nested_phase_restore() {
    let boundaries = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = boundaries.clone();
    let session = Session::begin_observed(Some(Arc::new(move |phase| {
        observed.lock().unwrap().push(phase as usize);
    })));
    {
        let _layout = enter(Phase::Layout);
        let _paint = enter(Phase::Paint);
    }
    session.finish();
    assert_eq!(
        *boundaries.lock().unwrap(),
        [
            Phase::Layout as usize,
            Phase::Paint as usize,
            Phase::Layout as usize,
            Phase::PipelineOther as usize,
        ]
    );
}
#[test]
fn phases_sessions_and_threads_do_not_overlap() {
    let outer = Session::begin();
    increment(Counter::RebuiltNodes, 1);
    {
        let _paint = enter(Phase::Paint);
        increment(Counter::RebuiltNodes, 2);
        let inner = Session::begin();
        increment(Counter::RebuiltNodes, 99);
        assert_eq!(
            inner.finish().0[Phase::PipelineOther as usize][Counter::RebuiltNodes as usize],
            99
        );
        std::thread::spawn(|| increment(Counter::RebuiltNodes, 100))
            .join()
            .unwrap();
        increment(Counter::RebuiltNodes, 3);
    }
    let snapshot = outer.finish();
    assert_eq!(
        snapshot.0[Phase::Paint as usize][Counter::RebuiltNodes as usize],
        5
    );
    assert_eq!(
        snapshot
            .0
            .iter()
            .map(|r| r[Counter::RebuiltNodes as usize])
            .sum::<u64>(),
        6
    );
}
#[test]
fn aborted_capture_restores_collector() {
    let outer = Session::begin();
    let _ = std::panic::catch_unwind(|| {
        let _session = Session::begin();
        let _phase = enter(Phase::Paint);
        increment(Counter::MulticolProbes, 9);
        panic!("abort sample");
    });
    increment(Counter::MulticolProbes, 1);
    assert_eq!(
        outer.finish().0[Phase::PipelineOther as usize][Counter::MulticolProbes as usize],
        1
    );
}
#[test]
fn phase_windows_exclude_the_boundary_observer() {
    let pause = std::time::Duration::from_millis(30);
    let session = Session::begin_observed(Some(Arc::new(move |_| std::thread::sleep(pause))));
    let window = crate::work::Clock::now();
    let wall = std::time::Instant::now();
    {
        let _layout = enter(Phase::Layout);
    }
    let measured = window.elapsed();
    let wall = wall.elapsed();
    session.finish();
    assert!(wall >= pause * 2, "two boundaries slept: {wall:?}");
    assert!(
        measured < pause,
        "observer time stays out of the window: {measured:?}"
    );
}
#[test]
fn phase_windows_without_an_observer_are_wall_time() {
    let session = Session::begin();
    let before = crate::work::Clock::now();
    {
        let _layout = enter(Phase::Layout);
    }
    let after = crate::work::Clock::now();
    session.finish();
    assert_eq!(before.observed, after.observed);
    assert_eq!(
        after.duration_since(before),
        after.wall.duration_since(before.wall)
    );
}
