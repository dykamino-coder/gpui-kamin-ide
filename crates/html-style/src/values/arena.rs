//! Арены выражений документа (`calc()` и `anchor()`): `Len` несёт ручку,
//! тело живёт в арене документа и освобождается вместе с ним.
//!
//! Ручка `ArenaKey { owner, slot }` типизирована и проверяема: `owner` —
//! серийный номер области (документ целиком или поколение кадра) из
//! процессного счётчика, поэтому чужая или устаревшая ручка распознаётся,
//! а не читается чужим значением. Записи разбора идут в область документа,
//! записи сборки кадра (интерполяция, значения листа у хоста) — в область
//! кадра, которая сбрасывается на первой сборке после краски документа:
//! кадр GPUI — render, layout, prepaint, paint, и ручки кадра N читаются
//! только до его краски. Документ ручки приходит с контекстом значения
//! (`ValueCx`).

use super::{DocumentValues, ValueCx};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};

/// Ручка записи арены: область-владелец и номер записи в ней.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArenaKey {
    owner: u32,
    slot: u32,
}

impl ArenaKey {
    /// Ручка без записи: так выглядит переполнение арены; читается запасным
    /// значением.
    pub const INVALID: ArenaKey = ArenaKey { owner: 0, slot: 0 };
}

/// Записи одного рода: область документа и область текущего кадра.
pub struct Arena<T> {
    doc: RefCell<Vec<T>>,
    frame: RefCell<Vec<T>>,
    /// Records of the document region by the hash of their value (`store_once`).
    once: std::cell::Cell<std::collections::HashMap<u64, Vec<u32>>>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena {
            doc: RefCell::new(Vec::new()),
            frame: RefCell::new(Vec::new()),
            once: Default::default(),
        }
    }
}

impl<T> Arena<T> {
    /// Records of the document region.
    pub fn records(&self) -> usize {
        self.doc.borrow().len()
    }

    pub(super) fn clear_frame(&self) {
        self.frame.borrow_mut().clear();
    }
}

impl DocumentValues {
    /// Clears the frame region of every expression arena.
    pub(super) fn clear_frame_arenas(&self) {
        self.calc.clear_frame();
        self.anchor.clear_frame();
        self.math.clear_frame();
    }

    /// The frame region and how many records it holds: a computation that
    /// changed it left handles valid for this frame only.
    pub fn frame_arena_mark(&self) -> (u32, usize) {
        let len = self.calc.frame.borrow().len()
            + self.anchor.frame.borrow().len()
            + self.math.frame.borrow().len();
        (self.frame_owner.get(), len)
    }
}

static NEXT_OWNER: AtomicU32 = AtomicU32::new(1);

/// Новый номер области.
pub(super) fn new_owner() -> u32 {
    let mut owner = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
    if owner == 0 {
        owner = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
    }
    owner
}

/// Записать значение в арену документа контекста: на сборке кадра — в
/// область кадра, иначе — в область документа.
pub fn store<T>(vcx: ValueCx<'_>, pick: fn(&DocumentValues) -> &Arena<T>, value: T) -> ArenaKey {
    let doc = vcx.doc;
    let (owner, list) = if vcx.frame {
        (doc.frame_owner.get(), &pick(doc).frame)
    } else {
        (doc.owner.get(), &pick(doc).doc)
    };
    let mut list = list.borrow_mut();
    let Ok(slot) = u32::try_from(list.len()) else {
        debug_assert!(false, "арена документа переполнена");
        return ArenaKey::INVALID;
    };
    list.push(value);
    ArenaKey { owner, slot }
}

/// `store` of a value the document region keeps once: an equal value
/// stored before (with the same `hash`) gives its record back. A restyle in
/// place cascades its targets again (`dom::restyle_probe`), and every probe
/// would otherwise add the same bodies to the document region for as long
/// as the document lives; equal values also give equal handles, so styles
/// compare by their values. The frame region is cleared with its frame.
pub fn store_once<T: PartialEq>(
    vcx: ValueCx<'_>,
    pick: fn(&DocumentValues) -> &Arena<T>,
    value: T,
    hash: u64,
) -> ArenaKey {
    let doc = vcx.doc;
    if vcx.frame {
        return store(vcx, pick, value);
    }
    let arena = pick(doc);
    let mut once = arena.once.take();
    let slots = once.entry(hash).or_default();
    let kept = {
        let list = arena.doc.borrow();
        slots
            .iter()
            .copied()
            .find(|&s| list.get(s as usize) == Some(&value))
    };
    let key = match kept {
        Some(slot) => ArenaKey {
            owner: doc.owner.get(),
            slot,
        },
        None => {
            let key = store(vcx, pick, value);
            if key != ArenaKey::INVALID {
                slots.push(key.slot);
            }
            key
        }
    };
    arena.once.set(once);
    key
}
/// Прочитать запись по ручке в документе контекста; `None` — ручка чужая
/// или устарела (сброшенный кадр): это ошибка связи, и отладочная сборка её
/// не прячет.
pub fn load<T: Clone>(
    vcx: ValueCx<'_>,
    pick: fn(&DocumentValues) -> &Arena<T>,
    key: ArenaKey,
) -> Option<T> {
    let doc = vcx.doc;
    let arena = pick(doc);
    let list = if key.owner == doc.owner.get() {
        Some(&arena.doc)
    } else if key.owner == doc.frame_owner.get() {
        Some(&arena.frame)
    } else {
        None
    };
    let got = list.and_then(|list| list.borrow().get(key.slot as usize).cloned());
    debug_assert!(
        got.is_some() || key == ArenaKey::INVALID,
        "ручка арены без записи: {key:?}"
    );
    got
}

#[cfg(test)]
mod tests {
    use crate::style::values::value::Sum;

    #[test]
    fn an_equal_sum_stored_again_takes_its_first_record() {
        let values = crate::values::DocumentValues::detached(Default::default());
        let vcx = values.values();
        let sum = Sum {
            px: 4.0,
            pct: 0.5,
            has_percentage: true,
            ..Sum::default()
        };
        let other = Sum { px: 5.0, ..sum };
        fn pick(d: &crate::values::DocumentValues) -> &super::Arena<Sum> {
            &d.calc
        }
        let first = super::store_once(vcx, pick, sum, 1);
        assert_eq!(super::store_once(vcx, pick, sum, 1), first);
        assert_ne!(super::store_once(vcx, pick, other, 1), first);
        assert_eq!(values.calc.doc.borrow().len(), 2);
    }
}
