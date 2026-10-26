//! Одинаковая подготовка поточной стопки до и после определения ширины.
use crate::dom::{Element, Node};
use crate::layout::fragment::clone::clone_wrapper_item;
use crate::layout::fragment::line_shape::group_inline_runs;
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Position};

pub(crate) fn prepared(
    e: &Element,
    merged: &BoxStyle,
    opts: &RenderOpts,
    vertical: bool,
    width: Option<f32>,
) -> Element {
    let positioned = |s: &crate::style::computed::Computed| {
        matches!(s.position, Some(Position::Absolute | Position::Fixed))
    };
    // Строчные прогоны среди блоков — анонимными блоками (CSS 2.1
    // §9.2.1.1); неизвестная ширина разрешается в deferred layout.
    let mut boxed = e.clone();
    boxed.children =
        crate::layout::fragment::flow::box_children::projected(&e.children, merged, opts.cx.vcx())
            .into();
    if !vertical {
        boxed.children =
            crate::layout::fragment::flow::column_percentages(&boxed.children, merged, width, opts)
                .into();
    }
    let grouped_e = group_inline_runs(&boxed);
    let ge: &Element = grouped_e.as_ref().unwrap_or(&boxed);
    let floats_blocked = super::block_floats(ge, positioned, width, opts.cx.vcx());
    let ge: &Element = floats_blocked.as_ref().unwrap_or(ge);
    // Обёртка flex/сетки с ЕДИНСТВЕННЫМ элементом-`clone`
    // (`clone_wrapper_item`): по блочной оси такая обёртка
    // раскладывается ровно как блок с этим ребёнком, и фрагменты
    // клонированного украшения строятся у самого элемента.
    let unwrapped = ge
        .children
        .iter()
        .any(|n| matches!(n, Node::Element(c) if clone_wrapper_item(c).is_some()))
        .then(|| {
            let mut g = ge.clone();
            for n in g.children.iter_mut() {
                if let Node::Element(c) = n
                    && let Some(item) = clone_wrapper_item(c)
                {
                    *c = item;
                }
            }
            g
        });
    unwrapped.unwrap_or_else(|| ge.clone())
}
