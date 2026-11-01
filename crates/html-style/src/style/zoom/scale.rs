//! Масштаб zoom собственных длин элемента: тени, размеры, поля, рамки, шрифт.

use crate::style::computed::{Computed, Shadow, Sides};
use crate::style::values::value::Len;

mod rule_widths;
mod shape_lengths;
mod structured;

/// The absolute part of a `calc()` sum in the element's zoom. Font-relative terms
/// follow the zoomed font size, viewport terms the zoomed viewport
/// (`ViewportCx::for_box`), percentages their basis.
fn scale_calc(
    i: crate::style::values::value::CalcId,
    k: f32,
    vcx: crate::values::ValueCx<'_>,
) -> Len {
    let mut sum = crate::style::values::value::calc_get(i, vcx);
    if sum.px == 0.0 {
        return Len::Calc(i);
    }
    sum.px *= k;
    Len::Calc(crate::style::values::value::calc_store(sum, vcx))
}

pub(super) fn scale_shadow(sh: &mut Shadow, k: f32) {
    sh.x *= k;
    sh.y *= k;
    sh.blur *= k;
    sh.spread *= k;
}

/// Собственные длины в точках × `k`. Доли, `auto`, единицы окна, calc-арена
/// и единицы шрифта не трогаются (§493; шрифтовые считаются от уже
/// домноженного кегля).
pub fn scale_own(c: &mut Computed, k: f32, vcx: crate::values::ValueCx<'_>) {
    rule_widths::scale(c, k);
    let mul = |l: &mut Option<Len>| match *l {
        Some(Len::Px(v)) => *l = Some(Len::Px(v * k)),
        Some(Len::EmPx(e, add)) => *l = Some(Len::EmPx(e, add * k)),
        Some(Len::LhPx(e, add)) => *l = Some(Len::LhPx(e, add * k)),
        Some(Len::Calc(i)) => *l = Some(scale_calc(i, k, vcx)),
        _ => {}
    };
    let sides = |s: &mut Sides| {
        for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
            mul(one);
        }
    };
    for l in {
        let group_0 = &mut *c.reset_box_layout;
        let group_1 = &mut *c.reset_fragmentation;
        [
            &mut group_0.width,
            &mut group_0.height,
            &mut group_0.min_width,
            &mut group_0.min_height,
            &mut group_0.max_width,
            &mut group_0.max_height,
            &mut c.reset_flex.flex_basis,
            &mut group_1.column_width,
            &mut group_1.column_gap,
            &mut c.reset_shapes.shape_margin,
        ]
    } {
        mul(l);
    }
    let inh = &mut *c.inh;
    for l in {
        let group_0 = &mut *inh.text_flow;
        [
            &mut group_0.letter_spacing,
            &mut group_0.word_spacing,
            &mut group_0.text_indent,
            &mut inh.font.line_height,
            &mut group_0.tab_size_len,
        ]
    } {
        mul(l);
    }
    sides(&mut c.reset_box_layout.padding);
    sides(&mut c.reset_box_layout.margin);
    sides(&mut c.reset_border.border_width);
    sides(&mut c.reset_positioned.inset);
    for corner in {
        let group_0 = &mut *c.reset_border;
        [
            &mut group_0.radius.tl,
            &mut group_0.radius.tr,
            &mut group_0.radius.br,
            &mut group_0.radius.bl,
        ]
    } {
        mul(corner);
    }
    if let Some(pair) = c.reset_flex.gap.as_mut() {
        mul(&mut pair.0);
        mul(&mut pair.1);
    }
    if c.inh.table_list.border_spacing.is_some()
        && let Some(pair) = c.inh.table_list.border_spacing.as_mut()
    {
        mul(&mut pair.0);
        mul(&mut pair.1);
    }
    if let Some(o) = c.reset_border.outline.as_mut() {
        mul(&mut o.width);
        mul(&mut o.offset);
    }
    // `background-size` в точках — тоже длина (§493; `zoom/background-size`);
    // `cover`/`contain` и доли зуму безразличны.
    if let crate::style::computed::BgSize::Fixed(w, h) = &mut c.reset_background.bg_size {
        mul(w);
        mul(h);
    }
    // `contain-intrinsic-*` хранится точками (`zoom/contain-intrinsic-height`,
    // `-width`: `10rem` при кегле корня 1px и `zoom: 10` — сторона 100).
    c.reset_context.contain_intrinsic.0 = c.reset_context.contain_intrinsic.0.map(|v| v * k);
    c.reset_context.contain_intrinsic.1 = c.reset_context.contain_intrinsic.1.map(|v| v * k);
    if let Some((x, y)) = c.reset_transforms.translate.as_mut() {
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
    if let Some(t) = c.reset_transforms.transform.as_mut() {
        scale_transform(t, k);
    }
    // `perspective` и точки отсчёта преобразования — тоже длины.
    c.reset_transforms.perspective = c.reset_transforms.perspective.map(|d| d * k);
    c.reset_transforms.transform_origin_px.0 =
        c.reset_transforms.transform_origin_px.0.map(|v| v * k);
    c.reset_transforms.transform_origin_px.1 =
        c.reset_transforms.transform_origin_px.1.map(|v| v * k);
    c.reset_transforms.transform_origin_z = c.reset_transforms.transform_origin_z.map(|v| v * k);
    c.reset_transforms.perspective_origin_px.0 =
        c.reset_transforms.perspective_origin_px.0.map(|v| v * k);
    c.reset_transforms.perspective_origin_px.1 =
        c.reset_transforms.perspective_origin_px.1.map(|v| v * k);
    for sh in {
        let effects = &mut *c.reset_effects;
        effects
            .shadows
            .iter_mut()
            .chain(effects.inset_shadows.iter_mut())
    } {
        scale_shadow(sh, k);
    }
    if c.inh.text_ink.text_shadow.is_some() || !c.inh.text_ink.text_shadow_rest.is_empty() {
        let ink = &mut *c.inh.text_ink;
        for sh in ink
            .text_shadow
            .iter_mut()
            .chain(ink.text_shadow_rest.iter_mut())
        {
            scale_shadow(sh, k);
        }
    }
    // `vertical-align: 20px` хранится точками со знаком — тоже длина.
    c.reset_text_box.vertical_shift_px = c.reset_text_box.vertical_shift_px.map(|v| v * k);
    structured::scale(c, k);
    if let Some(s) = c.reset_shapes.shape_outside.as_mut() {
        *s = shape_lengths::scale_px(s, k);
    }
    // Длины украшений текста (css-text-decor-4): толщина, смещение
    // подчёркивания, отступы концов.
    let dl = |l: &mut crate::style::computed::DecorLen| match l {
        crate::style::computed::DecorLen::Px(v) | crate::style::computed::DecorLen::Mix(_, v) => {
            *v *= k
        }
        _ => {}
    };
    if let Some(t) = c.reset_text_box.td_thickness.as_mut() {
        dl(t);
    }
    if let Some(o) = c.inh.text_ink.underline_offset.as_mut() {
        dl(o);
    }
    if let Some(Some(pair)) = c.reset_text_box.td_inset.as_mut() {
        pair.iter_mut().for_each(dl);
    }
}

/// Длины внутри `transform` под множитель `k`: столбец сдвига ×k, строка w
/// (перспектива) ÷k — подобие S·M·S⁻¹, S = diag(k, k, k, 1). Нужна и там, где
/// `transform: inherit` берёт значение родителя уже после прохода зума
/// (`paint::effects::transform`), с `k = eff_ребёнка / eff_родителя`.
pub fn scale_transform(t: &mut crate::style::computed::Transform, k: f32) {
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
    // Вся строка w (не только m34): `perspective(d) rotateY(a)` кладёт
    // долю перспективы и в m41 — подобие S·M·S⁻¹, S = diag(k, k, k, 1).
    for v in t.m4[3].iter_mut().take(3) {
        *v /= k;
    }
}
