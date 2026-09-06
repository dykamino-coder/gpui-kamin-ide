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
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::computed::{Computed, Position, PositionAnchor};
use crate::dom::Node;
use crate::value::{AnchorFn, AnchorSide, Len, anchor_get};

thread_local! {
    /// Именованные якоря кадра: имя → рамка (border box). Повтор имени
    /// перезаписывает — «the last element in tree order» (§target anchor
    /// element, п.3); «ближайший предок» пока не отличается.
    static NAMED: RefCell<HashMap<String, Bounds<Pixels>>> = RefCell::new(HashMap::new());
    /// Неявные якоря: `node_id` порождающего элемента → его рамка (для
    /// псевдоэлементов с `position-anchor: auto`, §implicit).
    static IMPLICIT: RefCell<HashMap<u64, Bounds<Pixels>>> = RefCell::new(HashMap::new());
}

/// Расходник кадра — чистится в `interact::frame_sanitize`.
pub fn reset() {
    NAMED.with(|m| m.borrow_mut().clear());
    IMPLICIT.with(|m| m.borrow_mut().clear());
}

/// Проба якоря для коробки `e`: нужна, когда у неё есть `anchor-name` или
/// её псевдоэлемент ссылается на неё как на неявный якорь. Канвас во всю
/// коробку (`absolute` + `size_full`) — та же форма, что у `edge_probe`.
pub fn probe_for(e: &crate::dom::Element) -> Option<AnyElement> {
    let names: Vec<String> = e.style.anchor_name.clone().unwrap_or_default();
    let implicit = e.node_id != 0
        && e.children.iter().any(|n| {
            matches!(n, Node::Element(c)
                if c.tag.starts_with("::")
                    && c.style.position_anchor == Some(PositionAnchor::Auto))
        });
    if names.is_empty() && !implicit {
        return None;
    }
    let id = e.node_id;
    Some(
        gpui::canvas(
            move |bounds: Bounds<Pixels>, _, _| {
                NAMED.with(|m| {
                    let mut m = m.borrow_mut();
                    for n in &names {
                        m.insert(n.clone(), bounds);
                    }
                });
                if implicit {
                    IMPLICIT.with(|m| m.borrow_mut().insert(id, bounds));
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
                _ => false,
            };
            let s = &mut e.style.inset;
            for slot in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                *slot = settle_len(*slot, has_default, known);
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
    /// План есть только у абсолюта хотя бы с одной `anchor()`-вставкой.
    fn of(own: &Computed, inherited: &Computed) -> Option<AnchorPlan> {
        if !matches!(own.position, Some(Position::Absolute) | Some(Position::Fixed)) {
            return None;
        }
        let side = |l: Option<Len>, m: Option<Len>| -> Option<SidePlan> {
            let Some(Len::Anchor(i)) = l else { return None };
            Some(SidePlan {
                f: anchor_get(i)?,
                margin: match m {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                },
            })
        };
        let sides = [
            side(own.inset.top, own.margin.top),
            side(own.inset.right, own.margin.right),
            side(own.inset.bottom, own.margin.bottom),
            side(own.inset.left, own.margin.left),
        ];
        if sides.iter().all(Option::is_none) {
            return None;
        }
        let default_anchor = match &own.position_anchor {
            Some(PositionAnchor::Named(n)) => Some(DefaultAnchor::Named(n.clone())),
            Some(PositionAnchor::Auto) => own.implicit_anchor.map(DefaultAnchor::Implicit),
            _ => None,
        };
        Some(AnchorPlan {
            sides,
            default_anchor,
            cb_flipped: flipped(inherited),
            own_flipped: flipped(own),
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

    /// Сдвиг по оси стороны `k` (0 top, 1 right, 2 bottom, 3 left).
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
        let mut f = Some(sp.f.clone());
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
                (Some(want), _) => return want - own_edge,
                // Запасная длина в точках — вставка от края содержащего
                // блока; раскладка уже поставила коробку на нулевую вставку.
                (None, Some(Len::Px(v))) => return if end_side { -v } else { v },
                (None, Some(Len::Anchor(j))) => f = anchor_get(j),
                // Якоря нет и запаса нет: по спеке — статическая позиция,
                // но статически это было неотличимо (`settle_static` такие
                // уже снял); остаток — якорь, который есть в документе, но
                // ниже по дереву (`anchor-position-circular`). Коробка
                // остаётся у края содержащего блока.
                (None, _) => return 0.0,
            }
        }
        0.0
    }
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
        // По оси побеждает начальная сторона: `left` при заданных `left` и
        // `right` (растяжка между двумя якорями меняет РАЗМЕР — шаг 2).
        let dx = if self.plan.sides[3].is_some() {
            self.plan.shift(3, bounds)
        } else {
            self.plan.shift(1, bounds)
        };
        let dy = if self.plan.sides[0].is_some() {
            self.plan.shift(0, bounds)
        } else {
            self.plan.shift(2, bounds)
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
