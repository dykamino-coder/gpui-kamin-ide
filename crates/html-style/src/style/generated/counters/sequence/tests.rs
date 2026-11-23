//! Persistent sequences are checked against a mutable oracle and retained revisions.
use super::*;
use std::cell::Cell;

fn check<T: Clone>(root: &Link<T>) -> (usize, u8) {
    let Some(n) = root else {
        return (0, 0);
    };
    let (a, ah) = check(&n.left);
    let (b, bh) = check(&n.right);
    assert!(ah.abs_diff(bh) <= 1);
    assert_eq!(n.len, a + b + 1);
    assert_eq!(n.height, ah.max(bh) + 1);
    (n.len, n.height)
}
#[test]
fn mutations_preserve_snapshots_and_balance() {
    let mut seq = Sequence::default();
    let mut plain = Vec::new();
    let mut kept = Vec::new();
    let mut seed = 7u64;
    for step in 0..10000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let at = (seed >> 32) as usize;
        match (step % 5, plain.len()) {
            (_, 0) | (0 | 1, _) => {
                let i = at % (plain.len() + 1);
                plain.insert(i, step);
                seq.insert(i, step);
            }
            (2, n) => {
                let i = at % n;
                plain[i] = step;
                seq.set(i, step);
            }
            (3, n) => {
                let i = at % n;
                assert_eq!(plain.remove(i), seq.remove(i));
            }
            (_, n) => {
                let i = at % (n + 1);
                plain.truncate(i);
                seq.truncate(i);
            }
        }
        assert_eq!(seq.iter().copied().collect::<Vec<_>>(), plain);
        check(&seq.0);
        if step % 71 == 0 {
            kept.push((seq.clone(), plain.clone()));
        }
    }
    for (old, plain) in kept {
        assert_eq!(old.iter().copied().collect::<Vec<_>>(), plain);
        check(&old.0);
    }
}
struct Counted {
    id: usize,
    live: Rc<Cell<usize>>,
    copies: Rc<Cell<usize>>,
}
impl Counted {
    fn new(id: usize, live: &Rc<Cell<usize>>, copies: &Rc<Cell<usize>>) -> Self {
        live.set(live.get() + 1);
        Self {
            id,
            live: live.clone(),
            copies: copies.clone(),
        }
    }
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.copies.set(self.copies.get() + 1);
        Self::new(self.id, &self.live, &self.copies)
    }
}
impl Drop for Counted {
    fn drop(&mut self) {
        self.live.set(self.live.get() - 1);
    }
}
#[test]
fn snapshots_share_prefixes_and_repeated_updates_release_old_paths() {
    let live = Rc::new(Cell::new(0));
    let copies = Rc::new(Cell::new(0));
    let mut seq = Sequence::default();
    for id in 0..4096 {
        seq.push(Counted::new(id, &live, &copies));
    }
    assert_eq!(copies.get(), 0, "unique paths mutate in place");
    let old = seq.clone();
    assert_eq!(copies.get(), 0, "snapshot copies no payloads");
    for n in 0..10000 {
        seq.set(n % 4096, Counted::new(n, &live, &copies));
        assert!(live.get() <= 8192, "only the old and current trees remain");
    }
    assert_eq!(old.get(100).unwrap().id, 100);
    assert!(
        copies.get() <= 8192,
        "no prefix or whole-context clone on each update"
    );
    drop(old);
    assert_eq!(live.get(), 4096);
    drop(seq);
    assert_eq!(live.get(), 0);
}

#[test]
fn removing_shared_deep_paths_keeps_the_original_order_and_metadata() {
    let mut plain: Vec<_> = (0..1024).collect();
    let mut seq = Sequence::default();
    for &n in &plain {
        seq.push(n);
    }
    let old = seq.clone();
    let mut seed = 31u64;
    while !plain.is_empty() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let at = (seed >> 32) as usize % plain.len();
        assert_eq!(seq.remove(at), plain.remove(at));
        check(&seq.0);
        assert_eq!(seq.iter().copied().collect::<Vec<_>>(), plain);
    }
    assert_eq!(
        old.iter().copied().collect::<Vec<_>>(),
        (0..1024).collect::<Vec<_>>()
    );
    check(&old.0);
}
