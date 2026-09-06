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

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Styled, Window, px,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::computed::{Align, Computed, Position, PositionAnchor};
use crate::dom::Node;
use crate::value::{AnchorFn, AnchorSide, AnchorSize, Len, anchor_get};

thread_local! {
    /// Именованные якоря кадра: имя → рамка (border box). Повтор имени
    /// перезаписывает — «the last element in tree order» (§target anchor
    /// element, п.3); «ближайший предок» пока не отличается.
    static NAMED: RefCell<HashMap<String, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    /// Неявные якоря: `node_id` порождающего элемента → его рамка (для
    /// псевдоэлементов с `position-anchor: auto`, §implicit).
    static IMPLICIT: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    /// Те же именованные пробы кадра С ПОРЯДКОМ СБОРКИ (`Computed::anchor_seq`):
    /// на следующем кадре станут `LAST_NAMED`, и размер по `anchor-size()`
    /// возьмёт «последний якорь с этим именем раньше меня по дереву».
    static NAMED_SEQ: RefCell<Vec<(String, u32, Bounds<Pixels>)>> = RefCell::new(Vec::new());
    /// Реестры ПРОШЛОГО кадра — для величин, которые нужны ДО раскладки
    /// (`anchor-size()`, растяжка в клетке `position-area`): размер решает
    /// taffy, а рамки известны только на подготовке; стенд ждёт устоявшихся
    /// кадров, и значение из кадра N−1 доезжает к N+1.
    static LAST_NAMED: RefCell<Vec<(String, u32, Bounds<Pixels>)>> = RefCell::new(Vec::new());
    static LAST_IMPLICIT: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    /// Рамки СОДЕРЖАЩИХ БЛОКОВ кадра (padding box): `node_id` элемента с
    /// `establishes_cb` → рамка. Нужны сетке `position-area`.
    static CB: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    /// Размер inset-modified containing block (клетки за вычетом вставок) по
    /// ключу коробки: текущий кадр и прошлый — для `place-self: stretch`.
    static AREA_NOW: RefCell<HashMap<u64, (f32, f32)>> = RefCell::new(HashMap::new());
    static AREA_LAST: RefCell<HashMap<u64, (f32, f32)>> = RefCell::new(HashMap::new());
    /// Счётчик порядка сборки элементов в кадре (`next_seq`).
    static SEQ: Cell<u32> = const { Cell::new(0) };
}

/// Расходник кадра — чистится в `interact::frame_sanitize`. Реестры текущего
/// кадра не выбрасываются, а переезжают в `LAST_*`.
pub fn reset() {
    NAMED.with(|m| m.borrow_mut().clear());
    let named = NAMED_SEQ.with(|v| std::mem::take(&mut *v.borrow_mut()));
    LAST_NAMED.with(|v| *v.borrow_mut() = named);
    let implicit = IMPLICIT.with(|m| std::mem::take(&mut *m.borrow_mut()));
    LAST_IMPLICIT.with(|m| *m.borrow_mut() = implicit);
    CB.with(|m| m.borrow_mut().clear());
    let area = AREA_NOW.with(|m| std::mem::take(&mut *m.borrow_mut()));
    AREA_LAST.with(|m| *m.borrow_mut() = area);
    SEQ.with(|s| s.set(0));
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
pub fn probe_for(e: &crate::dom::Element, c: &Computed) -> Option<AnyElement> {
    let names: Vec<String> = e.style.anchor_name.clone().unwrap_or_default();
    let implicit = e.node_id != 0
        && e.children.iter().any(|n| {
            matches!(n, Node::Element(k) if k.tag.starts_with("::") && wants_implicit(&k.style))
        });
    // Содержащий блок абсолюта — каждая коробка с `establishes_cb`: её
    // padding box читает сетка `position-area` (§position-area-grid-resolution).
    let cb = e.node_id != 0 && crate::inline::establishes_cb(&e.style);
    if names.is_empty() && !implicit && !cb {
        return None;
    }
    let id = e.node_id;
    let seq = c.anchor_seq;
    // Рамка якоря по спеке — BORDER box (§determining), а абсолютный канвас
    // с нулевыми вставками раскладка ставит в PADDING box (taffy: «insets
    // are resolved against the container size minus border»). Расширяем на
    // видимую рамку; содержащему блоку нужен как раз padding box — как есть.
    let bw = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = e.style.borders();
    let border = [bw(b.top), bw(b.right), bw(b.bottom), bw(b.left)];
    Some(
        gpui::canvas(
            move |bounds: Bounds<Pixels>, _, _| {
                if cb {
                    CB.with(|m| m.borrow_mut().insert(id, bounds));
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
                NAMED.with(|m| {
                    let mut m = m.borrow_mut();
                    for n in &names {
                        m.insert(n.clone(), outer);
                    }
                });
                NAMED_SEQ.with(|v| {
                    let mut v = v.borrow_mut();
                    for n in &names {
                        v.push((n.clone(), seq, outer));
                    }
                });
                if implicit {
                    IMPLICIT.with(|m| m.borrow_mut().insert(id, outer));
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

/// Статически неразрешимые `anchor()` решаются до отрисовки (§anchor-resolution:
/// неразрешимая функция «computes to its specified fallback value. If no
/// fallback value is specified, it makes the declaration … invalid at
/// computed-value time» — вставка становится `auto`, коробка остаётся на
/// статической позиции). Решать это на подготовке кадра поздно: слои и
/// щупы статической позиции выбираются при сборке дерева по `edge_set`.
/// Без этого шага две пары, зелёные сейчас, уходили бы в красное
/// (`position-anchor-none/normal-pseudo-element-implicit-002`).
pub fn settle_static(nodes: &mut [Node]) {
    fn names(nodes: &[Node], out: &mut HashSet<String>) {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            if let Some(list) = &e.style.anchor_name {
                out.extend(list.iter().cloned());
            }
            names(&e.children, out);
        }
    }
    fn settle(nodes: &mut [Node], known: &HashSet<String>) {
        for n in nodes.iter_mut() {
            let Node::Element(e) = n else { continue };
            let has_default = match &e.style.position_anchor {
                Some(PositionAnchor::Named(n)) => known.contains(n),
                Some(PositionAnchor::Auto) => e.style.implicit_anchor.is_some(),
                // `normal` при непустой `position-area` ведёт себя как `auto`
                // (§position-anchor); без области — как `none`.
                Some(PositionAnchor::Normal) | None => {
                    e.style.position_area.is_some() && e.style.implicit_anchor.is_some()
                }
                _ => false,
            };
            // Список запасных позиций без самого перебора: коробка застряла бы
            // в первом варианте, даже когда спека велит его заменить за
            // переполнение (`position-try-fallbacks-001`: `bottom:
            // anchor(outside)` + `flip-block` — коробка над якорем, за верхом
            // содержащего блока). Пока перебора нет — как до шага 1: `anchor()`
            // неразрешима, вставка падает в запасное значение или в `auto`.
            let empty = HashSet::new();
            let (has_default, known_here) = if e.style.position_try {
                (false, &empty)
            } else {
                (has_default, known)
            };
            let s = &mut e.style.inset;
            for slot in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                *slot = settle_len(*slot, has_default, known_here);
            }
            settle(&mut e.children, known);
        }
    }
    let mut known = HashSet::new();
    names(nodes, &mut known);
    settle(nodes, &known);
}

/// Одна вставка: цепочка запасных значений раскручивается, пока не
/// найдётся разрешимая функция или обычная длина.
fn settle_len(l: Option<Len>, has_default: bool, known: &HashSet<String>) -> Option<Len> {
    let mut cur = l;
    for _ in 0..4 {
        let Some(Len::Anchor(i)) = cur else { return cur };
        let f = anchor_get(i)?;
        let resolvable = match &f.name {
            Some(n) => known.contains(n),
            None => has_default,
        };
        if resolvable {
            return cur;
        }
        cur = f.fallback;
    }
    cur
}

/// Якорь по умолчанию коробки (§position-anchor).
#[derive(Clone, Debug)]
enum DefaultAnchor {
    Named(String),
    Implicit(u64),
}

/// `position-anchor` → якорь по умолчанию; `normal`/не задано при непустой
/// `position-area` — как `auto` (§position-anchor).
fn default_anchor_of(c: &Computed) -> Option<DefaultAnchor> {
    match &c.position_anchor {
        Some(PositionAnchor::Named(n)) => Some(DefaultAnchor::Named(n.clone())),
        Some(PositionAnchor::Auto) => c.implicit_anchor.map(DefaultAnchor::Implicit),
        Some(PositionAnchor::Normal) | None if c.position_area.is_some() => {
            c.implicit_anchor.map(DefaultAnchor::Implicit)
        }
        _ => None,
    }
}

/// Дорожки `position-area` по одной оси (§position-area-syntax).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AreaKw {
    Start,
    Center,
    End,
    SpanStart,
    SpanEnd,
    SpanAll,
}

/// Ось ключевого слова: физическая, логическая по письму, либо неясная
/// (`start`/`end`/`center`/`span-all` без оси — решается по соседу).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AreaAxis {
    X,
    Y,
    Block,
    Inline,
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaSide {
    pub kw: AreaKw,
    pub axis: AreaAxis,
    /// `left`/`right`/`top`/`bottom` — физические стороны (без переворота
    /// письмом); прочие слова — логические.
    pub logical: bool,
    /// `self-*` — по письму самой коробки, иначе содержащего блока.
    pub self_wm: bool,
}

/// `position-area` как записана: два слова (одно дополняется по §syntax).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionArea(pub AreaSide, pub AreaSide);

/// Разбор `<position-area>` (§position-area-syntax). `none` и негодное → `None`.
pub fn parse_area(v: &str) -> Option<PositionArea> {
    fn one(w: &str) -> Option<AreaSide> {
        let w = w.to_ascii_lowercase();
        let (span, rest) = match w.strip_prefix("span-") {
            Some(r) => (true, r),
            None => (false, w.as_str()),
        };
        let any = |kw| {
            Some(AreaSide {
                kw,
                axis: AreaAxis::Any,
                logical: true,
                self_wm: false,
            })
        };
        match (span, rest) {
            (true, "all") => return any(AreaKw::SpanAll),
            (false, "center") => return any(AreaKw::Center),
            (_, "all") | (_, "center") => return None,
            _ => {}
        }
        let (self_wm, rest) = match rest.strip_prefix("self-") {
            Some(r) => (true, r),
            None => (false, rest),
        };
        let (axis, end, logical) = match rest {
            "left" if !self_wm => (AreaAxis::X, false, false),
            "right" if !self_wm => (AreaAxis::X, true, false),
            "top" if !self_wm => (AreaAxis::Y, false, false),
            "bottom" if !self_wm => (AreaAxis::Y, true, false),
            "x-start" => (AreaAxis::X, false, true),
            "x-end" => (AreaAxis::X, true, true),
            "y-start" => (AreaAxis::Y, false, true),
            "y-end" => (AreaAxis::Y, true, true),
            "block-start" => (AreaAxis::Block, false, true),
            "block-end" => (AreaAxis::Block, true, true),
            "inline-start" => (AreaAxis::Inline, false, true),
            "inline-end" => (AreaAxis::Inline, true, true),
            "start" => (AreaAxis::Any, false, true),
            "end" => (AreaAxis::Any, true, true),
            _ => return None,
        };
        let kw = match (span, end) {
            (false, false) => AreaKw::Start,
            (false, true) => AreaKw::End,
            (true, false) => AreaKw::SpanStart,
            (true, true) => AreaKw::SpanEnd,
        };
        Some(AreaSide {
            kw,
            axis,
            logical,
            self_wm,
        })
    }
    if v.trim() == "none" {
        return None;
    }
    let words: Vec<&str> = v.split_whitespace().collect();
    let (a, b) = match words.as_slice() {
        // Одно слово: с ясной осью второе — `span-all`, иначе повтор.
        [a] => {
            let a = one(a)?;
            let b = if a.axis == AreaAxis::Any {
                a
            } else {
                AreaSide {
                    kw: AreaKw::SpanAll,
                    axis: AreaAxis::Any,
                    logical: true,
                    self_wm: false,
                }
            };
            (a, b)
        }
        [a, b] => (one(a)?, one(b)?),
        _ => return None,
    };
    Some(PositionArea(a, b))
}

/// Линии сетки 3×3 по одной физической оси: 0 — начало содержащего блока,
/// 1 — начало якоря, 2 — конец якоря, 3 — конец содержащего блока; уже в
/// физическом порядке (`lo` левее/выше).
#[derive(Clone, Copy, Debug)]
struct Tracks {
    lo: u8,
    hi: u8,
}

fn tracks_of(kw: AreaKw, flip: bool) -> Tracks {
    let (lo, hi) = match kw {
        AreaKw::Start => (0, 1),
        AreaKw::Center => (1, 2),
        AreaKw::End => (2, 3),
        AreaKw::SpanStart => (0, 2),
        AreaKw::SpanEnd => (1, 3),
        AreaKw::SpanAll => (0, 3),
    };
    if flip {
        Tracks {
            lo: 3 - hi,
            hi: 3 - lo,
        }
    } else {
        Tracks { lo, hi }
    }
}

/// Область в физических осях `(x, y)` — как `PositionArea::ToPhysical` у
/// Blink: неясное слово берёт ось, противоположную соседу; оба неясных —
/// первое блочная ось, второе строчная (письмо коробки, если слова `self-*`).
fn physical_area(area: PositionArea, cb: &Computed, own: &Computed) -> (Tracks, Tracks) {
    let wm_vertical = |s: AreaSide| {
        if s.self_wm {
            own.vertical == Some(true)
        } else {
            cb.vertical == Some(true)
        }
    };
    let axis_x = |s: AreaSide| -> Option<bool> {
        match s.axis {
            AreaAxis::X => Some(true),
            AreaAxis::Y => Some(false),
            AreaAxis::Block => Some(wm_vertical(s)),
            AreaAxis::Inline => Some(!wm_vertical(s)),
            AreaAxis::Any => None,
        }
    };
    let (a_x, b_x) = match (axis_x(area.0), axis_x(area.1)) {
        (Some(a), Some(b)) if a != b => (a, b),
        (Some(a), _) => (a, !a),
        (None, Some(b)) => (!b, b),
        (None, None) => {
            let v = if area.0.self_wm || area.1.self_wm {
                own.vertical == Some(true)
            } else {
                cb.vertical == Some(true)
            };
            (v, !v)
        }
    };
    // Логическое слово переворачивается письмом своей оси (как `start`/`end`
    // в `anchor()`); физическое — никогда.
    let flip_of = |s: AreaSide, x: bool| -> bool {
        if !s.logical {
            return false;
        }
        let f = flipped(if s.self_wm { own } else { cb });
        if x { f.0 } else { f.1 }
    };
    let ta = tracks_of(area.0.kw, flip_of(area.0, a_x));
    let tb = tracks_of(area.1.kw, flip_of(area.1, b_x));
    if a_x { (ta, tb) } else { (tb, ta) }
}

/// Выравнивание коробки в клетке по одной оси.
#[derive(Clone, Copy, PartialEq)]
enum Al {
    Start,
    Center,
    End,
    AnchorCenter,
    Stretch,
}

/// Умолчание `normal` по области (§position-area-alignment; Blink
/// `AlignJustifySelfFromPhysical`): только центр → `center`; все три →
/// `anchor-center`; иначе — к неназванной дорожке.
fn default_al(t: Tracks) -> Al {
    match (t.lo, t.hi) {
        (0, 3) => Al::AnchorCenter,
        (1, 2) => Al::Center,
        (0, _) => Al::End,
        _ => Al::Start,
    }
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
    cb_vertical: bool,
    /// Содержащий блок: `node_id` в реестре `CB`; 0 или `fixed` — окно.
    cb_node: u64,
    fixed: bool,
    /// Ключ в `AREA_NOW` (размер клетки для растяжки на следующем кадре).
    key: u64,
}

/// Как `IsFlippedX`/`IsFlippedY` у Blink: x перевёрнут при горизонтальном
/// rtl и при vertical-rl, y — при вертикальном письме с `direction: rtl`.
fn flipped(c: &Computed) -> (bool, bool) {
    let vertical = c.vertical == Some(true);
    let rtl = c.rtl == Some(true);
    let x = if vertical { c.vertical_rl == Some(true) } else { rtl };
    (x, vertical && rtl)
}

impl AnchorPlan {
    /// План есть у абсолюта хотя бы с одной `anchor()`-вставкой или с
    /// `position-area` при якоре по умолчанию.
    fn of(own: &Computed, inherited: &Computed) -> Option<AnchorPlan> {
        if !matches!(own.position, Some(Position::Absolute) | Some(Position::Fixed)) {
            return None;
        }
        let m = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let margin = [
            m(own.margin.top),
            m(own.margin.right),
            m(own.margin.bottom),
            m(own.margin.left),
        ];
        let side = |l: Option<Len>, margin: f32| -> Option<SidePlan> {
            let Some(Len::Anchor(i)) = l else { return None };
            let f = anchor_get(i)?;
            // `anchor-size()` во вставке — величина, не край: сдвига не даёт.
            if f.size.is_some() {
                return None;
            }
            Some(SidePlan { f, margin })
        };
        let inset = [own.inset.top, own.inset.right, own.inset.bottom, own.inset.left];
        let sides = [
            side(inset[0], margin[0]),
            side(inset[1], margin[1]),
            side(inset[2], margin[2]),
            side(inset[3], margin[3]),
        ];
        let default_anchor = default_anchor_of(own);
        let area = own
            .position_area
            .filter(|_| default_anchor.is_some())
            .map(|a| physical_area(a, inherited, own));
        if sides.iter().all(Option::is_none) && area.is_none() {
            return None;
        }
        Some(AnchorPlan {
            sides,
            default_anchor,
            cb_flipped: flipped(inherited),
            own_flipped: flipped(own),
            area,
            inset,
            margin,
            align_self: own.align_self,
            justify_self: own.justify_self,
            cb_vertical: inherited.vertical == Some(true),
            cb_node: own.cb_node,
            fixed: own.position == Some(Position::Fixed)
                && !(inherited.transform_ancestor || inherited.transform.is_some()),
            key: own.anchor_key,
        })
    }

    fn lookup(&self, name: Option<&str>) -> Option<Bounds<Pixels>> {
        match name {
            Some(n) => NAMED.with(|m| m.borrow().get(n).copied()),
            None => match &self.default_anchor {
                Some(DefaultAnchor::Named(n)) => {
                    NAMED.with(|m| m.borrow().get(n.as_str()).copied())
                }
                Some(DefaultAnchor::Implicit(id)) => {
                    IMPLICIT.with(|m| m.borrow().get(id).copied())
                }
                None => None,
            },
        }
    }

    /// Доля [0;1] вдоль физической оси от её начала, куда указывает
    /// `<anchor-side>`; `None` — сторона другой оси (функция неразрешима).
    /// Таблица — как `ResolveAnchorValue` у Blink.
    fn fraction(&self, side: AnchorSide, y_axis: bool, end_side: bool) -> Option<f32> {
        let cb = if y_axis { self.cb_flipped.1 } else { self.cb_flipped.0 };
        let own = if y_axis { self.own_flipped.1 } else { self.own_flipped.0 };
        let flip = |f: bool, t: f32| if f { 1.0 - t } else { t };
        Some(match side {
            AnchorSide::Top if y_axis => 0.0,
            AnchorSide::Bottom if y_axis => 1.0,
            AnchorSide::Left if !y_axis => 0.0,
            AnchorSide::Right if !y_axis => 1.0,
            AnchorSide::Top | AnchorSide::Bottom | AnchorSide::Left | AnchorSide::Right => {
                return None;
            }
            AnchorSide::Inside => {
                if end_side {
                    1.0
                } else {
                    0.0
                }
            }
            AnchorSide::Outside => {
                if end_side {
                    0.0
                } else {
                    1.0
                }
            }
            AnchorSide::Start => flip(cb, 0.0),
            AnchorSide::End => flip(cb, 1.0),
            AnchorSide::SelfStart => flip(own, 0.0),
            AnchorSide::SelfEnd => flip(own, 1.0),
            AnchorSide::Pct(p) => flip(cb, p),
        })
    }

    /// Куда просится край по `anchor()`: экранная координата; вставка в
    /// точках от края содержащего блока (запасное значение); ничего.
    fn resolve_anchor(&self, first: AnchorFn, y_axis: bool, end_side: bool) -> Edge {
        let mut f = Some(first);
        // Цепочка запасных значений: `anchor(top, anchor(--a1 bottom))`.
        for _ in 0..4 {
            let Some(cur) = f.take() else { break };
            let hit = self.lookup(cur.name.as_deref()).and_then(|a| {
                let t = self.fraction(cur.side, y_axis, end_side)?;
                let (start, len) = if y_axis {
                    (f32::from(a.origin.y), f32::from(a.size.height))
                } else {
                    (f32::from(a.origin.x), f32::from(a.size.width))
                };
                Some(start + len * t + cur.add)
            });
            match (hit, cur.fallback) {
                (Some(want), _) => return Edge::Abs(want),
                (None, Some(Len::Px(v))) => return Edge::FromEdge(v),
                (None, Some(Len::Anchor(j))) => f = anchor_get(j),
                // Якоря нет и запаса нет: по спеке — статическая позиция,
                // но статически это было неотличимо (`settle_static` такие
                // уже снял); остаток — якорь, который есть в документе, но
                // ниже по дереву (`anchor-position-circular`).
                (None, _) => return Edge::None,
            }
        }
        Edge::None
    }

    /// Сдвиг по оси стороны `k` (0 top, 1 right, 2 bottom, 3 left) — без
    /// `position-area`: коробка стоит у края содержащего блока на нулевой
    /// вставке.
    fn shift(&self, k: usize, own: Bounds<Pixels>) -> f32 {
        let Some(sp) = &self.sides[k] else { return 0.0 };
        let y_axis = k % 2 == 0;
        let end_side = k == 1 || k == 2;
        // Край ПОЛЕЙ коробки по этой стороне.
        let own_edge = match k {
            0 => f32::from(own.origin.y) - sp.margin,
            1 => f32::from(own.origin.x) + f32::from(own.size.width) + sp.margin,
            2 => f32::from(own.origin.y) + f32::from(own.size.height) + sp.margin,
            _ => f32::from(own.origin.x) - sp.margin,
        };
        match self.resolve_anchor(sp.f.clone(), y_axis, end_side) {
            Edge::Abs(want) => want - own_edge,
            // Запасная длина в точках — вставка от края содержащего
            // блока; раскладка уже поставила коробку на нулевую вставку.
            Edge::FromEdge(v) => {
                if end_side {
                    -v
                } else {
                    v
                }
            }
            Edge::None => 0.0,
        }
    }

    /// Край inset-modified containing block со стороны `k`: край клетки,
    /// сдвинутый авторской вставкой (`auto` → 0; доля — от клетки, она и
    /// есть содержащий блок; `anchor()` — к краю якоря, §position-area).
    fn imcb_edge(&self, k: usize, cell_edge: f32, cell_len: f32) -> f32 {
        let y_axis = k % 2 == 0;
        let end_side = k == 1 || k == 2;
        let sign = if end_side { -1.0 } else { 1.0 };
        match self.inset[k] {
            Some(Len::Px(v)) => cell_edge + sign * v,
            Some(Len::Pct(p)) => cell_edge + sign * p * cell_len,
            Some(Len::Anchor(i)) => match anchor_get(i) {
                Some(f) if f.size.is_none() => match self.resolve_anchor(f, y_axis, end_side) {
                    Edge::Abs(w) => w,
                    Edge::FromEdge(v) => cell_edge + sign * v,
                    Edge::None => cell_edge,
                },
                _ => cell_edge,
            },
            _ => cell_edge,
        }
    }

    /// Выравнивание в оси и нужен ли сдвиг в исходный содержащий блок
    /// (§position-area-alignment; Blink `ComputeAlignment`): явное значение;
    /// иначе `normal` — к единственной не-`auto` вставке оси (unsafe), иначе
    /// умолчание области.
    fn align_for(&self, x_axis: bool, t: Tracks) -> (Al, bool) {
        // css-align: `align-self` — блочная ось СОДЕРЖАЩЕГО БЛОКА,
        // `justify-self` — строчная.
        let explicit = if x_axis == self.cb_vertical {
            self.align_self
        } else {
            self.justify_self
        };
        if let Some(a) = explicit {
            let al = match a {
                Align::Center => Al::Center,
                Align::End => Al::End,
                Align::Stretch => Al::Stretch,
                Align::Start | Align::Baseline => Al::Start,
            };
            return (al, true);
        }
        let set = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
        let (s_set, e_set) = if x_axis {
            (set(self.inset[3]), set(self.inset[1]))
        } else {
            (set(self.inset[0]), set(self.inset[2]))
        };
        match (s_set, e_set) {
            (true, false) => (Al::Start, false),
            (false, true) => (Al::End, false),
            _ => (default_al(t), true),
        }
    }

    /// Рамка исходного содержащего блока: реестр `CB` по `cb_node`; корень
    /// и `fixed` — окно. Нет в реестре (блок не через `styled_div_with`,
    /// например строчный) — область не применяется.
    fn cb_bounds(&self, window: &Window) -> Option<Bounds<Pixels>> {
        if self.fixed || self.cb_node == 0 {
            return Some(Bounds {
                origin: gpui::point(px(0.0), px(0.0)),
                size: window.viewport_size(),
            });
        }
        CB.with(|m| m.borrow().get(&self.cb_node).copied())
    }

    /// Одна ось области: сетка → клетка → inset-modified containing block →
    /// выравнивание → сдвиг границы коробки и длина IMCB (для растяжки).
    fn axis_place(
        &self,
        x: bool,
        t: Tracks,
        a: Bounds<Pixels>,
        cb: Bounds<Pixels>,
        own: Bounds<Pixels>,
    ) -> (f32, f32) {
        let f = f32::from;
        let (cs, ce, as_, ae, os, olen) = if x {
            (
                f(cb.origin.x),
                f(cb.origin.x) + f(cb.size.width),
                f(a.origin.x),
                f(a.origin.x) + f(a.size.width),
                f(own.origin.x),
                f(own.size.width),
            )
        } else {
            (
                f(cb.origin.y),
                f(cb.origin.y) + f(cb.size.height),
                f(a.origin.y),
                f(a.origin.y) + f(a.size.height),
                f(own.origin.y),
                f(own.size.height),
            )
        };
        // §position-area-grid-resolution: якорь за краем содержащего блока
        // раздвигает крайние линии.
        let lines = [cs.min(as_), as_, ae, ce.max(ae)];
        let (s, e) = (lines[t.lo as usize], lines[t.hi as usize]);
        let (k_start, k_end) = if x { (3usize, 1usize) } else { (0, 2) };
        let is = self.imcb_edge(k_start, s, e - s);
        let ie = self.imcb_edge(k_end, e, e - s);
        let (m_s, m_e) = (self.margin[k_start], self.margin[k_end]);
        // Выравнивается коробка ПОЛЕЙ (`auto`-поля уже нули: `m` в `of`).
        let mbox = olen + m_s + m_e;
        let (al, shift_in) = self.align_for(x, t);
        let mut pos = match al {
            Al::Start | Al::Stretch => is,
            Al::End => ie - mbox,
            Al::Center => (is + ie - mbox) / 2.0,
            Al::AnchorCenter => (as_ + ae - mbox) / 2.0,
        };
        // Умолчание css-align для абсолюта: переполнил IMCB, но влезает в
        // исходный содержащий блок — «shift … to stay within».
        if shift_in && mbox <= ce - cs {
            pos = pos.clamp(cs, ce - mbox);
        }
        (pos + m_s - os, (ie - is).max(0.0))
    }

    /// Сдвиг коробки в клетку `position-area`; `None` — области нет или
    /// якорь/содержащий блок не найдены (тогда — обычные `anchor()`-вставки).
    fn area_place(&self, own: Bounds<Pixels>, window: &Window) -> Option<(f32, f32)> {
        let (tx, ty) = self.area?;
        let a = self.lookup(None)?;
        let cb = self.cb_bounds(window)?;
        let (dx, w) = self.axis_place(true, tx, a, cb, own);
        let (dy, h) = self.axis_place(false, ty, a, cb, own);
        AREA_NOW.with(|m| m.borrow_mut().insert(self.key, (w, h)));
        Some((dx, dy))
    }
}

/// Куда просится край по `anchor()`.
enum Edge {
    Abs(f32),
    FromEdge(f32),
    None,
}

/// Обернуть готовую коробку сдвигом к якорю; без `anchor()`-вставок — как есть.
pub fn place(el: AnyElement, own: &Computed, inherited: &Computed) -> AnyElement {
    match AnchorPlan::of(own, inherited) {
        Some(plan) => AnchorPlace {
            child: Some(el),
            plan,
        }
        .into_any_element(),
        None => el,
    }
}

/// Заместитель абсолютной коробки с якорными вставками: своей коробки не
/// заводит (отдаёт `layout_id` ребёнка, как `LatePlace`), сдвиг считает на
/// подготовке кадра — к этому моменту якори раньше по дереву уже в реестре.
pub struct AnchorPlace {
    child: Option<AnyElement>,
    plan: AnchorPlan,
}

impl Element for AnchorPlace {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.as_mut().unwrap().request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        // `position-area` считает обе оси сама (вставки там — края клетки).
        // Без области по оси побеждает начальная сторона: `left` при
        // заданных `left` и `right` (растяжка между двумя якорями меняет
        // РАЗМЕР — отдельный шаг).
        let (dx, dy) = match self.plan.area_place(bounds, window) {
            Some(d) => d,
            None => (
                if self.plan.sides[3].is_some() {
                    self.plan.shift(3, bounds)
                } else {
                    self.plan.shift(1, bounds)
                },
                if self.plan.sides[0].is_some() {
                    self.plan.shift(0, bounds)
                } else {
                    self.plan.shift(2, bounds)
                },
            ),
        };
        let child = self.child.as_mut().unwrap();
        window.with_element_offset(gpui::point(px(dx), px(dy)), |window| {
            child.prepaint(window, cx)
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.as_mut().unwrap().paint(window, cx);
    }
}

impl IntoElement for AnchorPlace {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

/// Якорь из реестра ПРОШЛОГО кадра: по имени — последняя запись с этим
/// именем, собранная РАНЬШЕ коробки (`seq`), — «the last element in tree
/// order» среди «laid out strictly before» (§target anchor element);
/// неявный — по `node_id` хозяина.
fn last_lookup(name: Option<&str>, default: &Option<DefaultAnchor>, seq: u32) -> Option<Bounds<Pixels>> {
    let by_name = |n: &str| {
        LAST_NAMED.with(|v| {
            v.borrow()
                .iter()
                .filter(|(k, s, _)| k == n && *s < seq)
                .max_by_key(|(_, s, _)| *s)
                .map(|(_, _, b)| *b)
        })
    };
    match name {
        Some(n) => by_name(n),
        None => match default {
            Some(DefaultAnchor::Named(n)) => by_name(n),
            Some(DefaultAnchor::Implicit(id)) => LAST_IMPLICIT.with(|m| m.borrow().get(id).copied()),
            None => None,
        },
    }
}

/// Размеры абсолюта, зависящие от якоря, — В ТОЧКИ до раскладки (зовёт
/// `render::element` сразу после слияния стилей):
/// * `anchor-size()` в `width/height/min-*/max-*` (§anchor-size-fn) — из
///   реестра прошлого кадра; неразрешимая — запасное значение, без него
///   `auto` («invalid at computed-value time»);
/// * `place-self: stretch` в клетке `position-area` — размер IMCB прошлого
///   кадра за вычетом полей, рамки и отбивок (`width` у нас — содержимое).
/// Первый кадр отдаёт запасные значения, второй — верные; стенд ждёт
/// устоявшихся кадров.
pub fn resolve_sizes(c: &mut Computed, inherited: &Computed) {
    if !matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed)) {
        return;
    }
    let seq = c.anchor_seq;
    let default_anchor = default_anchor_of(c);
    let cb_vertical = inherited.vertical == Some(true);
    let own_vertical = c.vertical == Some(true);
    let one = |l: Option<Len>, y_axis: bool| -> Option<Len> {
        let Some(Len::Anchor(i)) = l else { return l };
        let mut f = anchor_get(i);
        for _ in 0..4 {
            let Some(cur) = f.take() else { break };
            // `anchor()` в размере негодна (§anchor-fn: только вставки).
            let Some(kind) = cur.size else { return None };
            let hit = last_lookup(cur.name.as_deref(), &default_anchor, seq).map(|b| {
                let width = match kind {
                    AnchorSize::Width => true,
                    AnchorSize::Height => false,
                    AnchorSize::Implicit => !y_axis,
                    AnchorSize::Block => cb_vertical,
                    AnchorSize::Inline => !cb_vertical,
                    AnchorSize::SelfBlock => own_vertical,
                    AnchorSize::SelfInline => !own_vertical,
                };
                let v = if width { b.size.width } else { b.size.height };
                f32::from(v) + cur.add
            });
            match (hit, cur.fallback) {
                (Some(v), _) => return Some(Len::Px(v)),
                (None, Some(Len::Anchor(j))) => f = anchor_get(j),
                (None, fb) => return fb,
            }
        }
        None
    };
    c.width = one(c.width, false);
    c.min_width = one(c.min_width, false);
    c.max_width = one(c.max_width, false);
    c.height = one(c.height, true);
    c.min_height = one(c.min_height, true);
    c.max_height = one(c.max_height, true);
    if c.position_area.is_none() || default_anchor.is_none() {
        return;
    }
    let Some((w, h)) = AREA_LAST.with(|m| m.borrow().get(&c.anchor_key).copied()) else {
        return;
    };
    let pxv = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let block_stretch = c.align_self == Some(Align::Stretch);
    let inline_stretch = c.justify_self == Some(Align::Stretch);
    let (x_stretch, y_stretch) = if cb_vertical {
        (block_stretch, inline_stretch)
    } else {
        (inline_stretch, block_stretch)
    };
    let b = c.borders();
    if x_stretch {
        let extra = pxv(c.margin.left) + pxv(c.margin.right) + pxv(b.left) + pxv(b.right)
            + pxv(c.padding.left) + pxv(c.padding.right);
        c.width = Some(Len::Px((w - extra).max(0.0)));
    }
    if y_stretch {
        let extra = pxv(c.margin.top) + pxv(c.margin.bottom) + pxv(b.top) + pxv(b.bottom)
            + pxv(c.padding.top) + pxv(c.padding.bottom);
        c.height = Some(Len::Px((h - extra).max(0.0)));
    }
}
