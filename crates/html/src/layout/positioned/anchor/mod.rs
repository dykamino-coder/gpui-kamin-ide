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

use gpui::{AnyElement, App, Bounds, IntoElement, Pixels, Styled, Window, px};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::dom::Node;
use crate::style::computed::{Align, Computed, PositionAnchor};
use crate::style::values::value::{AnchorFn, Len};
use area::Tracks;
use settle::DefaultAnchor;

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

/// Sizes resolved before layout from the previous frame are only correct once
/// the anchor geometry they read has settled; nothing else repaints a static
/// page, so ask for one more frame while it is still changing.
fn reframe_if_stale(window: &mut Window) {
    if REFRAMES.with(|r| r.get()) < 8 {
        REFRAMES.with(|r| r.set(r.get() + 1));
        request_rebuild(window);
    }
}

/// Rebuild the page on the next frame. Called from `prepaint`, where
/// `window.refresh()` is ignored (GPUI drops invalidation while drawing), so the
/// view is notified from a next-frame callback instead.
fn request_rebuild(window: &mut Window) {
    if let Some(view) = window.current_view_opt() {
        window.on_next_frame(move |window, cx| {
            cx.notify(view);
            window.refresh();
        });
    }
}

/// Расходник кадра — чистится в `interact::frame_sanitize`. Реестры текущего
/// кадра не выбрасываются, а переезжают в `LAST_*`.
pub fn reset() {
    crate::layout::positioned::absolute_overflow::reset();
    NAMED.with(|m| m.borrow_mut().clear());
    let named = NAMED_SEQ.with(|v| std::mem::take(&mut *v.borrow_mut()));
    LAST_NAMED.with(|v| *v.borrow_mut() = named);
    let implicit = IMPLICIT.with(|m| std::mem::take(&mut *m.borrow_mut()));
    LAST_IMPLICIT.with(|m| *m.borrow_mut() = implicit);
    CB.with(|m| m.borrow_mut().clear());
    CB_PARENT.with(|m| m.borrow_mut().clear());
    let area = AREA_NOW.with(|m| std::mem::take(&mut *m.borrow_mut()));
    AREA_LAST.with(|m| *m.borrow_mut() = area);
    let cell = CELL_NOW.with(|m| std::mem::take(&mut *m.borrow_mut()));
    CELL_LAST.with(|m| *m.borrow_mut() = cell);
    SEQ.with(|s| s.set(0));
    TF_NEXT.with(|n| n.set(0));
    USED_LAST.with(|u| u.set(false));
}

/// Порядковый номер сборки элемента в кадре: зовёт `render::element` в
/// порядке дерева, номер устойчив от кадра к кадру (дерево то же).
pub fn next_seq() -> u32 {
    SEQ.with(|s| {
        let v = s.get() + 1;
        s.set(v);
        v
    })
}

/// Read the actual padding box of an absolute containing block in this frame.
pub(super) fn containing_bounds(node: u64) -> Option<Bounds<Pixels>> {
    CB.with(|map| map.borrow().get(&node).copied())
}

/// Положить плоский трансформ на стек подготовки (`Transformed::prepaint`).
pub fn tf_push(m: [[f32; 3]; 2]) {
    let id = TF_NEXT.with(|n| {
        let v = n.get() + 1;
        n.set(v);
        v
    });
    TF.with(|s| s.borrow_mut().push((id, m)));
}

/// Снять трансформ со стека подготовки.
pub fn tf_pop() {
    TF.with(|s| {
        s.borrow_mut().pop();
    });
}

/// Рамка через весь стек: углы идут от ВНУТРЕННЕГО трансформа к внешнему
/// (экран = внешний(…внутренний(p))), результат — объемлющий прямоугольник
/// (§2: «axis-aligned bounding rectangle»). Второе — номер внутреннего
/// трансформа (0 — стек пуст).
fn tf_map(r: Bounds<Pixels>) -> (Bounds<Pixels>, u32) {
    TF.with(|s| {
        let s = s.borrow();
        let Some(&(top, _)) = s.last() else {
            return (r, 0);
        };
        let x0 = f32::from(r.origin.x);
        let y0 = f32::from(r.origin.y);
        let x1 = x0 + f32::from(r.size.width);
        let y1 = y0 + f32::from(r.size.height);
        let (mut lx, mut ly, mut hx, mut hy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (mut x, mut y) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
            for (_, m) in s.iter().rev() {
                (x, y) = (
                    m[0][0] * x + m[0][1] * y + m[0][2],
                    m[1][0] * x + m[1][1] * y + m[1][2],
                );
            }
            lx = lx.min(x);
            ly = ly.min(y);
            hx = hx.max(x);
            hy = hy.max(y);
        }
        let b = Bounds {
            origin: gpui::point(px(lx), px(ly)),
            size: gpui::size(px(hx - lx), px(hy - ly)),
        };
        (b, top)
    })
}

/// Лежит ли трансформ `id` на текущем стеке — то есть предок ли он коробки,
/// которая сейчас готовится.
fn tf_under(id: u32) -> bool {
    TF.with(|s| s.borrow().iter().any(|(i, _)| *i == id))
}

/// Ключ коробки в реестрах между кадрами: `node_id`; у псевдоэлемента он 0 —
/// ключ от хозяина (`implicit_anchor`) и вида.
pub fn key_of(e: &crate::dom::Element) -> u64 {
    if e.node_id != 0 {
        return e.node_id;
    }
    (1u64 << 63) | (e.style.implicit_anchor.unwrap_or(0) << 1) | u64::from(e.tag == "::after")
}

/// Ссылается ли коробка на НЕЯВНЫЙ якорь: `position-anchor: auto`, либо
/// `normal` (в том числе не задано) при непустой `position-area`
/// (§position-anchor: «normal: If position-area is none, behaves as none.
/// Otherwise, behaves as auto»).
fn wants_implicit(c: &Computed) -> bool {
    match &c.position_anchor {
        Some(PositionAnchor::Auto) => true,
        Some(PositionAnchor::Normal) | None => c.position_area.is_some(),
        _ => false,
    }
}

/// Проба якоря для коробки `e`: нужна, когда у неё есть `anchor-name` или
/// её псевдоэлемент ссылается на неё как на неявный якорь. Канвас во всю
/// коробку (`absolute` + `size_full`) — та же форма, что у `edge_probe`.
/// `hidden` — `visibility: hidden` коробки (§position-visibility).
pub fn probe_for(e: &crate::dom::Element, c: &Computed, hidden: bool) -> Option<AnyElement> {
    let names: Vec<String> = e.style.anchor_name.clone().unwrap_or_default();
    let implicit = e.node_id != 0
        && e.children.iter().any(|n| {
            matches!(n, Node::Element(k) if k.tag.starts_with("::") && wants_implicit(&k.style))
        });
    // Содержащий блок абсолюта — каждая коробка с `establishes_cb`: её
    // padding box читает сетка `position-area` (§position-area-grid-resolution),
    // связь с её собственным содержащим блоком — приемлемость якоря (§target).
    let cb = e.node_id != 0 && crate::text::inline::establishes_cb(&e.style);
    if names.is_empty() && !implicit && !cb {
        return None;
    }
    let id = e.node_id;
    let seq = c.anchor_seq;
    let own_cb = c.cb_node;
    // CSS Anchor Positioning 1 resolves logical border-box edges before snapping.
    // The absolute probe covers the padding box; expand it by the CSS borders.
    // The containing-block record remains the padding box (CSS 2.1 section 10.1).
    let bw = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = e.style.borders();
    let border = [bw(b.top), bw(b.right), bw(b.bottom), bw(b.left)];
    Some(
        gpui::canvas_with_unrounded_bounds(
            move |bounds: Bounds<Pixels>, window: &mut Window, _: &mut App| {
                if cb {
                    CB.with(|m| m.borrow_mut().insert(id, bounds));
                    CB_PARENT.with(|m| m.borrow_mut().insert(id, own_cb));
                }
                if names.is_empty() && !implicit {
                    return;
                }
                let outer = Bounds {
                    origin: gpui::point(
                        bounds.origin.x - px(border[3]),
                        bounds.origin.y - px(border[0]),
                    ),
                    size: gpui::size(
                        bounds.size.width + px(border[1] + border[3]),
                        bounds.size.height + px(border[0] + border[2]),
                    ),
                };
                // Маска обрезки в точке пробы — пересечение `overflow`-обрезок
                // всех предков: по ней `AnchorPlace` решает, обрезан ли якорь
                // промежуточными коробками (§position-visibility).
                // Рамка раскладки — до трансформов: `Transformed` матрицу
                // применяет только на отрисовке. Отображённую снимаем здесь же
                // по стеку предков (`tf_map`), выбирает её цель (`lookup`):
                // `transform-001/002/009` — якорь с `translate(-200px, -100px)
                // scale(2)` обязан стоять там, где нарисован.
                let (rect_tf, tf_top) = tf_map(outer);
                let rec = AnchorRec {
                    rect: outer,
                    clip: window.content_mask().bounds,
                    hidden,
                    id,
                    cb: own_cb,
                    rect_tf,
                    tf_top,
                };
                NAMED.with(|m| {
                    let mut m = m.borrow_mut();
                    for n in &names {
                        m.insert(n.clone(), rec);
                    }
                });
                NAMED_SEQ.with(|v| {
                    let mut v = v.borrow_mut();
                    for n in &names {
                        v.push((n.clone(), seq, rec));
                    }
                });
                if implicit {
                    IMPLICIT.with(|m| m.borrow_mut().insert(id, rec));
                }
                if USED_LAST.with(|u| u.get()) {
                    let same = |r: &AnchorRec| r.rect == rec.rect && r.rect_tf == rec.rect_tf;
                    let stale = names.iter().any(|n| {
                        !LAST_NAMED.with(|v| {
                            v.borrow()
                                .iter()
                                .any(|(k, s, r)| k == n && *s == seq && same(r))
                        })
                    }) || (implicit
                        && !LAST_IMPLICIT.with(|m| m.borrow().get(&id).is_some_and(same)));
                    if stale {
                        reframe_if_stale(window);
                    }
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
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
