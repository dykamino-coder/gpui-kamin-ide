//! Вытеснение по байтам, отсутствие полного сброса и конкурентный доступ `BoundedCache`.

use super::{BoundedCache, ENTRY_OVERHEAD, FloatKey, Hinted, hint};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn len(v: &Vec<u8>) -> usize {
    v.len()
}

/// Вес записи `u32 → Vec<u8>` длины `n`.
fn weight(n: usize) -> usize {
    n + ENTRY_OVERHEAD + size_of::<u32>()
}

#[test]
fn evicts_least_recently_used_by_bytes() {
    let cache: BoundedCache<u32, Vec<u8>> = BoundedCache::new(weight(100) * 3, len);
    for k in 0..3 {
        cache.insert(k, vec![0; 100]);
    }
    assert_eq!(cache.len(), 3);
    // Обращение к 0 делает самой давней запись 1.
    assert!(cache.get(&0).is_some());
    cache.insert(3, vec![0; 100]);
    assert_eq!(cache.len(), 3);
    assert!(cache.get(&1).is_none());
    for k in [0, 2, 3] {
        assert!(cache.get(&k).is_some(), "key {k}");
    }
    assert!(cache.bytes() <= weight(100) * 3);
}

#[test]
fn heavy_entry_evicts_only_what_it_needs() {
    let cache: BoundedCache<u32, Vec<u8>> = BoundedCache::new(weight(100) * 10, len);
    for k in 0..10 {
        cache.insert(k, vec![0; 100]);
    }
    // Запись весом в три мелких вытесняет три самых давних, не весь кэш.
    cache.insert(10, vec![0; weight(100) * 3 - weight(0)]);
    assert_eq!(cache.len(), 8);
    for k in 0..3 {
        assert!(cache.get(&k).is_none(), "key {k}");
    }
    for k in 3..=10 {
        assert!(cache.get(&k).is_some(), "key {k}");
    }
    assert_eq!(cache.bytes(), weight(100) * 10);
}

#[test]
fn entry_over_budget_is_not_cached_and_evicts_nothing() {
    let cache: BoundedCache<u32, Vec<u8>> = BoundedCache::new(weight(100) * 2, len);
    cache.insert(0, vec![0; 100]);
    cache.insert(1, vec![0; weight(100) * 2]);
    assert!(cache.get(&1).is_none());
    assert!(cache.get(&0).is_some());
}

#[test]
fn replacing_a_key_keeps_the_byte_count_exact() {
    let cache: BoundedCache<u32, Vec<u8>> = BoundedCache::new(MIB_TEST, len);
    cache.insert(7, vec![0; 10]);
    cache.insert(7, vec![0; 30]);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.bytes(), weight(30));
}

const MIB_TEST: usize = super::MIB;

#[test]
fn allocated_key_bytes_are_budgeted_even_for_negative_results() {
    let cache: BoundedCache<String, Option<()>> =
        BoundedCache::new(512, |_| 0).with_key_weight(String::len);
    cache.insert("x".repeat(300), None);
    cache.insert("y".repeat(300), None);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.bytes(), 300 + ENTRY_OVERHEAD + size_of::<String>());
    cache.insert("z".repeat(600), None);
    assert_eq!(cache.len(), 1);
}

#[test]
fn borrowed_lookup_and_hinted_keys() {
    let names: BoundedCache<String, Vec<u8>> = BoundedCache::new(MIB_TEST, len);
    names.insert("a.png".to_owned(), vec![1]);
    assert_eq!(names.get("a.png"), Some(vec![1]));
    // Равные ключи с равной подсказкой находятся; неравные — нет даже при общей подсказке.
    let floats: BoundedCache<Hinted<(FloatKey, u32)>, Vec<u8>> = BoundedCache::new(MIB_TEST, len);
    let key = |x: f32| Hinted {
        hint: hint(3u32),
        key: (x.into(), 3),
    };
    floats.insert(key(0.5), vec![2]);
    assert_eq!(floats.get(&key(0.5)), Some(vec![2]));
    assert_eq!(floats.get(&key(0.25)), None);
    assert_eq!(
        floats.get(&Hinted {
            hint: hint(4u32),
            key: (0.5.into(), 3)
        }),
        None
    );
}

#[test]
fn float_keys_have_canonical_equality_and_hashes() {
    for values in [[0.0, -0.0], [f32::NAN, f32::from_bits(0xff80_0001)]] {
        let [a, b] = values.map(FloatKey::from);
        assert_eq!(a, a);
        assert_eq!(a, b);
        assert_eq!(hint(a), hint(b));
    }
    for values in [
        [1.0, 1.0f32.next_up()],
        [f32::INFINITY, f32::NEG_INFINITY],
        [f32::INFINITY, f32::NAN],
    ] {
        let [a, b] = values.map(FloatKey::from);
        assert_ne!(a, b);
    }
}

#[test]
fn nan_and_zero_entries_are_found_and_replaced() {
    let cache = BoundedCache::new(MIB_TEST, len);
    for values in [[0.0, -0.0], [f32::NAN, f32::from_bits(0x7f80_0123)]] {
        let key = |v: f32| {
            let v = FloatKey::from(v);
            Hinted {
                hint: hint(v),
                key: v,
            }
        };
        cache.insert(key(values[0]), vec![1]);
        assert_eq!(
            cache.get_or_insert_with(key(values[1]), || panic!("cache miss")),
            vec![1]
        );
        cache.insert(key(values[1]), vec![2]);
        assert_eq!(cache.get(&key(values[0])), Some(vec![2]));
    }
    assert_eq!(cache.len(), 2);
}

#[test]
fn create_runs_without_the_lock() {
    let cache: BoundedCache<u32, Vec<u8>> = BoundedCache::new(MIB_TEST, len);
    // Вложенное обращение к тому же кэшу зависло бы, если бы замок держался.
    let value = cache.get_or_insert_with(1, || {
        assert!(cache.get(&2).is_none());
        cache.insert(2, vec![2]);
        vec![1]
    });
    assert_eq!(value, vec![1]);
    assert_eq!(cache.get(&2), Some(vec![2]));
}

#[test]
fn concurrent_access_stays_within_budget_and_returns_created_values() {
    let budget = weight(64) * 16;
    let cache: Arc<BoundedCache<u32, Vec<u8>>> = Arc::new(BoundedCache::new(budget, len));
    let created = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let cache = cache.clone();
            let created = created.clone();
            std::thread::spawn(move || {
                for i in 0..2000u32 {
                    // Горячие ключи 0..4 остаются в кэше, холодные 4..40 вытесняют друг друга.
                    let key = if i % 2 == 0 {
                        (i + t) % 4
                    } else {
                        4 + (i * 7 + t) % 36
                    };
                    let value = cache.get_or_insert_with(key, || {
                        created.fetch_add(1, Ordering::Relaxed);
                        vec![key as u8; 64]
                    });
                    assert_eq!(value, vec![key as u8; 64]);
                    assert!(cache.bytes() <= budget);
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("worker");
    }
    assert!(cache.bytes() <= budget);
    assert!(cache.len() <= 16);
    // Попадания были: без кэша каждое обращение создавало бы значение заново.
    assert!(created.load(Ordering::Relaxed) < 8 * 2000 * 3 / 4);
}
