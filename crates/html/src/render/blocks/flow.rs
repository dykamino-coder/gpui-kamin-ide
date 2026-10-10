//! Цикл по детям блока: строчные прогоны, блочные дети, позиционированные (blocks_flow).

mod inline_node;
pub(super) use inline_node::inline_node;

mod element;
pub(super) use element::paint_element;

use crate::dom::Node;
use crate::paint::effects::containment_paint;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::text_box_line_style;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

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
    let mut below = BelowRun {
        start: below_run_start,
        end: below_run_end,
        zs: below_zs,
    };
    for (idx, n) in nodes.iter().enumerate() {
        if run_breaks.contains(&idx) && !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            out.push(paint_inline_step7(
                letter_scope.paragraph(&taken, inherited, opts),
            ));
        }
        let is_inline = inline_node(n, inherited, ordered_context, &pending);
        if is_inline {
            pending.push(n.clone());
            continue;
        }
        if !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            out.push(paint_inline_step7(
                letter_scope.paragraph(&taken, inherited, opts),
            ));
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
        out.push(paint_inline_step7(
            letter_scope.paragraph(&pending, inherited, opts),
        ));
    }
    // `text-box-trim` (css-inline-3 §4.2): у блочного контейнера срезается
    // блочно-начальная сторона ПЕРВОЙ отформатированной строки и
    // блочно-конечная — ПОСЛЕДНЕЙ. Выражается отрицательным полем на первом и
    // последнем ребёнке: коробка ужимается ровно на срез, а содержимое
    // остаётся на месте.
    //
    // Строку ищет `text_box_line_style` по css-pseudo-4: у контейнера с
    // блочным содержимым это первая строка ПЕРВОГО in-flow блочного ребёнка,
    // и если у того строки нет (пустой `<div>`, пустая анонимная коробка) —
    // срезать нечего (`half-leading-block-box-001/003`). «Intervening
    // non-zero padding or borders» — отступы и рамки ПОТОМКОВ между
    // контейнером и строкой (`-004/-005`), а не самого контейнера: его
    // собственный отступ срезу не мешает (`-006`). Метрики — от корневой
    // строчной коробки найденной строки, то есть от стиля её блока.
    if (inherited.text_box_trim_start || inherited.text_box_trim_end)
        && !out.is_empty()
        && !ordered_context
    {
        // Срез с одной стороны: полулидинг строки плюс расстояние от
        // подъёма/спуска до заданной метрики края (`text` — ноль, `cap`/`ex`
        // — остаток над прописной/строчной, `alphabetic` — весь спуск).
        let trim_for = |line_style: &Computed, start: bool| -> f32 {
            let size = match line_style.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            };
            let family = line_style.font_family.clone().unwrap_or_default();
            let (ascent, descent, cap) = crate::text::metrics::vmetrics_px(&family, size);
            let line = match line_style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => ascent + descent,
            };
            // Полулидинг — половина разницы между высотой строки и метрикой
            // содержимого (CSS 2.1 §10.8.1).
            let half = (line - (ascent + descent)) / 2.0;
            // Край — у КОРНЕВОЙ СТРОЧНОЙ КОРОБКИ найденной строки (css-inline-3
            // §text-box-trim: «to the specified metric of its root inline
            // box»): `text-box-edge` наследуемое, `inline::inherit` его несёт,
            // а явное `auto` на блоке строки перекрывает `ex` контейнера
            // (`not-ignore-nested-text-box-edge`; Blink `AdjustEdges`:
            // kAuto = kText).
            if start {
                let over = match line_style.text_box_over {
                    crate::style::computed::TextEdge::Cap => ascent - cap,
                    crate::style::computed::TextEdge::Ex => {
                        ascent - crate::text::metrics::ch_ex_px(&family, size).1
                    }
                    _ => 0.0,
                };
                half + over
            } else {
                let under = match line_style.text_box_under {
                    // Алфавитная линия может стоять НАД нулём глифа (BASE `romn`,
                    // `BaselineDiagnostic`: +50/1000 — `text-box-trim-end-002`).
                    crate::style::computed::TextEdge::Alphabetic => {
                        descent + crate::text::fonts::alphabetic_em(&family) * size
                    }
                    _ => 0.0,
                };
                half + under
            }
        };
        if inherited.text_box_trim_start
            && let Some(line_style) = text_box_line_style(nodes, inherited, true)
        {
            let trim = trim_for(&line_style, true);
            if trim > 0.0 {
                let first = out.remove(0);
                // Вертикальный блок кладёт детей рядом (`flex_row` у
                // `vertical-lr`, `flex_row_reverse` у `vertical-rl`): блок-старт —
                // левый или правый край, верхнее поле двигало строку ВДОЛЬ неё
                // (`text-box-trim-half-leading-block-box-002`).
                let holder = match (
                    inherited.vertical == Some(true),
                    inherited.vertical_rl == Some(true),
                ) {
                    (false, _) => div().mt(px(-trim)),
                    (true, false) => div().ml(px(-trim)),
                    (true, true) => div().mr(px(-trim)),
                };
                out.insert(0, holder.child(first).into_any_element());
            }
        }
        if inherited.text_box_trim_end
            && let Some(line_style) = text_box_line_style(nodes, inherited, false)
        {
            let trim = trim_for(&line_style, false);
            if trim > 0.0 {
                let last = out.pop().expect("список не пуст");
                // Блок-конец вертикали: правый край у `vertical-lr`, левый у
                // `vertical-rl`.
                let holder = match (
                    inherited.vertical == Some(true),
                    inherited.vertical_rl == Some(true),
                ) {
                    (false, _) => div().mb(px(-trim)),
                    (true, false) => div().mr(px(-trim)),
                    (true, true) => div().ml(px(-trim)),
                };
                out.push(holder.child(last).into_any_element());
            }
        }
    }
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
