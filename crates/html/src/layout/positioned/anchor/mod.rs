//! ★ ЗАМЕРЕНО И ОТКАЧЕНО (09.09, v168, `scout-anchorpos-2026-09.md`
//! MC-OOF-COPIES + ANCHOR-FRAGMENT-UNION, 4 хунка): `drop_escaping_oof`
//! вычёркивает из копий фрагмента не только `fixed`, но и абсолют, чей
//! содержащий блок вне многоколоночника; `union_rect` объединяет рамки
//! фрагментов якоря. Обещание +2…+4. Замер срезом 113 пар вместе с CLAMP-BFC:
//! −5 на фрагментации флекса и сетки. Возвращать вместе с FRAG-OOF.
//! Якорное позиционирование (css-anchor-position-1), шаг 1: `anchor-name`,
//! `position-anchor` и функция `anchor()` во вставках абсолютной коробки.
//!
//! Порядок вычислений. Раскладка под нами считает рамки ВСЕХ коробок до
//! подготовки кадра, а `prepaint` идёт в порядке дерева; внепоточные коробки
//! при этом дописываются ПОСЛЕДНИМИ детьми своего слоя
//! (`interact::late_close`/`cb_close`/`icb_close`). Поэтому:
//! * якорь снимает свою рамку пробой-канвасом (`probe_for`, по образцу
//!   `interact::spot_probe`/`edge_probe`) в своём `prepaint` и кладёт её в
//!   реестр кадра;
//! * позиционированная коробка, идущая позже, читает реестр в СВОЁМ
//!   `prepaint` и сдвигается через `with_element_offset` — тем же приёмом,
//!   что `interact::LatePlace`.
//!
//! Якорь, которого в реестре ещё нет (он ниже по дереву или позже в том же
//! слое), по спеке и не acceptable («laid out strictly before»): функция
//! падает на запасное значение.
//!
//! Раскладке `anchor()`-вставка отдаётся НУЛЁМ (`apply::len_to_gpui`):
//! коробка встаёт к краю содержащего блока, а сдвиг до якоря дорисовывает
//! `AnchorPlace`. Тот же ноль делает запасное значение тривиальным:
//! `anchor(left, 20px)` в `right` — это сдвиг на −20 точек от нулевой вставки.
//! Статически неразрешимые функции (нет имени и якоря по умолчанию; имя,
//! которого в документе нет) сводятся ДО сборки (`settle_static`): их место
//! — статическая позиция, а её выбирает сборщик дерева по `edge_set`.

mod area;
mod geometry;
pub(crate) mod parse;
pub(crate) mod place;
mod plan;
pub(crate) mod resolve;
pub(crate) mod settle;

use gpui::{AnyElement, Bounds, Pixels};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::style::computed::Align;
use crate::style::values::value::{AnchorFn, Len};
use area::Tracks;
use settle::DefaultAnchor;
mod probe;
pub use probe::key_of;
pub use probe::probe_for;
mod tf_stack;
pub(super) use tf_stack::containing_bounds;
pub use tf_stack::tf_pop;
pub use tf_stack::tf_push;
use tf_stack::{tf_map, tf_under};
mod frame;
pub use frame::next_seq;
pub use frame::reset;
use frame::{reframe_if_stale, request_rebuild};

/// Запись якоря в реестре кадра: рамка (border box), маска обрезки в точке
/// пробы и `visibility: hidden` (§position-visibility: anchor-visible), свой
/// `node_id` и ближайший содержащий блок (приемлемость якоря, §target).
#[derive(Clone, Copy, Debug)]
pub struct AnchorRec {
    pub rect: Bounds<Pixels>,
    pub clip: Bounds<Pixels>,
    pub hidden: bool,
    pub id: u64,
    pub cb: u64,
    /// Та же рамка ПОСЛЕ трансформов предков и своего (§2 «Determining the
    /// Anchor»: «includes … transforms … the axis-aligned bounding
    /// rectangle»), и номер самого внутреннего трансформа в стеке `TF`
    /// (0 — трансформов нет, `rect_tf == rect`).
    pub rect_tf: Bounds<Pixels>,
    pub tf_top: u32,
}

/// Итог размещения кандидата на подготовке кадра: сдвиг коробки, переполнил
/// ли край коробки полей inset-modified containing block (§fallback,
/// `position-visibility: no-overflow`) и размер IMCB по осям
/// (`position-try-order`).
#[derive(Clone, Copy, Debug)]
struct Placement {
    dx: f32,
    dy: f32,
    overflow: bool,
    imcb: (f32, f32),
    /// Размер клетки `position-area` по осям — она и есть содержащий блок
    /// коробки (§position-area: «makes that the box's containing block»);
    /// `(0, 0)` без области.
    cell: (f32, f32),
}

thread_local! {
    /// Именованные якоря кадра: имя → запись. Повтор имени
    /// перезаписывает — «the last element in tree order» (§target anchor
    /// element, п.3); «ближайший предок» пока не отличается.
    static NAMED: RefCell<HashMap<String, AnchorRec>> = RefCell::new(HashMap::new());
    /// Неявные якоря: `node_id` порождающего элемента → его запись (для
    /// псевдоэлементов с `position-anchor: auto`, §implicit).
    static IMPLICIT: RefCell<HashMap<u64, AnchorRec>> = RefCell::new(HashMap::new());
    /// Те же именованные пробы кадра С ПОРЯДКОМ СБОРКИ (`Computed::anchor_seq`):
    /// на следующем кадре станут `LAST_NAMED`, и размер по `anchor-size()`
    /// возьмёт «последний якорь с этим именем раньше меня по дереву».
    static NAMED_SEQ: RefCell<Vec<(String, u32, AnchorRec)>> = const { RefCell::new(Vec::new()) };
    /// Реестры ПРОШЛОГО кадра — для величин, которые нужны ДО раскладки
    /// (`anchor-size()`, растяжка в клетке `position-area`): размер решает
    /// taffy, а рамки известны только на подготовке; стенд ждёт устоявшихся
    /// кадров, и значение из кадра N−1 доезжает к N+1.
    /// Полная запись (не только рамка): прошлый кадр подменяет текущий и там,
    /// где якорь ещё не подготовлен (`AnchorPlan::rec`), — нужны `id`/`cb`
    /// для приемлемости.
    static LAST_NAMED: RefCell<Vec<(String, u32, AnchorRec)>> = const { RefCell::new(Vec::new()) };
    static LAST_IMPLICIT: RefCell<HashMap<u64, AnchorRec>> = RefCell::new(HashMap::new());
    /// Рамки СОДЕРЖАЩИХ БЛОКОВ кадра (padding box): `node_id` элемента с
    /// `establishes_cb` → рамка. Нужны сетке `position-area`.
    static CB: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    /// Размер inset-modified containing block (клетки за вычетом вставок) по
    /// ключу коробки: текущий кадр и прошлый — для `place-self: stretch`.
    static AREA_NOW: RefCell<HashMap<u64, (f32, f32)>> = RefCell::new(HashMap::new());
    static AREA_LAST: RefCell<HashMap<u64, (f32, f32)>> = RefCell::new(HashMap::new());
    /// Размер САМОЙ клетки (без вставок) — база долей размеров, полей и
    /// отбивок: текущий кадр и прошлый (`resolve_sizes`).
    static CELL_NOW: RefCell<HashMap<u64, (f32, f32)>> = RefCell::new(HashMap::new());
    static CELL_LAST: RefCell<HashMap<u64, (f32, f32)>> = RefCell::new(HashMap::new());
    /// Счётчик порядка сборки элементов в кадре (`next_seq`).
    static SEQ: Cell<u32> = const { Cell::new(0) };
    /// Стек плоских трансформов предков на подготовке кадра: номер и
    /// аффинная `[[a, b, tx], [c, d, ty]]` в css-точках окна. Кладёт
    /// `interact::Transformed::prepaint` вокруг ребёнка; номера идут заново
    /// каждый кадр (`reset`), дерево то же — номера те же.
    static TF: RefCell<Vec<(u32, [[f32; 3]; 2])>> = const { RefCell::new(Vec::new()) };
    static TF_NEXT: Cell<u32> = const { Cell::new(0) };
    /// Цепочка содержащих блоков: `node_id` блока → `node_id` его ближайшего
    /// содержащего блока (0 — начальный). Нужна приемлемости якоря (§target:
    /// содержащий блок коробки должен быть в цепочке содержащих блоков якоря).
    static CB_PARENT: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
    /// Последний удачный вариант `position-try-fallbacks` по ключу коробки
    /// (§fallback: «the element keeps those styles until it overflows again»):
    /// 0 — базовые стили, k — k-й элемент списка. Живёт между кадрами,
    /// чистится на сборке документа (`settle_static`).
    static CHOSEN: RefCell<HashMap<u64, usize>> = RefCell::new(HashMap::new());
    /// This frame's build sized a box from the PREVIOUS frame's anchor
    /// registry (`anchor-size()`, `position-area` cell percentages).
    static USED_LAST: Cell<bool> = const { Cell::new(false) };
    /// Extra frames requested for this document because those previous-frame
    /// values changed (bounded, so an oscillating page cannot loop forever).
    static REFRAMES: Cell<u32> = const { Cell::new(0) };
}

/// Одна вставка с `anchor()` и поле коробки по этой стороне: функция
/// выравнивает край ПОЛЕЙ («edge of the … inset-modified containing block»),
/// а `prepaint` видит рамку без полей.
#[derive(Clone)]
struct SidePlan {
    f: AnchorFn,
    margin: f32,
}

pub struct AnchorPlan {
    /// По часовой: top, right, bottom, left — как `Sides`.
    sides: [Option<SidePlan>; 4],
    default_anchor: Option<DefaultAnchor>,
    /// Перевёрнуты ли оси письма СОДЕРЖАЩЕГО БЛОКА (`start`/`end`/`<pct>`)
    /// и СОБСТВЕННОГО письма коробки (`self-start`/`self-end`): `(x, y)`.
    cb_flipped: (bool, bool),
    own_flipped: (bool, bool),
    /// `position-area` в физических осях `(x, y)` — только при якоре по
    /// умолчанию (§position-area: иначе «this value has no effect»).
    area: Option<(Tracks, Tracks)>,
    /// Авторские вставки (края клетки; `auto` → 0) и поля в точках — по часовой.
    inset: [Option<Len>; 4],
    margin: [f32; 4],
    align_self: Option<Align>,
    justify_self: Option<Align>,
    /// Приставка `safe` у `align-self`/`justify-self` (css-align §overflow-values).
    safe_align: bool,
    safe_justify: bool,
    cb_vertical: bool,
    /// Содержащий блок: `node_id` в реестре `CB`; 0 или `fixed` — окно.
    cb_node: u64,
    fixed: bool,
    /// Порядок сборки коробки (`Computed::anchor_seq`): якорь из реестра
    /// прошлого кадра годен, только если собран РАНЬШЕ неё (`last_named`).
    seq: u32,
    /// Ссылается ли коробка на якорь по умолчанию — `position-area`,
    /// `anchor()` без имени, `anchor-center` (§position-visibility: anchor-valid).
    refs_default: bool,
}

/// Заместитель абсолютной коробки с якорными вставками: своей коробки не
/// заводит (отдаёт `layout_id` ребёнка, как `LatePlace`), сдвиг считает на
/// подготовке кадра — к этому моменту якори раньше по дереву уже в реестре.
pub struct AnchorPlace {
    child: Option<AnyElement>,
    /// Кандидаты: [0] — базовые стили, дальше — `position-try-fallbacks`.
    plans: Vec<AnchorPlan>,
    order: u8,
    key: u64,
    visibility: u8,
}
