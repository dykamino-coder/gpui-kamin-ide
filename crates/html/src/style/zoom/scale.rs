//! Масштаб zoom собственных длин элемента: тени, размеры, поля, рамки, шрифт.

use super::*;

pub(super) fn scale_shadow(sh: &mut Shadow, k: f32) {
    sh.x *= k;
    sh.y *= k;
    sh.blur *= k;
    sh.spread *= k;
}

/// Собственные длины в точках × `k`. Доли, `auto`, единицы окна, calc-арена
/// и единицы шрифта не трогаются (§493; шрифтовые считаются от уже
/// домноженного кегля).
pub(super) fn scale_own(c: &mut Computed, k: f32) {
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
    for corner in [
        &mut c.radius.tl,
        &mut c.radius.tr,
        &mut c.radius.br,
        &mut c.radius.bl,
    ] {
        mul(corner);
    }
    for pair in [c.gap.as_mut(), c.border_spacing.as_mut()]
        .into_iter()
        .flatten()
    {
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
    let dl = |l: &mut crate::style::computed::DecorLen| match l {
        crate::style::computed::DecorLen::Px(v) | crate::style::computed::DecorLen::Mix(_, v) => {
            *v *= k
        }
        _ => {}
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
