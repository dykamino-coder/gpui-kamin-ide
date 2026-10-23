//! Параметры многоколоночной коробки перед выбором маршрута раскладки.

use crate::dom::Element;
use crate::layout::multicol::spanner::{has_deep_spanner, multicol_container};
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(crate) fn column_plan(e: &Element, merged: &Computed, opts: &RenderOpts) -> ColumnPlan {
    let used_gap = match merged.column_gap {
        Some(Len::Px(v)) => v,
        // Доля — от ширины содержимого коробки (css-align-3 §8.1), в
        // единицах раскладки 1/64 точки, как Blink `LayoutUnit`: иначе
        // `30%` от 50 даёт 15.0000006, и край колонки 25.4999 округлялся
        // не так, как у точного `15px` (`multicol-nested-033`).
        Some(Len::Pct(k)) if matches!(e.style.width, Some(Len::Px(_))) => {
            let w = match e.style.width {
                Some(Len::Px(w)) => w,
                _ => 0.0,
            };
            crate::layout::fragment::line_shape::layout_unit(k * w)
        }
        _ => match merged.font_size {
            Some(Len::Px(size)) => size,
            _ => opts.base_size(),
        },
    };
    // `column-*` — только у блочных контейнеров (css-multicol-1 §2):
    // сетка ими не режется (`grid-multicol-001`).
    let multicol = multicol_container(&e.style);
    // css-multicol-1 §3.4 шаги (05)-(07): N считается по ВЫЧИСЛЕННЫМ
    // 'column-width', 'column-gap' и используемой ширине коробки.
    // Пока брались заданные значения, `column-width: 6em` не
    // проходил гейт `Len::Px`, `count_from_width` был `None`,
    // `width_driven` — ложью, и многоколоночник не включался вовсе:
    // `multicol-width-001` рисовался одним абзацем в 30 знаков
    // вместо пяти колонок по шесть. Blink
    // абсолютизирует 'column-width' в `float` ещё на вычисленном
    // значении (`css_properties.json5:7513`
    // `ConvertComputedLength<float>`) и в `length_utils.cc:1311`
    // `ResolveUsedColumnCount` читает уже точки.
    // css-multicol-1 §cw: вычисленное значение сохраняется, используемое
    // ограничено снизу 1px, в том числе при расчёте числа колонок (§pseudo-algorithm).
    let column_width = merged
        .column_width
        .filter(|_| multicol)
        .map(|width| match width {
            Len::Px(w) => Len::Px(w.max(1.0)),
            other => other,
        });
    let column_count = merged.column_count.filter(|_| multicol);
    // Ось прогрессии колонок — СТРОЧНАЯ ось многоколоночника
    // (css-multicol-1 §2, `Overview.bs:375-379`: «The column boxes are
    // ordered in the inline base direction»). В вертикальном письме она
    // вертикальна, блочная — горизонтальна, у `vertical-rl` от ПРАВОГО
    // края (css-writing-modes-4 §3.1). `inline-size`/`block-size`
    // разложены в физические `height`/`width` ещё в каскаде, поэтому
    // строчный размер коробки здесь — `height`, блочный — `width`.
    // `direction: rtl` в вертикали (колонки снизу вверх) — прежним путём.
    let col_rl = merged.wm().block_rl();
    let col_axis = if merged.wm().rtl() {
        crate::layout::fragment::types::StackAxis::Horizontal
    } else {
        crate::layout::fragment::types::StackAxis::of(merged.wm())
    };
    let col_vert = col_axis.is_vertical();
    let col_inline_size = if col_vert {
        merged.height
    } else {
        merged.width
    };
    let count_from_width = match (column_width, col_inline_size) {
        (Some(Len::Px(w)), Some(Len::Px(box_w))) if w > 0.0 => {
            Some((((box_w + used_gap) / (w + used_gap)).floor().max(1.0)) as u16)
        }
        _ => None,
    };
    // Used column-count (css-multicol §3.4, как ResolveUsedColumnCount
    // в blink): заданы оба — МЕНЬШЕЕ из числа и «сколько влезает»;
    // только ширина — сколько влезает.
    let used_count = match (column_count, count_from_width) {
        (Some(c), Some(fw)) => Some(c.min(fw)),
        (Some(c), None) => Some(c),
        (None, fw) => fw,
    };
    let width_driven = column_count.is_none()
        && count_from_width.is_none()
        && matches!(column_width, Some(Len::Px(w)) if w > 0.0);
    // Заданный `column-height` без числа колонок — одна колонка, но с
    // рядами (`column-height-012`: `column-height:40px` и 80px
    // содержимого — два ряда по 40).
    let height_driven = e.style.column_height.is_some() && used_count.is_none_or(|n| n <= 1);
    // `column-count: 1` — тоже многоколоночник (css-multicol-1 §2; Blink
    // `ComputedStyle::SpecifiesColumns`: «!HasAutoColumnCount()»), и
    // спаннер в нём режет содержимое на ряды: ряд до спаннера — свой
    // контекст форматирования и держит свои флоаты
    // (`multicol-span-float-002`: `Pink` вставал между флоатами первой
    // строки), рамка предка режется по фрагментам
    // (`multicol-span-all-children-height-008`). Только при спаннере:
    // без него одна колонка по-прежнему рисуется обычным блоком
    // (переполнения одной колонки вбок здесь нет). Элемент списка — мимо:
    // сегментный путь маркер не рисует
    // (`multicol-span-all-list-item-001/002`).
    let lone_span = used_count == Some(1)
        && !height_driven
        && e.style.column_wrap.is_none()
        && e.list_item.is_none()
        && has_deep_spanner(e);
    ColumnPlan {
        used_gap,
        column_width,
        col_rl,
        col_axis,
        col_vert,
        col_inline_size,
        used_count,
        width_driven,
        height_driven,
        lone_span,
    }
}

pub(crate) struct ColumnPlan {
    pub(super) used_gap: f32,
    pub(super) column_width: Option<Len>,
    pub(super) col_rl: bool,
    pub(super) col_axis: crate::layout::fragment::types::StackAxis,
    pub(super) col_vert: bool,
    pub(super) col_inline_size: Option<Len>,
    pub(super) used_count: Option<u16>,
    pub(super) width_driven: bool,
    pub(super) height_driven: bool,
    pub(super) lone_span: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_column_width_uses_one_pixel_without_changing_computed_value() {
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800., 600.),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        for (css, expected) in [
            ("column-width:0", Len::Px(0.)),
            ("columns:0", Len::Px(0.)),
            ("column-width:0.5px", Len::Px(0.5)),
            ("column-width:0em", Len::Em(0.)),
            ("column-width:0.01em", Len::Em(0.01)),
        ] {
            let doc = crate::document::state::DocState::new(Default::default());
            let mut style = Computed::default();
            style.apply_decls_with_vars(
                &crate::style::css::parse_decls(&format!("width:50px; column-gap:0; {css}")),
                &Default::default(),
                doc.values(),
            );
            assert_eq!(style.column_width, Some(expected), "{css}");
            let e = Element::synthetic("div", style.clone(), Default::default(), vec![]);
            let merged =
                crate::style::cascade::inherit::inherit(&Computed::default(), &style, doc.values());
            let plan = column_plan(&e, &merged, &opts);
            assert_eq!(plan.column_width, Some(Len::Px(1.0)), "{css}");
            assert_eq!(plan.used_count, Some(50), "{css}");
            assert_eq!(style.column_width, Some(expected));
        }
    }
}
