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
//! только до его краски. Чтение находит владельца по `owner` и вне его
//! области (отложенные замыкания GPUI): реестр живых документов потока.

use super::DocState;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Weak;
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
pub(crate) struct Arena<T> {
    doc: RefCell<Vec<T>>,
    frame: RefCell<Vec<T>>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena {
            doc: RefCell::new(Vec::new()),
            frame: RefCell::new(Vec::new()),
        }
    }
}

impl<T> Arena<T> {
    pub(super) fn clear_frame(&self) {
        self.frame.borrow_mut().clear();
    }
}

static NEXT_OWNER: AtomicU32 = AtomicU32::new(1);

thread_local! {
    /// Живые области потока: номер → документ (`Weak`: реестр документ не
    /// держит, `Drop` его вычёркивает).
    static OWNERS: RefCell<HashMap<u32, Weak<DocState>>> = RefCell::new(HashMap::new());
}

/// Новый номер области, зарегистрированный за документом `doc`.
pub(super) fn new_owner(doc: &Weak<DocState>) -> u32 {
    let mut owner = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
    if owner == 0 {
        owner = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
    }
    OWNERS.with(|o| o.borrow_mut().insert(owner, doc.clone()));
    owner
}

/// Вычеркнуть область (документ умер или кадр сброшен).
pub(super) fn forget_owner(owner: u32) {
    let _ = OWNERS.try_with(|o| o.borrow_mut().remove(&owner));
}

/// Записать значение в арену текущего документа: на сборке кадра — в
/// область кадра, иначе — в область документа.
pub(crate) fn store<T>(pick: fn(&DocState) -> &Arena<T>, value: T) -> ArenaKey {
    let (doc, frame) = super::current_mode();
    let (owner, list) = if frame {
        (doc.frame_owner.get(), &pick(&doc).frame)
    } else {
        (doc.owner.get(), &pick(&doc).doc)
    };
    let mut list = list.borrow_mut();
    let Ok(slot) = u32::try_from(list.len()) else {
        debug_assert!(false, "арена документа переполнена");
        return ArenaKey::INVALID;
    };
    list.push(value);
    ArenaKey { owner, slot }
}

/// Прочитать запись по ручке; `None` — ручка чужая для живых документов
/// потока или устарела (сброшенный кадр, умерший документ): это ошибка
/// связи, и отладочная сборка её не прячет.
pub(crate) fn load<T: Clone>(pick: fn(&DocState) -> &Arena<T>, key: ArenaKey) -> Option<T> {
    let cur = super::current();
    let doc = if key.owner == cur.owner.get() || key.owner == cur.frame_owner.get() {
        Some(cur)
    } else {
        OWNERS.with(|o| o.borrow().get(&key.owner).and_then(Weak::upgrade))
    };
    let got = doc.and_then(|doc| {
        let arena = pick(&doc);
        let list = if key.owner == doc.owner.get() {
            &arena.doc
        } else {
            &arena.frame
        };
        list.borrow().get(key.slot as usize).cloned()
    });
    debug_assert!(
        got.is_some() || key == ArenaKey::INVALID,
        "ручка арены без записи: {key:?}"
    );
    got
}
