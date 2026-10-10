//! `zoom` (css-viewport-1 §zoom-property) — отдельный проход после каскада.
//!
//! Спека, §476-493: «the used value of a CSS property … is pre-multiplied …
//! by the used value of 'zoom' for the element»; «nested values of 'zoom'
//! multiply»; доли и `auto` не трогаются; кегль домножается всегда. Blink
//! делает то же в момент вычисления длины (`CSSToLengthConversionData` с
//! множителем `Zoom()`), ДО раскладки.
//!
//! Проход идёт по СОБСТВЕННЫМ стилям дерева (`Element::style`, до слияния
//! `inline::inherit`) и пишет только в элементы, у которых действующий зум
//! или свой множитель отличны от единицы. Страница без `zoom` не заходит ни
//! в одну ветку записи: `zoom` у всех `None`, `eff` везде 1, работа прохода —
//! один обход дерева с копированием семи `Option<f32>`.
//!
//! Арифметика (всё сходится к «длина в точках × действующий зум»):
//! * свои длины в точках × `eff` — они ещё не несут ни одного множителя;
//! * свой кегль в `em`/`%`/`ch`/`ex`/`ic` считается от кегля РОДИТЕЛЯ, а тот
//!   уже несёт `eff_родителя`, — домножается на СВОЙ множитель `own`;
//! * кегль не задан — наследуется домноженным: ставится `Em(own)`, слияние
//!   переведёт его в точки от родительского (`resolve_em`);
//! * наследуемые длины в точках (`line-height`, `letter-spacing`, …), у
//!   элемента не заданные, придут от предка уже с `eff_родителя` — им
//!   дописывается собственное значение `px × own`;
//! * явное `inherit` на ненаследуемых размерах — значение родителя (свойство
//!   не наследуется, значит его собственное и есть вычисленное, уже с
//!   `eff_родителя`) × `own`, флаг снимается, слияние ничего не копирует.
//!
//! Что остаётся шагу 2: природный размер картинок (`render.rs`,
//! `natural_ratio`), `vw`/`vh` (`Computed::resolve_viewport`), px внутри
//! `shape-outside`/`clip-path`, `Calc(i)`, SVG, а также ЯВНОЕ наследование
//! `transform: inherit` / `perspective: inherit` — флагов `*_inherit` для
//! них в `Computed` пока нет, и `explicit` их не видит.

use crate::dom::Node;
use crate::style::computed::{Computed, Shadow, Sides};
use crate::style::values::value::Len;

mod explicit;
mod scale;
use explicit::{explicit, remember};
use scale::{scale_own, scale_shadow};

/// Наследуемые длины, которые проход несёт вниз В ТОЧКАХ: значение
/// ближайшего предка, задавшего свойство, уже домноженное на его зум.
/// Единицы шрифта не несём: слияние переведёт их от кегля потомка, а кегль
/// под зумом уже домножен.
#[derive(Clone, Copy, Default)]
struct Carried {
    line_height: Option<f32>,
    letter_spacing: Option<f32>,
    word_spacing: Option<f32>,
    text_indent: Option<f32>,
    tab_size: Option<f32>,
    text_shadow: Option<Shadow>,
    border_spacing: Option<(Option<f32>, Option<f32>)>,
    underline_offset: Option<f32>,
}

/// Проход по дереву: действующий зум — произведение по предкам.
pub fn resolve(nodes: &mut [Node]) {
    walk(nodes, None, 1.0, Carried::default());
}

fn walk(nodes: &mut [Node], parent: Option<&Computed>, parent_eff: f32, carried: Carried) {
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        let own = e.style.zoom.unwrap_or(1.0);
        let eff = parent_eff * own;
        let zoomed = (eff - 1.0).abs() > f32::EPSILON;
        // `zoom: 2` внутри `zoom: .5` даёт eff = 1, но унаследованные точки
        // всё равно надо домножить на свой множитель — гейт по обоим.
        if zoomed || (own - 1.0).abs() > f32::EPSILON {
            e.style.zoom_eff = zoomed.then_some(eff);
            apply(&mut e.style, parent, eff, own, &carried);
        }
        let mut below = carried;
        remember(&e.style, &mut below);
        walk(&mut e.children, Some(&e.style), eff, below);
    }
}

fn apply(c: &mut Computed, parent: Option<&Computed>, eff: f32, own: f32, carried: &Carried) {
    // Кегль — первым и отдельно: от него слияние считает все `em` элемента,
    // и в `scale_own` он НЕ входит (иначе множитель лёг бы дважды).
    c.font_size = match c.font_size {
        Some(Len::Px(v)) => Some(Len::Px(v * eff)),
        Some(Len::Em(k)) => Some(Len::Em(k * own)),
        Some(Len::Pct(k)) => Some(Len::Pct(k * own)),
        Some(Len::Ch(k)) => Some(Len::Ch(k * own)),
        Some(Len::Ex(k)) => Some(Len::Ex(k * own)),
        Some(Len::Ic(k)) => Some(Len::Ic(k * own)),
        Some(Len::EmPx(k, add)) => Some(Len::EmPx(k * own, add * eff)),
        // `lh` в кегле берётся от строки РОДИТЕЛЯ (`inline::inherit`,
        // `from_parent`), а она уже несёт зум предков — домножается только
        // СВОЙ множитель, как у `em`.
        Some(Len::Lh(k)) => Some(Len::Lh(k * own)),
        Some(Len::LhPx(k, add)) => Some(Len::LhPx(k * own, add * eff)),
        None if (own - 1.0).abs() > f32::EPSILON => Some(Len::Em(own)),
        other => other,
    };
    scale_own(c, eff);
    // То же у высоты строки: `line-height: 2lh` решается от строки родителя
    // и должен «still multiply by our own zoom» (relative-units-from-parent:
    // `zoom: 2; line-height: 2lh` ≡ `line-height: 4lh`). Точечная часть
    // `LhPx` уже домножена в `scale_own`.
    c.line_height = match c.line_height {
        Some(Len::Lh(k)) => Some(Len::Lh(k * own)),
        Some(Len::LhPx(k, add)) => Some(Len::LhPx(k * own, add)),
        other => other,
    };
    if (own - 1.0).abs() > f32::EPSILON {
        inherited(c, own, carried);
        explicit(c, parent, own);
    }
}

/// Наследуемые длины в точках, у элемента не заданные: своё значение —
/// точки предка (уже с его зумом) × свой множитель.
fn inherited(c: &mut Computed, own: f32, carried: &Carried) {
    let put = |slot: &mut Option<Len>, from: Option<f32>| {
        if slot.is_none()
            && let Some(v) = from
        {
            *slot = Some(Len::Px(v * own));
        }
    };
    put(&mut c.line_height, carried.line_height);
    put(&mut c.letter_spacing, carried.letter_spacing);
    put(&mut c.word_spacing, carried.word_spacing);
    put(&mut c.text_indent, carried.text_indent);
    put(&mut c.tab_size_len, carried.tab_size);
    if c.text_shadow.is_none()
        && let Some(mut sh) = carried.text_shadow
    {
        scale_shadow(&mut sh, own);
        c.text_shadow = Some(sh);
    }
    if c.underline_offset.is_none()
        && let Some(v) = carried.underline_offset
    {
        c.underline_offset = Some(crate::style::computed::DecorLen::Px(v * own));
    }
    if c.border_spacing.is_none()
        && let Some((row, col)) = carried.border_spacing
    {
        let px = |v: Option<f32>| v.map(|v| Len::Px(v * own));
        c.border_spacing = Some((px(row), px(col)));
    }
}

fn side(s: &Sides, i: usize) -> Option<Len> {
    match i {
        0 => s.top,
        1 => s.right,
        2 => s.bottom,
        _ => s.left,
    }
}

fn set_side(s: &mut Sides, i: usize, v: Len) {
    match i {
        0 => s.top = Some(v),
        1 => s.right = Some(v),
        2 => s.bottom = Some(v),
        _ => s.left = Some(v),
    }
}

/// Явное `inherit` по сторонам: сторона родителя в точках × `k`, флаг
/// снимается — слияние эту сторону уже не копирует. Не в точках — флаг
/// остаётся, слияние копирует как прежде (доля и `auto` зуму безразличны).
fn per_side(flags: &mut [bool; 4], into: &mut Sides, from: &Sides, k: f32) {
    for i in 0..4 {
        if flags[i]
            && let Some(Len::Px(v)) = side(from, i)
        {
            set_side(into, i, Len::Px(v * k));
            flags[i] = false;
        }
    }
}
