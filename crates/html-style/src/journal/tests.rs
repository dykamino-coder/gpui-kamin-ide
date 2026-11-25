//! Tests of the journal: off, bounds, call sites and thread readers.

use super::*;

#[test]
fn off_means_no_records_and_no_detail_work() {
    let _one = test_reader().0;
    // No subscription in this test: nothing is computed, nothing kept.
    note("off", || unreachable!("detail of a disabled journal"));
    cache_hit("off");
    cache_miss("off", "new_key", || {
        unreachable!("detail of a disabled journal")
    });
    let j = take();
    assert!(j.causes.iter().all(|c| c.code != "off") && !j.caches.contains_key("off"));
}

#[test]
fn records_while_subscribed_with_the_callers_line_and_bounds() {
    let (_one, sub) = test_reader();
    let line = line!() + 1;
    note("code", || "detail".into());
    cache_hit("c");
    for _ in 0..EXAMPLES + 2 {
        cache_miss("c", "evicted", String::new);
    }
    let j = take();
    let mine: Vec<_> = j.causes.iter().filter(|c| c.code == "code").collect();
    assert_eq!(mine.len(), 1);
    assert!(
        mine[0]
            .site()
            .ends_with(&format!("journal/tests.rs:{line}"))
    );
    let c = &j.caches["c"];
    assert_eq!((c.hits, c.misses["evicted"]), (1, EXAMPLES as u64 + 2));
    assert_eq!(c.examples.len(), EXAMPLES);

    for _ in 0..CAUSES + 3 {
        note("many", String::new);
    }
    let j = take();
    assert_eq!(j.causes.len(), CAUSES);
    assert!(j.dropped >= 3);

    // The last reader leaving stops recording and drops the rest.
    note("left", String::new);
    drop(sub);
    assert!(!enabled());
    assert!(take().causes.is_empty());
}

#[test]
fn a_thread_reader_keeps_the_notes_of_its_own_thread_only() {
    let _one = test_reader().0;
    drop(take());
    let here = subscribe_thread();
    note("mine", String::new);
    std::thread::spawn(|| note("beside", String::new))
        .join()
        .unwrap();
    let codes: Vec<_> = take().causes.iter().map(|c| c.code).collect();
    assert!(
        codes.contains(&"mine") && !codes.contains(&"beside"),
        "{codes:?}"
    );
    // A process-wide reader takes every thread's notes again.
    let all = subscribe();
    std::thread::spawn(|| note("beside", String::new))
        .join()
        .unwrap();
    assert!(take().causes.iter().any(|c| c.code == "beside"));
    drop((all, here));
}
