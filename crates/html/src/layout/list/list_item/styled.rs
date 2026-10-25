//! Пункт списка с заданным стилем: маркер, его позиция и содержимое.

use super::{shrink0, tabular_marker};
use crate::dom::{Element, Node};
use crate::layout::build_contracts::Styling;
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed};
use crate::text::inline;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

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
    // Without line boxes the outside marker is top-aligned to the item, and
    // the item's content height is at least the marker's (csswg-drafts#2417,
    // #2418; Blink `UnpositionedListMarker::AddToBoxWithoutLineBoxes`). An
    // item with no content gets a hidden zero-width copy of the marker line.
    let empty = li.children.iter().all(crate::dom::whitespace::is_blank);
    let strut = (!no_marker && empty).then(|| {
        let mut d = opts
            .cx
            .contracts
            .build
            .styled_div(Styling::Text(&mark_style), &opts.cx)
            .w_0()
            .whitespace_nowrap()
            .children(image.is_none().then(|| SharedString::from(marker.clone())))
            .children(
                image
                    .as_ref()
                    .map(|image| {
                        opts.cx.contracts.build.blocks(
                            std::slice::from_ref(image),
                            &mark_style,
                            opts,
                        )
                    })
                    .into_iter()
                    .flatten(),
            );
        d.style().visibility = Some(gpui::Visibility::Hidden);
        d
    });
    // Внешний маркер (css-lists-3 §list-style-position `outside`) висит
    // СНАРУЖИ коробки пункта, концом к началу содержимого, и текст пункта
    // не двигает. Text transforms apply to its own text (§3.1.1).
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
        super::outside::position(
            opts.cx
                .contracts
                .build
                .styled_div(Styling::Text(&mark_style), &opts.cx),
            &li.style.used(),
            merged,
        )
        .whitespace_nowrap()
        // Хвост срезается только у СОБСТВЕННЫХ отбивок движка
        // (обычный пробел после номера пункта). Авторская
        // строка `list-style-type: "..."` идёт дословно:
        // `trim_end` в Rust считает пробелом и U+00A0, а
        // неразрывный пробел в такой строке — ЗНАЧАЩИЙ.
        // Остаток `list-style-type-string-005a/b/-006` не здесь,
        // но срезать значащий знак всё равно нельзя.
        // Отбивка после номера (`1. `) — часть маркера: она и
        // даёт зазор до содержимого. Пробел в конце строки
        // свернулся бы, поэтому он неразрывный.
        .children(
            image
                .as_ref()
                .map(|image| {
                    opts.cx.contracts.build.blocks(
                        &[image.clone(), Node::Text("\u{a0}".into())],
                        &mark_style,
                        opts,
                    )
                })
                .into_iter()
                .flatten(),
        )
        .children(image.is_none().then(|| {
            SharedString::from(match marker.strip_suffix(' ') {
                Some(head) => format!("{head}\u{a0}"),
                None => marker.clone(),
            })
        }))
    }))
    // Содержимое — своей колонкой, как прежде: выключка пункта
    // (`text-align: end` у `<li>`) иначе становится выравниванием
    // его детей, и блок с `text-align: initial` уезжал к концу
    // (`marker-text-align-001-ref`).
    .child(
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
            .children(strut),
    )
    .into_any_element()
}
