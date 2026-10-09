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

use crate::style::computed::{Computed, Shadow, Sides};
use crate::dom::Node;
use crate::style::values::value::Len;

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

fn scale_shadow(sh: &mut Shadow, k: f32) {
    sh.x *= k;
    sh.y *= k;
    sh.blur *= k;
    sh.spread *= k;
}

/// Собственные длины в точках × `k`. Доли, `auto`, единицы окна, calc-арена
/// и единицы шрифта не трогаются (§493; шрифтовые считаются от уже
/// домноженного кегля).
fn scale_own(c: &mut Computed, k: f32) {
    let mul = |l: &mut Option<Len>| match *l {
        Some(Len::Px(v)) => *l = Some(Len::Px(v * k)),
        Some(Len::EmPx(e, add)) => *l = Some(Len::EmPx(e, add * k)),
        Some(Len::LhPx(e, add)) => *l = Some(Len::LhPx(e, add * k)),
        _ => {}
    };
    let sides = |s: &mut Sides| {
        for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
            mul(one);
        }
    };
    for l in [
        &mut c.width,
        &mut c.height,
        &mut c.min_width,
        &mut c.min_height,
        &mut c.max_width,
        &mut c.max_height,
        &mut c.flex_basis,
        &mut c.letter_spacing,
        &mut c.word_spacing,
        &mut c.text_indent,
        &mut c.line_height,
        &mut c.tab_size_len,
        &mut c.column_width,
        &mut c.column_gap,
        &mut c.column_rule_width,
        &mut c.row_rule_width,
        &mut c.shape_margin,
    ] {
        mul(l);
    }
    sides(&mut c.padding);
    sides(&mut c.margin);
    sides(&mut c.border_width);
    sides(&mut c.inset);
    for corner in [&mut c.radius.tl, &mut c.radius.tr, &mut c.radius.br, &mut c.radius.bl] {
        mul(corner);
    }
    for pair in [c.gap.as_mut(), c.border_spacing.as_mut()].into_iter().flatten() {
        mul(&mut pair.0);
        mul(&mut pair.1);
    }
    if let Some(o) = c.outline.as_mut() {
        mul(&mut o.width);
        mul(&mut o.offset);
    }
    // `background-size` в точках — тоже длина (§493; `zoom/background-size`);
    // `cover`/`contain` и доли зуму безразличны.
    if let crate::style::computed::BgSize::Fixed(w, h) = &mut c.bg_size {
        mul(w);
        mul(h);
    }
    // `contain-intrinsic-*` хранится точками (`zoom/contain-intrinsic-height`,
    // `-width`: `10rem` при кегле корня 1px и `zoom: 10` — сторона 100).
    c.contain_intrinsic.0 = c.contain_intrinsic.0.map(|v| v * k);
    c.contain_intrinsic.1 = c.contain_intrinsic.1.map(|v| v * k);
    if let Some((x, y)) = c.translate.as_mut() {
        for t in [x, y] {
            if let Len::Px(v) = *t {
                *t = Len::Px(v * k);
            }
        }
    }
    // Длины ВНУТРИ `transform` (css-viewport-1 §493: домножается
    // ИСПОЛЬЗОВАННОЕ значение любого свойства, а сдвиг матрицы — длина).
    // Домножается только столбец сдвига и его плоский двойник `tr[i][0]`:
    // линейная часть (`lin`, поворот, масштаб, скос) безразмерна, а доли
    // собственного размера (`translate_pct`, `m4_pct`) уже считаются от
    // домноженной коробки — их множитель лёг бы вторым.
    if let Some(t) = c.transform.as_mut() {
        t.translate.0 *= k;
        t.translate.1 *= k;
        for row in t.tr.iter_mut() {
            row[0] *= k;
        }
        for row in t.m4.iter_mut().take(3) {
            row[3] *= k;
        }
        // `perspective(d)` живёт одной ячейкой m34 = −1/d
        // (`Computed::perspective4`): расстояние домножается — ячейка делится.
        if t.m4[3][2] != 0.0 {
            t.m4[3][2] /= k;
        }
    }
    // `perspective` и точки отсчёта преобразования — тоже длины.
    c.perspective = c.perspective.map(|d| d * k);
    c.transform_origin_px.0 = c.transform_origin_px.0.map(|v| v * k);
    c.transform_origin_px.1 = c.transform_origin_px.1.map(|v| v * k);
    c.transform_origin_z = c.transform_origin_z.map(|v| v * k);
    c.perspective_origin_px.0 = c.perspective_origin_px.0.map(|v| v * k);
    c.perspective_origin_px.1 = c.perspective_origin_px.1.map(|v| v * k);
    for sh in c
        .shadows
        .iter_mut()
        .chain(c.inset_shadows.iter_mut())
        .chain(c.text_shadow.iter_mut())
        .chain(c.text_shadow_rest.iter_mut())
    {
        scale_shadow(sh, k);
    }
    // `vertical-align: 20px` хранится точками со знаком — тоже длина.
    c.vertical_shift_px = c.vertical_shift_px.map(|v| v * k);
    // Длины украшений текста (css-text-decor-4): толщина, смещение
    // подчёркивания, отступы концов.
    let dl = |l: &mut crate::style::computed::DecorLen| {
        match l {
            crate::style::computed::DecorLen::Px(v) | crate::style::computed::DecorLen::Mix(_, v) => *v *= k,
            _ => {}
        }
    };
    if let Some(t) = c.td_thickness.as_mut() {
        dl(t);
    }
    if let Some(o) = c.underline_offset.as_mut() {
        dl(o);
    }
    if let Some(Some(pair)) = c.td_inset.as_mut() {
        pair.iter_mut().for_each(dl);
    }
}

/// Наследуемые длины в точках, у элемента не заданные: своё значение —
/// точки предка (уже с его зумом) × свой множитель.
fn inherited(c: &mut Computed, own: f32, carried: &Carried) {
    let put = |slot: &mut Option<Len>, from: Option<f32>| {
        if slot.is_none() && let Some(v) = from {
            *slot = Some(Len::Px(v * own));
        }
    };
    put(&mut c.line_height, carried.line_height);
    put(&mut c.letter_spacing, carried.letter_spacing);
    put(&mut c.word_spacing, carried.word_spacing);
    put(&mut c.text_indent, carried.text_indent);
    put(&mut c.tab_size_len, carried.tab_size);
    if c.text_shadow.is_none() && let Some(mut sh) = carried.text_shadow {
        scale_shadow(&mut sh, own);
        c.text_shadow = Some(sh);
    }
    if c.underline_offset.is_none() && let Some(v) = carried.underline_offset {
        c.underline_offset = Some(crate::style::computed::DecorLen::Px(v * own));
    }
    if c.border_spacing.is_none() && let Some((row, col)) = carried.border_spacing {
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
        if flags[i] && let Some(Len::Px(v)) = side(from, i) {
            set_side(into, i, Len::Px(v * k));
            flags[i] = false;
        }
    }
}

/// Явное `inherit` на ненаследуемых размерах и краях (`explicit-inherit/*`).
/// Родитель здесь — его СОБСТВЕННЫЙ стиль; для ненаследуемых свойств он и
/// есть вычисленный, и уже домножен на зум родителя.
fn explicit(c: &mut Computed, parent: Option<&Computed>, own: f32) {
    let Some(p) = parent else { return };
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(Len::Px(v * own)),
        _ => None,
    };
    if c.width_inherit && let Some(v) = px(p.width) {
        c.width = Some(v);
        c.width_inherit = false;
    }
    if c.height_inherit && let Some(v) = px(p.height) {
        c.height = Some(v);
        c.height_inherit = false;
    }
    // Порядок разрядов — как в `inline::inherit`: min-w, min-h, max-w, max-h.
    let minmax = [p.min_width, p.min_height, p.max_width, p.max_height];
    for (i, from) in minmax.into_iter().enumerate() {
        if c.minmax_inherit[i] && let Some(v) = px(from) {
            match i {
                0 => c.min_width = Some(v),
                1 => c.min_height = Some(v),
                2 => c.max_width = Some(v),
                _ => c.max_height = Some(v),
            }
            c.minmax_inherit[i] = false;
        }
    }
    // `padding: inherit` целиком: снимается, только если все стороны родителя
    // в точках или пусты — иначе слияние копирует весь набор, как прежде.
    if c.padding_inherit
        && [p.padding.top, p.padding.right, p.padding.bottom, p.padding.left]
            .iter()
            .all(|l| matches!(l, None | Some(Len::Px(_))))
    {
        c.padding = Sides {
            top: px(p.padding.top),
            right: px(p.padding.right),
            bottom: px(p.padding.bottom),
            left: px(p.padding.left),
        };
        c.padding_inherit = false;
    }
    per_side(&mut c.padding_inherit_side, &mut c.padding, &p.padding, own);
    per_side(&mut c.margin_inherit, &mut c.margin, &p.margin, own);
    per_side(&mut c.inset_inherit, &mut c.inset, &p.inset, own);
    per_side(&mut c.border_inherit_w, &mut c.border_width, &p.border_width, own);
    // `outline-width`/`outline-offset: inherit` и `background-size: inherit`
    // живут разрядами `inherit_bits`, и слияние (`inline::inherit`) копирует
    // точки родителя БЕЗ своего множителя (`zoom/outline-width`,
    // `outline-offset`, `background-size`: вторая коробка группы с зумом 2).
    // Значение родителя — его собственный стиль, уже с его зумом, × `own`;
    // разряд снимается. Не в точках — разряд остаётся, как прежде.
    use crate::style::computed::inh;
    if c.inherit_bits & (inh::OUTLINE_W | inh::OUTLINE_O) != 0 {
        let from = p.outline.unwrap_or_default();
        let mut o = c.outline.unwrap_or_default();
        let mut hit = false;
        if c.inherit_bits & inh::OUTLINE_W != 0 && let Some(v) = px(from.width) {
            o.width = Some(v);
            c.inherit_bits &= !inh::OUTLINE_W;
            hit = true;
        }
        if c.inherit_bits & inh::OUTLINE_O != 0 && let Some(v) = px(from.offset) {
            o.offset = Some(v);
            c.inherit_bits &= !inh::OUTLINE_O;
            hit = true;
        }
        if hit {
            c.outline = Some(o);
        }
    }
    if c.inherit_bits & inh::BG_SIZE != 0
        && let crate::style::computed::BgSize::Fixed(w, h) = p.bg_size
    {
        let k = |l: Option<Len>| px(l).or(l);
        c.bg_size = crate::style::computed::BgSize::Fixed(k(w), k(h));
        c.inherit_bits &= !inh::BG_SIZE;
    }
}

/// Запомнить для потомков собственные наследуемые длины в точках. Заданное
/// не в точках СБРАСЫВАЕТ запомненное: потомок унаследует не точки, и
/// домножать нечего (число `line-height`, `em` — считаются от его кегля).
fn remember(c: &Computed, k: &mut Carried) {
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let keep = |slot: &mut Option<f32>, l: Option<Len>| {
        if l.is_some() {
            *slot = px(l);
        }
    };
    keep(&mut k.line_height, c.line_height);
    keep(&mut k.letter_spacing, c.letter_spacing);
    keep(&mut k.word_spacing, c.word_spacing);
    keep(&mut k.text_indent, c.text_indent);
    keep(&mut k.tab_size, c.tab_size_len);
    if c.text_shadow.is_some() {
        k.text_shadow = c.text_shadow;
    }
    if let Some((row, col)) = c.border_spacing {
        k.border_spacing = Some((px(row), px(col)));
    }
    match c.underline_offset {
        Some(crate::style::computed::DecorLen::Px(v)) => k.underline_offset = Some(v),
        Some(_) => k.underline_offset = None,
        None => {}
    }
}
