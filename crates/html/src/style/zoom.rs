//! `zoom` (css-viewport-1 §zoom-property): длины элемента домножаются на его
//! действующий зум в стадии вычисленных значений
//! (`style::cascade::compute`), как Blink `CSSToLengthConversionData` с
//! множителем `Zoom()`.
//!
//! Спека, §476-493: «the used value of a CSS property … is pre-multiplied …
//! by the used value of 'zoom' for the element»; «nested values of 'zoom'
//! multiply»; доли и `auto` не трогаются; кегль домножается всегда.
//!
//! Арифметика (всё сходится к «длина в точках × действующий зум»):
//! * свои длины в точках × `eff` — они ещё не несут ни одного множителя;
//! * свой кегль в `em`/`%`/`ch`/`ex`/`ic` считается от кегля РОДИТЕЛЯ, а тот
//!   уже несёт `eff_родителя`, — домножается на СВОЙ множитель `own`;
//! * кегль не задан — наследуется домноженным: ставится `Em(own)`, стадия
//!   переведёт его в точки от родительского (`resolve_em`);
//! * наследуемые длины в точках (`line-height`, `letter-spacing`, …), у
//!   элемента не заданные, приходят от родителя уже с `eff_родителя` — им
//!   ставится собственное значение `px × own`;
//! * значение, которое берёт каскадное `inherit`, — вычисленное значение
//!   родителя (с `eff_родителя`) × `own` (`compute::zoom::parent_in_own_zoom`).
//!
//! Единицы окна домножаются при разрешении (`ViewportCx::for_box` — окно в
//! зуме коробки), точечная часть `calc()` — здесь.
//!
//! Что не домножается: природный размер картинок (`layout::replaced`,
//! `natural_ratio`), px внутри `shape-outside`/`clip-path` и SVG.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

mod carried;
mod scale;
use carried::Carried;
use scale::scale_shadow;
pub(crate) use scale::{scale_own, scale_transform};

/// Длины каскадного стиля `c` в зуме `eff` (собственный множитель `own`) под
/// вычисленным стилем родителя `parent`.
pub(crate) fn apply(
    c: &mut Computed,
    parent: Option<&Computed>,
    eff: f32,
    own: f32,
    vcx: crate::document::state::ValueCx<'_>,
) {
    // Кегль — первым и отдельно: от него стадия считает все `em` элемента,
    // и в `scale_own` он НЕ входит (иначе множитель лёг бы дважды).
    c.font_size = match c.font_size {
        Some(Len::Px(v)) => Some(Len::Px(v * eff)),
        Some(Len::Em(k)) => Some(Len::Em(k * own)),
        Some(Len::Pct(k)) => Some(Len::Pct(k * own)),
        Some(Len::Ch(k)) => Some(Len::Ch(k * own)),
        Some(Len::Ex(k)) => Some(Len::Ex(k * own)),
        Some(Len::Ic(k)) => Some(Len::Ic(k * own)),
        Some(Len::EmPx(k, add)) => Some(Len::EmPx(k * own, add * eff)),
        // `lh` в кегле берётся от строки РОДИТЕЛЯ (`from_parent`), а она уже
        // несёт зум предков — домножается только СВОЙ множитель, как у `em`.
        Some(Len::Lh(k)) => Some(Len::Lh(k * own)),
        Some(Len::LhPx(k, add)) => Some(Len::LhPx(k * own, add * eff)),
        None if (own - 1.0).abs() > f32::EPSILON => Some(Len::Em(own)),
        other => other,
    };
    scale_own(c, eff, vcx);
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
        inherited(c, own, &Carried::of(parent));
    }
}

/// Наследуемые длины в точках, у элемента не заданные: своё значение —
/// точки родителя (уже с его зумом) × свой множитель.
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
    // `text-shadow` is inherited as one list: the whole list is scaled.
    if c.text_shadow.is_none()
        && !c.text_shadow_none
        && let Some(mut sh) = carried.text_shadow.clone()
    {
        scale_shadow(&mut sh, own);
        c.text_shadow = Some(sh);
        c.text_shadow_rest = carried.text_shadow_rest.clone();
        for sh in &mut c.text_shadow_rest {
            scale_shadow(sh, own);
        }
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
