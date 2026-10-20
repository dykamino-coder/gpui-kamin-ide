//! Anchor registries and prepaint transform stack owned by one document.

use super::AnchorRec;
use gpui::{Bounds, Pixels};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct AnchorState {
    /// Именованные якоря кадра: имя → запись. Повтор имени
    /// перезаписывает — «the last element in tree order» (§target anchor
    /// element, п.3); «ближайший предок» пока не отличается.
    pub(super) named: RefCell<HashMap<String, AnchorRec>>,
    /// Неявные якоря: `node_id` порождающего элемента → его запись (для
    /// псевдоэлементов с `position-anchor: auto`, §implicit).
    pub(super) implicit: RefCell<HashMap<u64, AnchorRec>>,
    /// Те же именованные пробы кадра С ПОРЯДКОМ СБОРКИ (`Computed::anchor_seq`):
    /// на следующем кадре станут `LAST_NAMED`, и размер по `anchor-size()`
    /// возьмёт «последний якорь с этим именем раньше меня по дереву».
    pub(super) named_seq: RefCell<Vec<(String, u32, AnchorRec)>>,
    /// Реестры ПРОШЛОГО кадра — для величин, которые нужны ДО раскладки
    /// (`anchor-size()`, растяжка в клетке `position-area`): размер решает
    /// taffy, а рамки известны только на подготовке; стенд ждёт устоявшихся
    /// кадров, и значение из кадра N−1 доезжает к N+1.
    /// Полная запись (не только рамка): прошлый кадр подменяет текущий и там,
    /// где якорь ещё не подготовлен (`AnchorPlan::rec`), — нужны `id`/`cb`
    /// для приемлемости.
    pub(super) last_named: RefCell<Vec<(String, u32, AnchorRec)>>,
    pub(super) last_implicit: RefCell<HashMap<u64, AnchorRec>>,
    /// Рамки СОДЕРЖАЩИХ БЛОКОВ кадра (padding box): `node_id` элемента с
    /// `establishes_cb` → рамка. Нужны сетке `position-area`.
    pub(super) cb: RefCell<HashMap<u64, Bounds<Pixels>>>,
    /// Размер inset-modified containing block (клетки за вычетом вставок) по
    /// ключу коробки: текущий кадр и прошлый — для `place-self: stretch`.
    pub(super) area_now: RefCell<HashMap<u64, (f32, f32)>>,
    pub(super) area_last: RefCell<HashMap<u64, (f32, f32)>>,
    /// Размер САМОЙ клетки (без вставок) — база долей размеров, полей и
    /// отбивок: текущий кадр и прошлый (`resolve_sizes`).
    pub(super) cell_now: RefCell<HashMap<u64, (f32, f32)>>,
    pub(super) cell_last: RefCell<HashMap<u64, (f32, f32)>>,
    /// Счётчик порядка сборки элементов в кадре (`next_seq`).
    pub(super) seq: Cell<u32>,
    /// Стек плоских трансформов предков на подготовке кадра: номер и
    /// аффинная `[[a, b, tx], [c, d, ty]]` в css-точках окна. Кладёт
    /// `interact::Transformed::prepaint` вокруг ребёнка; номера идут заново
    /// каждый кадр (`reset`), дерево то же — номера те же.
    pub(super) tf: RefCell<Vec<(u32, [[f32; 3]; 2])>>,
    pub(super) tf_next: Cell<u32>,
    /// Цепочка содержащих блоков: `node_id` блока → `node_id` его ближайшего
    /// содержащего блока (0 — начальный). Нужна приемлемости якоря (§target:
    /// содержащий блок коробки должен быть в цепочке содержащих блоков якоря).
    pub(super) cb_parent: RefCell<HashMap<u64, u64>>,
    /// Последний удачный вариант `position-try-fallbacks` по ключу коробки
    /// (§fallback: «the element keeps those styles until it overflows again»):
    /// 0 — базовые стили, k — k-й элемент списка. Живёт между кадрами,
    /// чистится на сборке документа (`settle_static`).
    pub(super) chosen: RefCell<HashMap<u64, usize>>,
    /// This frame's build sized a box from the PREVIOUS frame's anchor
    /// registry (`anchor-size()`, `position-area` cell percentages).
    pub(super) used_last: Cell<bool>,
    /// Extra frames requested for this document because those previous-frame
    /// values changed (bounded, so an oscillating page cannot loop forever).
    pub(super) reframes: Cell<u32>,
}

impl AnchorState {
    /// The previous frame's inset-modified containing block for one anchor box.
    pub(super) fn last_area(&self, key: u64) -> Option<(f32, f32)> {
        self.area_last.borrow().get(&key).copied()
    }
}
