//! Блочный поток: дети блока в элементы (blocks).

use crate::layout::options::RenderOpts;
use crate::render::first_letter_descendants;
use crate::render::first_letter_scope;
use crate::render::first_line_descendants;
use crate::style::computed::formatting_context::own_context_style;
mod prepare_children;
pub(super) use prepare_children::prepare_children;

mod ordered_children;
use ordered_children::ordered_children;
mod flow_children;
use flow_children::flow_children;

use crate::dom::Node;
use crate::interactive::sticky::sticky_probe;
use crate::layout::block::sizing::BoxDimensions;
mod context;
use crate::layout::block::struts::zero_len;
use crate::layout::float::initial_letter::initial_letter_float;
use crate::layout::float::wrap::wrap_floats;
use crate::layout::replaced::replaced_used_style;
use crate::paint::stacking::by_layer;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
pub(crate) use context::{blocks, blocks_stack, blocks_stack_with_dimensions};
use gpui::AnyElement;
pub(super) mod canvas;
pub(super) mod flow;
mod plan;
pub(super) mod positioned;
mod text_only;
use flow::blocks_flow;
pub(crate) use positioned::inset_holder_box;

// Не разворачивать `text-emphasis` в поштучные руби (по знаку-аннотации над
// каждой буквой базы, кроме пробелов и пунктуации — css-text-decor-3 §5.3):
// поштучный атом меняет разбивку строки и подъём базовой линии
// (`text-emphasis-line-height-004a..d`, `text-emphasis-punctuation-1/2`).
// Вернуть можно только вместе с настоящей надстрочной аннотацией (сдвиг
// базовой линии без атома).
async fn stack(
    stk: &mut reblessive::Stk,
    nodes: &[Node],
    inherited: &BoxStyle,
    sizes: BoxDimensions,
    opts: &RenderOpts,
    fresh: bool,
) -> Vec<AnyElement> {
    crate::work::time_self!(_own, BuildSelfChildrenNs);
    let mut inner = context::child_options(inherited, &sizes, opts, fresh);
    // Clean content of a box replays its last frame (`layout::reuse::content`).
    let reuse = opts.cx.contracts.reuse.as_ref();
    let content = reuse.and_then(|r| r.content(nodes, inherited, opts, &inner, fresh));
    if let (Some(reuse), Some(content)) = (reuse, &content)
        && let Some(out) = reuse.replay_content(content, &inner)
    {
        return out;
    }
    if let Some(content) = &content {
        content.track(&mut inner);
    }
    let mut out = build(
        stk,
        nodes,
        inherited,
        sizes,
        &inner,
        opts.cx.constraints.cell_bfc,
    )
    .await;
    if let (Some(reuse), Some(content)) = (reuse, content) {
        reuse.record_content(content, &inner, &mut out);
    }
    out
}

async fn build(
    stk: &mut reblessive::Stk,
    nodes: &[Node],
    inherited: &BoxStyle,
    sizes: BoxDimensions,
    opts: &RenderOpts,
    cell_bfc: bool,
) -> Vec<AnyElement> {
    let (ordered, under_tf) = prepare_children::container_flags(inherited);
    if text_only::applies(nodes, inherited, ordered) {
        return text_only::flow(stk, nodes, inherited, opts, under_tf).await;
    }
    let make = || plan::prepare(nodes, inherited, &sizes, opts, cell_bfc);
    let plan = match opts.cx.contracts.reuse.as_ref() {
        Some(reuse) => {
            let inputs = plan::inputs(&sizes, opts, cell_bfc);
            reuse.child_plan(nodes, inherited, opts, &inputs, make)
        }
        None => std::rc::Rc::new(make()),
    };
    let mut out = vec![];
    // Порядок краски подслоя (§9.9 шаг 3): соседние распорки отрицательного
    // `z-index` стоят в порядке разметки, а рисоваться обязаны по z. Высота у
    // них нулевая и y общий, поэтому перестановка СОСЕДЕЙ раскладку не меняет
    // — в отличие от перестановки в общем списке детей, которая ломает
    // наложение. Прогон рвётся сам, как только между распорками встаёт что-то
    // ещё.
    let below_run_start = 0usize;
    let below_run_end = usize::MAX;
    let below_zs: Vec<i32> = vec![];
    // Липкому ребёнку нужны две вещи, которых он сам не видит: коробка
    // родителя и видимая часть ленты. Их снимает распорка — она идёт первой,
    // потому что готовит замер до отрисовки детей.
    let frame: crate::interactive::sticky::element::StickyCell = Default::default();
    if plan.sticky {
        out.push(sticky_probe(frame.clone()));
    }
    // Слой верхней отрисовки этого контейнера: позиционированные элементы
    // складывают сюда содержимое, а забирается оно последними детьми.
    blocks_flow(
        stk,
        &plan.nodes,
        plan.run_breaks.clone(),
        out,
        plan.letter_scope,
        inherited,
        opts,
        plan.ordered_context,
        plan.under_tf,
        frame,
        below_run_end,
        below_run_start,
        below_zs,
    )
    .await
}
