//! Цикл по детям блока: строчные прогоны, блочные дети, позиционированные (blocks_flow).

mod inline_node;
pub(super) use inline_node::inline_node;

mod element;
mod trim;
pub(super) use element::paint_element;

use crate::dom::Node;
use crate::paint::effects::containment_paint;
use crate::render::*;
use crate::style::computed::Computed;
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(super) fn blocks_flow(
    nodes: &[Node],
    run_breaks: Vec<usize>,
    mut pending: Vec<Node>,
    mut out: Vec<AnyElement>,
    mut letter_scope: crate::render::first_letter_scope::Scope,
    inherited: &Computed,
    opts: &RenderOpts,
    ordered_context: bool,
    under_tf: bool,
    frame: std::rc::Rc<std::cell::Cell<crate::interactive::sticky::element::StickyFrame>>,
    below_run_end: usize,
    below_run_start: usize,
    below_zs: Vec<i32>,
) -> Vec<AnyElement> {
    // Запросы среза строк (`text::line_trim`): свой контейнер адресует их
    // владельцам строк до раскладки детей, а сам забирает адресованные ему.
    let trim_req = trim::register(nodes, inherited, opts, ordered_context);
    let trim_own = crate::text::line_trim::take(inherited.self_node);
    let owner = inherited.self_node;
    let mut first_para = true;
    let mut below = BelowRun {
        start: below_run_start,
        end: below_run_end,
        zs: below_zs,
    };
    for (idx, n) in nodes.iter().enumerate() {
        if run_breaks.contains(&idx) && !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            let t = trim::para_flags(trim_own, std::mem::take(&mut first_para), false);
            out.push(paint_inline_step7(crate::text::line_trim::scoped(
                owner,
                t,
                || letter_scope.paragraph(&taken, inherited, opts),
            )));
        }
        let is_inline = inline_node(n, inherited, ordered_context, &pending);
        if is_inline {
            pending.push(n.clone());
            continue;
        }
        if !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            let t = trim::para_flags(
                trim_own,
                std::mem::take(&mut first_para),
                trim::rest_is_empty(&nodes[idx..]),
            );
            out.push(paint_inline_step7(crate::text::line_trim::scoped(
                owner,
                t,
                || letter_scope.paragraph(&taken, inherited, opts),
            )));
        }
        // Позиционированные с `z-index: auto` красятся В ПОРЯДКЕ ДЕРЕВА
        // (CSS 2.1 прил. E, шаг 8; Blink `paint_layer_paint_order_iterator.h`
        // — один список). Абсолют на статической позиции живёт в верхнем слое
        // (`late_push`), и тот выпускался только в конце контейнера — ПОВЕРХ
        // позиционированных соседей, идущих в дереве позже
        // (`position-sticky-stacking-context-002`: `#overlapped-red` накрывал
        // липкий и `relative`-брата). Перед таким соседом слой выпускается:
        // пустые заместители нулевой высоты раскладку не трогают, а щупы
        // накопленных абсолютов стоят раньше по списку — дырки к подготовке
        // их заместителей уже известны. Отрицательный `z-index` не трогаем:
        // у него своя сортировка прогона (`below_run_*`).
        if let Node::Element(e) = n
            && crate::layout::positioned::containing_block::late_pending()
            && !e.style.z_index.is_some_and(|z| z < 0)
            && matches!(
                e.style.position,
                Some(crate::style::computed::Position::Relative)
                    | Some(crate::style::computed::Position::Sticky)
            )
        {
            out.extend(crate::layout::positioned::containing_block::late_close());
            crate::layout::positioned::containing_block::late_open();
        }
        if let Node::Element(e) = n {
            paint_element(
                e,
                nodes,
                idx,
                inherited,
                opts,
                ordered_context,
                under_tf,
                &frame,
                &mut letter_scope,
                &mut out,
                &mut below,
            );
        }
    }
    if !pending.is_empty() {
        let t = trim::para_flags(trim_own, first_para, true);
        out.push(paint_inline_step7(crate::text::line_trim::scoped(
            owner,
            t,
            || letter_scope.paragraph(&pending, inherited, opts),
        )));
    }
    if (inherited.text_box_trim_start || inherited.text_box_trim_end)
        && !out.is_empty()
        && !ordered_context
    {
        trim::apply(&mut out, nodes, inherited, opts);
    }
    trim::release(trim_req);
    // Верхний слой: то, что обязано рисоваться поверх соседей, идёт последним
    // и возвращается на своё место замеренным сдвигом.
    out.extend(crate::layout::positioned::containing_block::late_close());
    containment_paint::collect(out, inherited)
}

pub(super) struct BelowRun {
    start: usize,
    end: usize,
    zs: Vec<i32>,
}
