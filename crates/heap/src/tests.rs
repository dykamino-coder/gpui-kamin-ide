//! Отчёт равен живым запрошенным байтам аллокатора: тестовый аллокатор считает их.

use super::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering::SeqCst};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

struct Live;
static LIVE: AtomicIsize = AtomicIsize::new(0);
thread_local! {
    static OWN: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
}

// Каждый поток копит свой счёт: тесты идут параллельно.
unsafe impl GlobalAlloc for Live {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = OWN.try_with(|own| own.set(own.get() + layout.size() as isize));
        LIVE.fetch_add(layout.size() as isize, SeqCst);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let _ = OWN.try_with(|own| own.set(own.get() - layout.size() as isize));
        LIVE.fetch_sub(layout.size() as isize, SeqCst);
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static GLOBAL: Live = Live;

fn own() -> isize {
    OWN.with(|own| own.get())
}

/// Живые байты, выделенные при построении значения, против его отчёта.
fn check<T: HeapSize>(make: impl FnOnce() -> T) {
    let before = own();
    let value = make();
    let live = own() - before;
    let mut ops = Ops::default();
    assert_eq!(value.heap_size(&mut ops) as isize, live);
    drop(value);
}

#[test]
fn containers_report_their_requested_bytes() {
    check(|| {
        let mut words = Vec::with_capacity(7);
        words.push(String::from("alpha"));
        words.push(String::with_capacity(40));
        words
    });
    check(|| Box::new([1u64, 2, 3]));
    check(|| Box::<str>::from("boxed text"));
    check(|| Some(vec![Box::new(5u32); 3]));
}

#[test]
fn shared_blocks_count_once_with_their_counter_header() {
    check(|| {
        let shared = Rc::new(String::from("shared"));
        vec![shared.clone(), shared]
    });
    check(|| Rc::<str>::from("text"));
    check(|| Arc::new([0u8; 3]));
    check(|| Rc::new(7u128));
}

#[test]
fn hash_tables_match_hashbrown_layout() {
    for count in [1usize, 3, 4, 7, 8, 15, 100, 1000] {
        check(|| {
            (0..count as u64)
                .map(|key| (key, key))
                .collect::<HashMap<u64, u64>>()
        });
        check(|| (0..count as u32).collect::<HashSet<u32>>());
    }
}

#[test]
fn the_sink_receives_every_measured_block_with_its_path() {
    let values = vec![String::from("a"), String::from("bb")];
    let mut blocks = Vec::new();
    let mut sink = |path: &'static str, pointer: *const u8, bytes: usize| {
        blocks.push((path, pointer as usize, bytes))
    };
    let mut ops = Ops::with_sink(&mut sink);
    let total = ops.at("page/dom", |ops| values.heap_size(ops));
    drop(ops);
    assert_eq!(
        blocks.iter().map(|(_, _, bytes)| bytes).sum::<usize>(),
        total
    );
    assert_eq!(
        blocks[0],
        (
            "page/dom",
            values.as_ptr() as usize,
            2 * size_of::<String>()
        )
    );
    assert_eq!(blocks[1], ("page/dom", values[0].as_ptr() as usize, 1));
}

#[test]
fn rc_block_address_is_the_allocation_start() {
    let shared = Rc::new(5u64);
    let mut blocks = Vec::new();
    let mut sink =
        |_: &'static str, pointer: *const u8, bytes: usize| blocks.push((pointer as usize, bytes));
    let mut ops = Ops::with_sink(&mut sink);
    shared.heap_size(&mut ops);
    drop(ops);
    let value = Rc::as_ptr(&shared) as usize;
    let expected = (value - 2 * size_of::<usize>(), 3 * size_of::<usize>());
    assert_eq!(blocks, vec![expected]);
    let mut ops = Ops::default();
    assert_eq!(ops.shallow(Rc::as_ptr(&shared)), expected.1);
    assert_eq!(
        ops.shallow(Rc::as_ptr(&shared)),
        0,
        "a shared block counts once"
    );
    let _ = LIVE.load(SeqCst);
}

#[test]
fn paths_split_one_walk_and_reports_merge() {
    let mut ops = Ops::default();
    let text = String::with_capacity(10);
    let list: Vec<u32> = Vec::with_capacity(2);
    ops.at("page/dom", |ops| {
        text.heap_size(ops);
        ops.at("page/style", |ops| list.heap_size(ops));
        ops.block(std::ptr::null(), 5)
    });
    let mut report = ops.report();
    assert_eq!(report.entries()["page/dom"], 15);
    assert_eq!(report.entries()["page/style"], 8);
    report.merge(&ops.report());
    report.add("gpui", 1);
    assert_eq!(report.total(), 47);
}
