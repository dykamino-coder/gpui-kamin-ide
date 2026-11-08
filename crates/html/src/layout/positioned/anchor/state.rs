//! Реестры якорей и стек трансформаций prepaint, принадлежащие одному документу.

use super::AnchorRec;
use gpui::{Bounds, Pixels};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

/// Hashing of node ids in the document's registries: a fixed seed, so the builder is zero-sized
/// (`RandomState` keeps 16 bytes in every map) and ids, which are not attacker-chosen, hash
/// faster.
#[derive(Clone, Copy, Default)]
pub(crate) struct NodeIdHash;

impl std::hash::BuildHasher for NodeIdHash {
    type Hasher = foldhash::fast::FoldHasher<'static>;

    fn build_hasher(&self) -> Self::Hasher {
        std::hash::BuildHasher::build_hasher(&foldhash::fast::FixedState::default())
    }
}

/// A registry keyed by node id.
pub(crate) type NodeMap<V> = HashMap<u64, V, NodeIdHash>;

#[derive(Default)]
pub(crate) struct AnchorState {
    /// Неявные якоря: `node_id` порождающего элемента → его запись (для
    /// псевдоэлементов с `position-anchor: auto`, §implicit).
    pub(super) implicit: RefCell<NodeMap<AnchorRec>>,
    /// Именованные пробы с порядком дерева для приемлемости OOF-якорей.
    pub(super) named_seq: RefCell<Vec<(String, u32, AnchorRec)>>,
    /// Рамки СОДЕРЖАЩИХ БЛОКОВ кадра (padding box): `node_id` элемента с
    /// `establishes_cb` → рамка. Нужны сетке `position-area`.
    pub(super) cb: RefCell<NodeMap<Bounds<Pixels>>>,
    /// Коробки с выбранным вариантом, присутствовавшие в текущем кадре.
    pub(super) present: RefCell<HashSet<u64, NodeIdHash>>,
    /// Внепоточные якоря обязаны предшествовать потребителю по дереву.
    pub(super) out_of_flow: RefCell<NodeMap<u32>>,
    /// Счётчик порядка сборки элементов в кадре (`next_seq`).
    pub(super) seq: Cell<u32>,
    /// Стек плоских трансформов предков на подготовке кадра: номер и
    /// аффинная `[[a, b, tx], [c, d, ty]]` в css-точках окна. Кладёт
    /// `transformed_element::Transformed::prepaint` вокруг ребёнка; номера идут заново
    /// каждый кадр (`reset`), дерево то же — номера те же.
    pub(super) tf: RefCell<Vec<(u32, [[f32; 3]; 2])>>,
    pub(super) tf_next: Cell<u32>,
    /// Цепочка содержащих блоков: `node_id` блока → `node_id` его ближайшего
    /// содержащего блока (0 — начальный). Нужна приемлемости якоря (§target:
    /// содержащий блок коробки должен быть в цепочке содержащих блоков якоря).
    pub(super) cb_parent: RefCell<NodeMap<u64>>,
    /// Последний удачный вариант `position-try-fallbacks` по ключу коробки
    /// (§fallback: «the element keeps those styles until it overflows again»):
    /// 0 — базовые стили, k — k-й элемент списка. Живёт между кадрами,
    /// удаляется после кадра без коробки; новый документ (и `update`)
    /// получает новое состояние, выбор прошлого документа его не переживает.
    pub(super) chosen: RefCell<NodeMap<super::trials::Chosen>>,
    /// Boxes whose anchor lookup found no acceptable anchor in this frame.
    pub(super) misses: RefCell<std::collections::HashSet<u64>>,
}
