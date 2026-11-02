//! Единицы lh/rlh в длинах элемента (css-values-4 §6.1.4): разрешаются после каскада по высоте
//! строки родителя или своей.

use super::*;

pub(super) fn inherit_lh_units(
    parent: &Computed,
    c: &mut Computed,
    vcx: crate::values::ValueCx<'_>,
) {
    // css-values-4 §6.1.4, оговорка про font-affecting properties:
    // «when ''lh'' or ''rlh'' units are used in the value of the
    // 'line-height' property or font-affecting properties on the element
    // they refer to, they resolve against the computed 'line-height' and
    // font metrics of the PARENT element». То есть `line-height: 2lh` и
    // `font-size: 2lh` меряются строкой РОДИТЕЛЯ, а не своей: иначе
    // выходит круговая зависимость. Для всего остального (`height: 1lh`)
    // базой остаётся своя строка — та же оговорка, скобка в конце абзаца.
    let parent_font = match parent.inh.font.font_size {
        Some(crate::style::values::value::Len::Px(v)) => v,
        _ => 16.0,
    };
    let parent_family = parent.inh.font.font_family.clone().unwrap_or_else(|| {
        if parent.inh.font.monospace == Some(true) {
            vcx.doc
                .font_snapshot()
                .mono_family(parent.inh.text_flow.lang.as_deref())
                .to_string()
        } else {
            String::new()
        }
    });
    let parent_line = match parent.inh.font.line_height {
        Some(crate::style::values::value::Len::Px(v)) => v,
        Some(crate::style::values::value::Len::Em(k))
        | Some(crate::style::values::value::Len::Pct(k)) => k * parent_font,
        _ => crate::style::font_metrics::normal_height(&parent_family, parent_font, vcx),
    };
    // `c` — это `own.clone()`: слияние с родителем ещё впереди, значит в
    // `c.line_height`/`c.font_size` лежит ровно то, что задано НА ЭТОМ
    // элементе. Незаданное поле — `None`, и ветка молчит.
    let from_parent = |l: &mut Option<crate::style::values::value::Len>| match *l {
        Some(crate::style::values::value::Len::Lh(k)) => {
            *l = Some(crate::style::values::value::Len::Px(k * parent_line))
        }
        Some(crate::style::values::value::Len::LhPx(k, add)) => {
            *l = Some(crate::style::values::value::Len::Px(k * parent_line + add))
        }
        _ => {}
    };
    from_parent(&mut c.inh.font.line_height);
    from_parent(&mut c.inh.font.font_size);
    // `lh` — вычисленный `line-height` САМОГО элемента, а `line-height`
    // (как кегль и гарнитура) НАСЛЕДУЕТСЯ: незаданное на элементе берётся у
    // родителя. Ребёнок `height: 3lh` внутри `font: 16px / 32px` обязан
    // выйти 96, а не 3 × normal(16) ≈ 55 (`line-clamp-auto-035`: блок
    // с `height: 3lh` не доставал до потолка и не прятался).
    let font = match c.inh.font.font_size {
        Some(crate::style::values::value::Len::Px(v)) => v,
        None => parent_font,
        _ => 16.0,
    };
    // `line-height: normal` — доля кегля ПО МЕТРИКАМ шрифта, а не
    // постоянные 1.2: у `lh`-единицы иначе выходила чужая высота строки
    // (`line-clamp-auto-*` меряют высоту в `lh`).
    let family = c
        .inh
        .font
        .font_family
        .clone()
        .or_else(|| parent.inh.font.font_family.clone())
        .unwrap_or_else(|| {
            if c.inh.font.monospace.or(parent.inh.font.monospace) == Some(true) {
                vcx.doc
                    .font_snapshot()
                    .mono_family(c.inh.text_flow.lang.as_deref())
                    .to_string()
            } else {
                String::new()
            }
        });
    let line = match c.inh.font.line_height.or(parent.inh.font.line_height) {
        Some(crate::style::values::value::Len::Px(v)) => v,
        Some(crate::style::values::value::Len::Em(k))
        | Some(crate::style::values::value::Len::Pct(k)) => k * font,
        _ => crate::style::font_metrics::normal_height(&family, font, vcx),
    };
    let fix = |l: &mut Option<crate::style::values::value::Len>| match *l {
        Some(crate::style::values::value::Len::Lh(k)) => {
            *l = Some(crate::style::values::value::Len::Px(k * line))
        }
        Some(crate::style::values::value::Len::LhPx(k, add)) => {
            *l = Some(crate::style::values::value::Len::Px(k * line + add))
        }
        _ => {}
    };
    fix(&mut c.reset_box_layout.width);
    fix(&mut c.reset_box_layout.height);
    fix(&mut c.reset_box_layout.min_width);
    fix(&mut c.reset_box_layout.min_height);
    fix(&mut c.reset_box_layout.max_width);
    fix(&mut c.reset_box_layout.max_height);
    for sides in {
        let group_0 = &mut *c.reset_box_layout;
        [&mut group_0.snap.padding, &mut group_0.snap.margin]
    } {
        for value in [
            &mut sides.top,
            &mut sides.right,
            &mut sides.bottom,
            &mut sides.left,
        ] {
            fix(value);
        }
    }
    // `background-size: 100px 1lh` — та же единица и та же база
    // (`lh-unit-same-element-*`). Чинится ЗДЕСЬ, а не в `background.rs`:
    // там доступен только запасной кегль 16 (`FontCx::FALLBACK` даёт
    // 1.2 × 16 = 19.2 — ровно то, что видно на снимке), а сплошной
    // перевод шрифтовых единиц в самом `background.rs` ломает другие пары.
    // `em` in `background-size`/`background-position` is the element's own
    // computed font size (css-values-4 §6.1.1); the painter only knows a
    // 16px fallback (`table-cell-align-002`: `.5em .5em` at 20px).
    // Only with a known own font size: a still relative `font-size`
    // (`1.25em`, resolved later in `inherit_stage`) would read as the 16px
    // fallback here (`multicol-fill-auto-003`); the painter then resolves
    // against the computed size as before.
    let font_known = matches!(
        c.inh.font.font_size,
        None | Some(crate::style::values::value::Len::Px(_))
    );
    let em = |l: &mut Option<crate::style::values::value::Len>| {
        if !font_known {
            return;
        }
        if let Some(crate::style::values::value::Len::Em(k)) = *l {
            *l = Some(crate::style::values::value::Len::Px(k * font));
        }
    };
    if let crate::style::computed::BgSize::Fixed(w, h) = &mut c.reset_background.bg_size {
        fix(w);
        fix(h);
        em(w);
        em(h);
    }
    em(&mut c.reset_background.bg_pos.x);
    em(&mut c.reset_background.bg_pos.y);
}
