//! Цикл по детям блока: строчные прогоны, блочные дети, позиционированные (blocks_flow).

use crate::layout::options::RenderOpts;
use crate::render::paragraph::route::paint_inline_step7;
use crate::style::computed::BoxStyle;
mod inline_node;
pub(super) use inline_node::inline_node;

mod element;
mod trim;
pub(super) use element::paint_element;

use crate::dom::Node;
use crate::paint::effects::containment_paint;
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(super) fn blocks_flow(
    nodes: &[Node],
    run_breaks: Vec<usize>,
    mut pending: Vec<Node>,
    mut out: Vec<AnyElement>,
    mut letter_scope: crate::render::first_letter_scope::Scope,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    ordered_context: bool,
    under_tf: bool,
    frame: std::rc::Rc<std::cell::Cell<crate::interactive::sticky::element::StickyFrame>>,
    below_run_end: usize,
    below_run_start: usize,
    below_zs: Vec<i32>,
) -> Vec<AnyElement> {
    let vcx = opts.cx.vcx();
    // Запросы среза строк (`text::line_trim`): свой контейнер адресует их
    // владельцам строк до раскладки детей, а сам забирает адресованные ему.
    let owner = inherited.self_node;
    let own_requests = trim::register(nodes, inherited, opts, ordered_context);
    let trim_own;
    let child_opts;
    let opts = if own_requests.is_empty()
        && !opts
            .cx
            .inline
            .trim_requests
            .iter()
            .any(|q| q.owner == owner)
    {
        trim_own = crate::text::line_trim::Trim::default();
        opts
    } else {
        let mut requests: Vec<_> = opts.cx.inline.trim_requests.iter().copied().collect();
        requests.extend(own_requests);
        trim_own = crate::text::line_trim::take(&requests, owner);
        requests.retain(|q| q.owner != owner);
        child_opts = opts.inner(|cx| cx.inline.trim_requests = std::rc::Rc::new(requests));
        &*child_opts
    };
    let mut late_layer =
        crate::layout::positioned::layer_scope::LayerScope::late(&opts.cx.position.state);
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
            out.push(paint_inline_step7(letter_scope.paragraph(
                &taken,
                inherited,
                &opts.inner(|cx| cx.inline.trim_para = Some((owner, t))),
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
            out.push(paint_inline_step7(letter_scope.paragraph(
                &taken,
                inherited,
                &opts.inner(|cx| cx.inline.trim_para = Some((owner, t))),
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
            && opts.cx.position.state.late_pending()
            && e.style.computed().z_index.is_none_or(|z| z >= 0)
            && matches!(
                e.style.computed().position,
                Some(crate::style::computed::Position::Relative)
                    | Some(crate::style::computed::Position::Sticky)
            )
        {
            out.extend(late_layer.finish());
            late_layer =
                crate::layout::positioned::layer_scope::LayerScope::late(&opts.cx.position.state);
        }
        if let Node::Element(e) = n {
            // Отложенный offset-трансформ абсолюта в начальном содержащем
            // блоке: размер окна известен только здесь (`motion::apply_icb`);
            // обёртку `transformed()` ниже строят от `e.style`.
            let patched;
            let e = if e.style.computed().offset_icb {
                let mut copy = e.clone();
                if copy.style.computed().has_viewport_lengths(vcx) {
                    copy.style.used_mut().resolve_viewport(opts.viewport, vcx);
                }
                crate::animation::motion::apply_icb(copy.style.used_mut(), opts.viewport, vcx);
                patched = copy;
                &patched
            } else {
                e
            };
            paint_element(
                e,
                nodes,
                idx,
                inherited,
                opts,
                ordered_context,
                under_tf,
                &frame,
                &mut out,
                &mut below,
            );
        }
    }
    if !pending.is_empty() {
        let t = trim::para_flags(trim_own, first_para, true);
        out.push(paint_inline_step7(letter_scope.paragraph(
            &pending,
            inherited,
            &opts.inner(|cx| cx.inline.trim_para = Some((owner, t))),
        )));
    }
    if (inherited.text_box_trim_start || inherited.text_box_trim_end)
        && !out.is_empty()
        && !ordered_context
    {
        trim::apply(&mut out, nodes, inherited, opts);
    }
    // Верхний слой: то, что обязано рисоваться поверх соседей, идёт последним
    // и возвращается на своё место замеренным сдвигом.
    out.extend(late_layer.finish());
    containment_paint::collect(out, inherited)
}

pub(super) struct BelowRun {
    start: usize,
    end: usize,
    zs: Vec<i32>,
}
