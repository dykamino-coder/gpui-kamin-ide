//! Пункт списка с заданным стилем: маркер, его позиция и содержимое.

use super::{shrink0, tabular_marker};
use crate::dom::{Element, Node};
use crate::layout::build_contracts::Styling;
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed};
use crate::text::inline;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

pub(crate) async fn render_with_style(
    stk: &mut reblessive::Stk,
    li: &Element,
    inherited: &Computed,
    merged: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    let resolved = super::marker::resolve(li, inherited, merged, vcx);
    let no_marker = resolved.is_none();
    // `inside`: маркер — ПЕРВЫЙ инлайновый кусок содержимого пункта
    // (css-lists-3 §4), поэтому он просто дописывается текстом в начало.
    // Своей колонки при этом нет, и текст пункта начинается там же, где
    // у обычного абзаца.
    let inside = merged.list_style_inside == Some(true);
    if inside {
        let mut kids: Vec<Node> = Vec::with_capacity(li.children.len() + 1);
        if let Some(marker) = resolved {
            kids.push(super::marker::node(li, merged, marker));
        }
        kids.extend(li.children.iter().cloned());
        return shrink0(
            opts.cx.contracts.build.styled_box(li, merged, &opts.cx),
            li,
            inherited,
        )
        .flex()
        .flex_col()
        .children(
            opts.cx
                .contracts
                .build
                .blocks_stack(stk, &kids, merged, opts)
                .await,
        )
        .into_any_element();
    }
    let (marker, marker_style, image) = resolved
        .map(|m| (m.text, m.style, m.image))
        .unwrap_or_default();
    // Знаки маркера без своего семейства — шрифтом ДОКУМЕНТА: голый
    // `apply_text` без семейства брал шрифт интерфейса (Segoe UI), и
    // строковый маркер выходил чужой гарнитурой рядом с Times пункта
    // (`list-style-type-string-*`).
    let mut mark_style = super::outside::content_style(marker_style, merged, vcx);
    if mark_style.font_family.as_deref().is_none_or(str::is_empty)
        && mark_style.monospace != Some(true)
    {
        mark_style.font_family = Some(opts.text.font_family.to_string());
    }
    tabular_marker(
        &mut mark_style,
        merged,
        li.style.used().marker_layer.as_deref(),
    );
    let marker = inline::transform_case(&marker, &mark_style);
    let marker_opts = super::outside_flow::options(opts);
    // Маркер и распорка пустого пункта используют CSS-метрики строки.
    // Прямой текстовый лист GPUI наследует line-height интерфейса, тогда
    // как `normal` первой строки зависит от шрифта пункта (CSS2 §10.8.1).
    mark_style.nowrap = Some(true);
    let marker_nodes = match image {
        Some(image) => vec![image, Node::Text("\u{a0}".into())],
        None => vec![Node::Text(match marker.strip_suffix(' ') {
            Some(head) => format!("{head}\u{a0}"),
            None => marker,
        })],
    };
    // Без строк внешний маркер выровнен по верху пункта, высота которого
    // не меньше маркера (csswg-drafts#2417, #2418; Blink
    // `UnpositionedListMarker::AddToBoxWithoutLineBoxes`). Скрытая строка
    // резервирует эту высоту, сохраняя собственную baseline шрифта пункта.
    let empty = li.children.iter().all(crate::dom::whitespace::is_blank);
    let strut = (!no_marker && empty).then(|| {
        let mut d = opts
            .cx
            .contracts
            .build
            .styled_div(Styling::Text(&mark_style), &opts.cx)
            .w_0()
            .flex()
            .items_start()
            // Заменяемый маркер увеличивает минимальную высоту, сохраняя
            // baseline шрифта самого пункта (CSS2 §10.8.1).
            .child(
                div()
                    .w_0()
                    .flex_shrink_0()
                    .children(opts.cx.contracts.build.blocks(
                        &[Node::Text("\u{a0}".into())],
                        &super::outside::content_style(None, merged, vcx),
                        opts,
                    )),
            )
            .children(
                opts.cx
                    .contracts
                    .build
                    .blocks(&marker_nodes, &mark_style, &marker_opts),
            );
        d.style().visibility = Some(gpui::Visibility::Hidden);
        d
    });
    // Внешний маркер (css-lists-3 §list-style-position `outside`) висит
    // СНАРУЖИ коробки пункта, концом к началу содержимого, и текст пункта
    // не двигает. Преобразования текста применяются к самому маркеру (§3.1.1).
    let anchor = super::placement::Anchor::default();
    shrink0(
        opts.cx.contracts.build.styled_box(li, merged, &opts.cx),
        li,
        inherited,
    )
    .relative()
    .flex()
    .flex_col()
    .children((!no_marker).then(|| {
        // Знаки маркера набираются шрифтом и цветом ПУНКТА:
        // отдельной коробке текстовые свойства не достаются сами,
        // и маркер выходил чужой гарнитурой и кеглем. Выключка
        // текста на него НЕ переносится: маркер стоит у своего
        // края колонки, куда бы ни равнялся текст пункта
        // (`list-style-position-018`). Со слоем `::marker` —
        // стилем слоя: разрядка, межсловный пробел, шрифт и цвет
        // маркера объявлены на нём (css-lists-3
        // §marker-properties).
        anchor.marker(
            super::outside::position(
                opts.cx
                    .contracts
                    .build
                    .styled_div(Styling::Text(&mark_style), &opts.cx),
                &li.style.used(),
                merged,
            )
            // Созданный суффикс — NBSP; авторский NBSP остаётся
            // значимым содержимым (css-lists-3 §3.1.1).
            .children(
                opts.cx
                    .contracts
                    .build
                    .blocks(&marker_nodes, &mark_style, &marker_opts),
            )
            .into_any_element(),
            merged.wm().vertical(),
        )
    }))
    // Содержимое — своей колонкой, как прежде: выключка пункта
    // (`text-align: end` у `<li>`) иначе становится выравниванием
    // его детей, и блок с `text-align: initial` уезжал к концу
    // (`marker-text-align-001-ref`).
    .child(
        anchor.content(
            div()
                .w_full()
                .flex()
                .flex_col()
                .children(
                    opts.cx
                        .contracts
                        .build
                        .blocks_stack(stk, &li.children, merged, opts)
                        .await,
                )
                .children(strut)
                .into_any_element(),
        ),
    )
    .into_any_element()
}

#[cfg(test)]
mod tests;
