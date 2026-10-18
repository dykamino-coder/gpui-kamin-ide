//! Дети копии и цельная коробка копии: вложенный ряд, неразрезаемый ребёнок, таблица.

use super::fixed::{drop_uncontained_oof, drop_viewport_fixed, fixed_cb_box};
use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::nested_box_w;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::table::table;
use crate::paint::effects::transform::transformed;
use crate::render::{RenderOpts, element};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn copy_kids(copy: &Element, first: bool, src: &Element, band: bool) -> Vec<Node> {
    let kids: Vec<Node> = if first {
        src.children.clone()
    } else {
        // Семя — только корень копии. Содержащий блок `fixed` на самой
        // коробке многоколоночника забирает слой контейнера
        // (`cb_open_fixed_only`) из копии 0 — повтор в копиях ≥ 1 дал бы
        // столько коробок, сколько колонок. Предки ВЫШЕ `e` не в счёт: их
        // содержащий блок вне контекста. `hoist_relative` снимает лишь
        // ВСТАВКИ, `transform`/`contain` остаются — проверка по
        // `copy.style` законна.
        let fixed_cb_root = fixed_cb_box(&copy.style);
        src.children
            .iter()
            .filter_map(|n| drop_viewport_fixed(n, fixed_cb_root))
            .collect()
    };
    if band {
        let fixed = fixed_cb_box(&copy.style);
        let abs = fixed || crate::text::inline::establishes_cb(&copy.style);
        return kids
            .iter()
            .filter_map(|n| drop_uncontained_oof(n, abs, fixed))
            .collect();
    }
    kids
}

#[allow(clippy::too_many_arguments)]
pub(super) fn copy_whole_box(
    opts: &RenderOpts,
    line_col_w: Option<f32>,
    merged: &Computed,
    copy: &Element,
    h: f32,
    nest_row: Option<f32>,
    nest_phase_k: f32,
    inner: &Computed,
    whole: bool,
    part: usize,
    kids: &mut Vec<Node>,
    frag_gap_guard: &mut Option<crate::paint::gap_rules::GapGuard>,
) -> Option<gpui::AnyElement> {
    if let Some(hh) = nest_row {
        let mut mc = copy.clone();
        mc.children = std::mem::take(kids);
        // Своя метка узла на каждую копию: состояние,
        // ключённое по `node_id`, у копий одного узла
        // иначе сливается в одно (`multicol-breaking-002`).
        mc.node_id ^= (part as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        // Ширина `auto` — по колонке (CSS 2.1 §10.3.3):
        // копия кладётся корнем, и её многоколоночнику
        // нужна ширина в точках для меры строк.
        if matches!(mc.style.width, None | Some(Len::Auto))
            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
        {
            mc.style.width = Some(Len::Px(w));
        } else if let (Some(Len::Pct(k)), Some(cw)) = (mc.style.width, line_col_w) {
            // Доля — от внешней колонки (CSS 2.1 §10.2): в точках её ждёт
            // доля `column-gap` коробки (`column_plan`).
            mc.style.width = Some(Len::Px(crate::layout::fragment::line_shape::layout_unit(
                k * cw,
            )));
        }
        // `height: auto` — высота из меры рядами
        // (`nested_rows_shape`): стопка с рядами
        // отдаёт полный последний ряд, а коробка
        // кончается на сбалансированном хвосте.
        if matches!(mc.style.height, None | Some(Len::Auto)) {
            let b = mc.style.borders();
            let px = |l: &Option<Len>| match l {
                Some(Len::Px(v)) => *v,
                _ => 0.0,
            };
            let bot = px(&mc.style.padding.bottom) + px(&b.bottom);
            mc.style.height = Some(Len::Px((h - bot).max(0.0)));
            mc.style.border_box = None;
        }
        drop(frag_gap_guard.take());
        crate::layout::fragment::types::set_outer_row(Some((hh, nest_phase_k)));
        let el = element(&mc, merged, opts);
        crate::layout::fragment::types::set_outer_row(None);
        return Some(el);
    }
    if whole {
        let mut mc = copy.clone();
        mc.children = std::mem::take(kids);
        if matches!(mc.style.width, None | Some(Len::Auto))
            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
        {
            mc.style.width = Some(Len::Px(w));
        }
        drop(frag_gap_guard.take());
        return Some(element(&mc, merged, opts));
    }
    if table_box(copy) {
        let mut tc = copy.clone();
        tc.children = std::mem::take(kids);
        drop(frag_gap_guard.take());
        return Some(transformed(table(&tc, inner, opts), inner, merged));
    }
    None
}
