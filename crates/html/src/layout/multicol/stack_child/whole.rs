//! Дети копии и цельная коробка копии: вложенный ряд, неразрезаемый ребёнок, таблица.

use super::fixed::{drop_viewport_fixed, fixed_cb_box};
use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::nested_box_w;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::table::table;
use crate::paint::effects::transform::transformed;
use crate::render::{RenderOpts, element};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn copy_kids(e: &Element, copy: &Element, first: bool, src: &Element) -> Vec<Node> {
    let kids: Vec<Node> = if first {
        src.children.clone()
    } else {
        // Семя — только коробка многоколоночника
        // и корень копии. `e` считается наравне
        // с предками внутри копии: содержащий
        // блок `fixed` на самой коробке контекста
        // фрагментации — это Blink
        // `fixedpos_containing_block`
        // (`out_of_flow_layout_part.cc:1369`),
        // фрагментаинерный потомок, а не
        // повторяемая коробка. Предки ВЫШЕ `e`
        // не в счёт: их содержащий блок вне
        // контекста. `hoist_relative` выше
        // снимает лишь ВСТАВКИ,
        // `transform`/`contain` остаются на
        // месте — проверка по `copy.style`
        // законна.
        let fixed_cb_root = fixed_cb_box(&e.style) || fixed_cb_box(&copy.style);
        src.children
            .iter()
            .filter_map(|n| drop_viewport_fixed(n, fixed_cb_root))
            .collect()
    };
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
        // Своя метка узла на каждую копию: буфер линеек
        // промежутков (`gap_items_for` по `node_id`) у
        // копий одного узла сливался в один, и первая
        // копия забирала линейки всех рядов — во
        // втором и третьем ряду их не было
        // (`multicol-breaking-002`, 0.65).
        mc.node_id ^= (part as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        // Ширина `auto` — по колонке (CSS 2.1 §10.3.3):
        // копия кладётся корнем, и её многоколоночнику
        // нужна ширина в точках для меры строк.
        if matches!(mc.style.width, None | Some(Len::Auto))
            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
        {
            mc.style.width = Some(Len::Px(w));
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
