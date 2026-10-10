//! Пробег обтекания формой (shape-outside): синтетический узел shape-flow.

use crate::dom::{Element, Node};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;
use std::ops::ControlFlow;

/// Пробег обтекания ФОРМОЙ: у всех флоатов `shape-outside` и известный размер (или картинка).
pub(super) fn shaped_run_of(floaters: &[Element]) -> bool {
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let sized = |e: &Element| -> Option<(f32, f32)> {
        let b = e.style.borders();
        Some((
            px_of(&e.style.width)?
                + px_of(&e.style.padding.left)?
                + px_of(&e.style.padding.right)?
                + px_of(&b.left)?
                + px_of(&b.right)?
                + px_of(&e.style.margin.left)?
                + px_of(&e.style.margin.right)?,
            px_of(&e.style.height)?
                + px_of(&e.style.padding.top)?
                + px_of(&e.style.padding.bottom)?
                + px_of(&b.top)?
                + px_of(&b.bottom)?
                + px_of(&e.style.margin.top)?
                + px_of(&e.style.margin.bottom)?,
        ))
    };
    let img_float = |f: &Element| {
        f.style
            .shape_outside
            .as_deref()
            .is_some_and(|r| r.contains("url("))
            && (f.tag == "img"
                || f.children
                    .iter()
                    .any(|n| matches!(n, Node::Element(c) if c.tag == "img")))
    };
    floaters.iter().any(|f| f.style.shape_outside.is_some())
        && floaters.iter().all(|f| sized(f).is_some() || img_float(f))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_shaped_run(
    parent: &Computed,
    cb_width: Option<Len>,
    out: &mut Vec<Node>,
    i: &mut usize,
    side: i8,
    floaters: &mut Vec<Element>,
    j: usize,
    rest: &mut Vec<Node>,
    out_of_flow: &mut Vec<Node>,
    shaped_run: bool,
) -> ControlFlow<()> {
    if shaped_run {
        // Whitespace between the run and a preceding block start
        // collapses away (CSS 2.1 §16.6.1); left here it became its own
        // line above the shaped floats (`shape-outside-001`: +16px).
        if out
            .iter()
            .all(|n| matches!(n, Node::Text(t) if blank_text(t)))
        {
            out.clear();
        }
        let mut host = Element {
            list_item: None,
            node_id: 0,
            anim: None,
            tag: "shape-flow".into(),
            style: Computed {
                // Ширина содержащего блока — для долей формы и поля.
                width: cb_width,
                // Вертикальное письмо: инлайн-размер содержащего блока —
                // его ФИЗИЧЕСКАЯ высота (css-writing-modes-4 §6.1). Строкам
                // ряда (`FlowRow::vertical_rl`) нужен её предел: сам
                // ряд лежит в автовысотном хосте и иначе получает 0.
                height: if parent.vertical == Some(true) {
                    parent.height
                } else {
                    None
                },
                ..Computed::default()
            },
            hover: None,
            first_letter: None,
            first_line: None,
            children: Vec::new(),
            // Подготовка выше сняла float с самих блоков — сторона и
            // число уезжают атрибутами.
            attrs: vec![
                (
                    "side".into(),
                    if side < 0 {
                        "left".into()
                    } else {
                        "right".into()
                    },
                ),
                ("count".into(), floaters.len().to_string()),
            ],
            inline: false,
        };
        host.children = std::mem::take(floaters)
            .into_iter()
            .map(Node::Element)
            .collect();
        host.children.extend(std::mem::take(rest));
        out.push(Node::Element(host));
        out.extend(std::mem::take(out_of_flow));
        *i = j;
        return ControlFlow::Break(());
    }
    ControlFlow::Continue(())
}
