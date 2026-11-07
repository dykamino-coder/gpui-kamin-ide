//! Проверки политики арен: индекс опции совпадает с собранным mimalloc,
//! `configure` ставит значение, окружение остаётся сильнее; `reclaim`
//! декоммитит освобождённое сразу, а не после задержки очистки.

use super::{ARENA_RESERVE, ARENA_RESERVE_KIB, arena_reserve_kib, configure};
use std::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn mi_options_print_out(
        out: unsafe extern "C" fn(msg: *const c_char, arg: *mut c_void),
        arg: *mut c_void,
    );
}

unsafe extern "C" fn collect(msg: *const c_char, arg: *mut c_void) {
    // SAFETY: `arg` — `&mut String` из `printed_options`, `msg` — C-строка mimalloc.
    let out = unsafe { &mut *arg.cast::<String>() };
    out.push_str(&unsafe { CStr::from_ptr(msg) }.to_string_lossy());
}

fn printed_options() -> Vec<String> {
    let mut text = String::new();
    // SAFETY: обратный вызов пишет только в `text`, живущий дольше вызова.
    unsafe { mi_options_print_out(collect, (&raw mut text).cast()) };
    text.lines()
        .filter_map(|line| line.strip_prefix("option '"))
        .filter_map(|rest| rest.split('\'').next())
        .map(str::to_owned)
        .collect()
}

unsafe extern "C" {
    fn mi_malloc(size: usize) -> *mut c_void;
    fn mi_free(p: *mut c_void);
    fn mi_process_info(
        elapsed_msecs: *mut usize,
        user_msecs: *mut usize,
        system_msecs: *mut usize,
        current_rss: *mut usize,
        peak_rss: *mut usize,
        current_commit: *mut usize,
        peak_commit: *mut usize,
        page_faults: *mut usize,
    );
}

fn committed() -> usize {
    let mut v = [0usize; 8];
    let [a, b, c, d, e, f, g, h] = &mut v;
    // SAFETY: восемь различных выходных ячеек живут дольше вызова.
    unsafe { mi_process_info(a, b, c, d, e, f, g, h) };
    v[5]
}

#[test]
fn reclaim_decommits_freed_memory_before_the_purge_delay() {
    const SIZE: usize = 48 << 20;
    // SAFETY: блок из mimalloc, записан в пределах размера и освобождён им же.
    unsafe {
        let p = mi_malloc(SIZE).cast::<u8>();
        assert!(!p.is_null());
        std::ptr::write_bytes(p, 1, SIZE);
        mi_free(p.cast());
    }
    let freed = committed();
    super::reclaim();
    let reclaimed = committed();
    assert!(
        reclaimed + SIZE / 2 <= freed,
        "commit {freed} -> {reclaimed} after reclaim"
    );
}

#[test]
fn arena_reserve_index_matches_linked_mimalloc() {
    let names = printed_options();
    assert_eq!(
        names.get(ARENA_RESERVE as usize).map(String::as_str),
        Some("arena_reserve"),
        "{names:?}"
    );
}

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "the test checks that the allocator's own environment override still wins"
)]
fn configure_sets_arena_reserve_unless_environment_overrides() {
    configure();
    if std::env::var_os("MIMALLOC_ARENA_RESERVE").is_none() {
        assert_eq!(arena_reserve_kib(), i64::from(ARENA_RESERVE_KIB));
    }
}
