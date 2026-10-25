//! Дети копии и цельная коробка копии: вложенный ряд, неразрезаемый ребёнок, таблица.

use super::fixed::{drop_uncontained_oof, drop_viewport_fixed, fixed_cb_box};
use crate::dom::{Element, Node};
use crate::layout::build_contracts::Effect;
use crate::layout::fragment::line_shape::nested_box_w;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::options::RenderOpts;
use crate::layout::table::table;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

pub(super) fn copy_kids(copy: &Element, first: bool, src: &Element, band: bool) -> Vec<Node> {
    let kids: Vec<Node> = if first {
        src.children.to_vec()
    } else {
        // Семя — только корень копии. Содержащий блок `fixed` на самой
        // коробке многоколоночника забирает слой контейнера
        // (`cb_open_fixed_only`) из копии 0 — повтор в копиях ≥ 1 дал бы
        // столько коробок, сколько колонок. Предки ВЫШЕ `e` не в счёт: их
        // содержащий блок вне контекста. `hoist_relative` снимает лишь
        // ВСТАВКИ, `transform`/`contain` остаются — проверка по
        // `copy.style` законна.
        let fixed_cb_root = fixed_cb_box(&copy.style.used());
        src.children
            .iter()
            .filter_map(|n| drop_viewport_fixed(n, fixed_cb_root))
            .collect()
    };
    if band {
        let fixed = fixed_cb_box(&copy.style.used());
        let abs = fixed || crate::text::inline::establishes_cb(&copy.style.used());
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
    merged: &BoxStyle,
    copy: &Element,
    h: f32,
    nest_row: Option<f32>,
    nest_phase_k: f32,
    inner: &BoxStyle,
    whole: bool,
    part: usize,
    kids: &mut Vec<Node>,
) -> Option<gpui::AnyElement> {
    if let Some(hh) = nest_row {
        let mut mc = copy.clone();
        mc.children = std::mem::take(kids).into();
        // Своя метка узла на каждую копию: состояние,
        // ключённое по `node_id`, у копий одного узла
        // иначе сливается в одно (`multicol-breaking-002`).
        mc.node_id ^= (part as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        // Ширина `auto` — по колонке (CSS 2.1 §10.3.3):
        // копия кладётся корнем, и её многоколоночнику
        // нужна ширина в точках для меры строк.
        if matches!(mc.style.used().width, None | Some(Len::Auto))
            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
        {
            mc.style.used_mut().width = Some(Len::Px(w));
        } else if let (Some(Len::Pct(k)), Some(cw)) = (mc.style.used().width, line_col_w) {
            // Доля — от внешней колонки (CSS 2.1 §10.2): в точках её ждёт
            // доля `column-gap` коробки (`column_plan`).
            mc.style.used_mut().width = Some(Len::Px(
                crate::layout::fragment::line_shape::layout_unit(k * cw),
            ));
        }
        // `height: auto` — высота из меры рядами
        // (`nested_rows_shape`): стопка с рядами
        // отдаёт полный последний ряд, а коробка
        // кончается на сбалансированном хвосте.
        if matches!(mc.style.used().height, None | Some(Len::Auto)) {
            let b = mc.style.used().borders();
            let px = |l: &Option<Len>| crate::style::values::value::px_lossy_zero(*l);
            let bot = px(&mc.style.used().padding.bottom) + px(&b.bottom);
            mc.style.used_mut().height = Some(Len::Px((h - bot).max(0.0)));
            mc.style.used_mut().border_box = None;
        }
        return Some(opts.cx.contracts.build.element(
            &mc,
            merged,
            &opts.with_outer_row((hh, nest_phase_k)),
        ));
    }
    // css-lists-3 §3: маркер принадлежит коробке пункта. Сборка через
    // blocks + styled_div теряет его и меняет высоту строки у summary.
    // Общий рисователь сохраняет маркер; разрезы по-прежнему даёт рекордер.
    // CSS 2.1 §9.4.1: fieldset содержит независимый BFC. Обычный blocks
    // не создаёт его анонимную коробку и выпускает нижнее поле ребёнка наружу.
    if whole || copy.list_item.is_some() || copy.tag == "fieldset" {
        let mut mc = copy.clone();
        mc.children = std::mem::take(kids).into();
        if matches!(mc.style.used().width, None | Some(Len::Auto))
            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
        {
            mc.style.used_mut().width = Some(Len::Px(w));
        }
        return Some(opts.cx.contracts.build.element(&mc, merged, opts));
    }
    if table_box(copy) {
        let mut tc = copy.clone();
        tc.children = std::mem::take(kids).into();
        // Содержащий блок таблицы в колонке — колонка (css-multicol-1 §2):
        // от её ширины и доли размеров содержимого, а не от многоколоночника.
        let col = line_col_w.map(|w| opts.inner(|cx| cx.constraints.avail_w = Some(w)));
        let opts = col.as_deref().unwrap_or(opts);
        return Some(opts.cx.contracts.build.effects(
            table(&tc, inner, opts),
            Effect::transform(inner, merged, opts.cx.vcx()),
        ));
    }
    None
}
