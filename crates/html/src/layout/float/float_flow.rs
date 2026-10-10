//! Поток флоатов.
// owner: A

use crate::dom::{Element, Node};
use crate::paint::effects::paint_scope::DepthScope;
use crate::paint::effects::paint_scope::snapshot as defer_depth;
use crate::render::{
    RenderOpts, blocks, gather_text, is_blank, measure_font, split_nodes, styled_div_with,
};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

/// Ряд обтекания: рядом с плавающим блоком столько текста, сколько помещается
/// в его высоту, остальное — под ним на всю ширину.
///
/// Место разреза считает `FloatFlow`: ширина контейнера известна только на
/// замере, а без неё непонятно, сколько текста влезает сбоку. Если размеры
/// плавающего блока не заданы явно, резать нечем — тогда ряд остаётся прежним:
/// две колонки до конца абзаца.
pub(crate) fn float_flow(row: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    // Ряд без разреза — прежнее поведение: две колонки до конца абзаца.
    // Стиль берётся СЛИТЫЙ: у ряда своя раскладка, и без неё дети встают
    // друг под другом вместо колонок.
    let merged = inherit(inherited, &row.style);
    // Ряд обтекания ФИЗИЧЕСКИЙ: левые флоаты собраны в начало, правые — в
    // конец (сборка `float_runs`), а `float: left/right` от письма не
    // зависят (CSS 2.1 §9.5.1). Унаследованное `direction: rtl` разворачивало
    // ряд (`apply.rs`: `rtl_row` → `flex_row_reverse`), и `float: right` в
    // rtl-контейнере вставал слева (эталоны `flexbox-writing-mode-010..015`,
    // `flexbox-align-self-vert-rtl-*-ref`). Письмо снимается только с самой
    // коробки ряда — дети наследуют прежнее `merged`.
    let mut row_layout = merged.clone();
    if row_layout.vertical != Some(true) {
        row_layout.rtl = Some(false);
    }
    let plain_row = |nodes: &[Node]| -> AnyElement {
        styled_div_with(row, &row_layout)
            .children(blocks(nodes, &merged, opts))
            .into_any_element()
    };
    // Разрез обтекания понимает РОВНО пару «плавающий блок + колонка
    // текста». Ряд из НЕСКОЛЬКИХ плавающих (четыре float:left подряд) обязан
    // остаться обычным флекс-рядом: прежде сюда попадали первые два, а
    // остальные ВЫБРАСЫВАЛИСЬ (flex-flow-001-ref: из «1 2 3 4» рисовались
    // «1 2» — 23 красных flexbox-ref'а с float).
    if row.children.iter().filter(|n| !is_blank(n)).count() != 2 {
        return plain_row(&row.children);
    }
    // Плавающий блок в этой паре всегда первый, текстовая колонка — вторая.
    // Раньше здесь стоял `match`, у которого ОБЕ ветви давали `(0, 1)`:
    // условие вычислялось и ни на что не влияло.
    let (float_ix, text_ix) = (0, 1);
    let (Some(Node::Element(floater)), Some(Node::Element(column))) =
        (row.children.get(float_ix), row.children.get(text_ix))
    else {
        return plain_row(&row.children);
    };
    // Колонка текста — синтетическая, у плавающего блока её роли нет.
    let (floater, column) = if column.style.flex_grow == Some(1.0) {
        (floater, column)
    } else if let Some(Node::Element(other)) = row.children.get(1) {
        (other, floater)
    } else {
        return plain_row(&row.children);
    };
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let (Some(fw), Some(fh)) = (px_of(floater.style.width), px_of(floater.style.height)) else {
        return plain_row(&row.children);
    };
    let mut plain = String::new();
    gather_text(&column.children, &mut plain);
    if plain.trim().is_empty() {
        return plain_row(&row.children);
    }

    let left = matches!(row.children.first(), Some(Node::Element(e)) if std::ptr::eq(e, floater));
    let floater = floater.clone();
    let rest = column.children.clone();
    let column_style = column.style.clone();
    let row_node = row.clone();
    let inherited_owned = inherited.clone();
    let opts_owned = opts.clone();
    let depth = defer_depth();
    let build: crate::layout::float::split_flow::Split =
        std::rc::Rc::new(move |split: usize, width: gpui::Pixels| {
            let _depth = DepthScope::enter(depth);
            let (beside, below) = split_nodes(&rest, split);
            let mut side = column_style.clone();
            side.display = Some(Display::Block);
            let column_el = Element {
                list_item: None,
                node_id: 0,
                anim: None,
                tag: "div".into(),
                style: side,
                hover: None,
                first_letter: None,
                first_line: None,
                children: beside,
                attrs: vec![],
                inline: false,
            };
            let row_children = if left {
                vec![Node::Element(floater.clone()), Node::Element(column_el)]
            } else {
                vec![Node::Element(column_el), Node::Element(floater.clone())]
            };
            let mut top = row_node.clone();
            top.tag = "div".into();
            top.children = row_children;
            let mut all = vec![Node::Element(top)];
            all.extend(below);
            // Ширина коробки задаётся явно: дерево раскладывается отдельным
            // корнем, и без неё текст считает себя свободным и не переносится.
            div()
                .flex()
                .flex_col()
                .w(width)
                .children(blocks(&all, &inherited_owned, &opts_owned))
                .into_any_element()
        });

    let size = match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    };
    let line = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => size * k,
        _ => size * normal_fraction(inherited, opts),
    };
    let flow = crate::layout::float::split_flow::FloatFlow::new(
        build,
        SharedString::from(plain),
        (fw, fh),
        measure_font(inherited, opts),
        size,
        line,
    )
    .into_any_element();
    // Ряд обтекания строится ОТЛОЖЕННО: `build` зовёт `FloatFlow::prepaint`
    // (`float.rs:211`), когда сторож клэмп-контейнера уже снят, поэтому ни
    // флоат, ни строки рядом с ним проб не пишут, и `ClampCut` ряда не видит.
    // Счётному режиму это стоило флоатов за точкой среза: «за точкой ничего
    // нет» → потолка нет → ряд высотой max(50, 2×32) = 64 виден под
    // четвёртой строкой (`line-clamp-with-floats-003/004`: 192 вместо 128).
    // css-overflow-4 §5.3: «Any in-flow or floating boxes that follow the
    // clamp point in the box tree» — невидимы, и в автоматическую высоту не
    // входят; Blink раскладывает такой флоат с `is_hidden_for_paint`
    // (`inline/line_breaker.cc:3794`, `block_layout_algorithm.cc:1794`,
    // `floats_utils.cc:76`). Проба коробки (строка 0, без номера абзаца)
    // даёт `ClampCut` признак «за точкой есть содержимое» — потолок снова на
    // низе N-й строки. Авто-режим не трогаем: там пересечённая коробка без
    // считаемых строк внутри ушла бы целиком.
    match crate::text::clamp::clamp_context() {
        Some((key, skip)) if inherited.line_clamp.is_some() => div()
            .relative()
            .child(flow)
            .child(crate::text::clamp::clamp_probe(
                crate::text::clamp::clamp_lines_for(key),
                0.0,
                skip,
                false,
                0.0,
                None,
                None,
            ))
            .into_any_element(),
        _ => flow,
    }
}
