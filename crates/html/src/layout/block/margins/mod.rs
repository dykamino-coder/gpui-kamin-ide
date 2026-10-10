//! Схлопывание полей в потоке (CSS 2.1 §8.3.1).
// owner: A

use crate::dom::Node;
use crate::layout::block::margin_inline_boxes;
use crate::layout::block::struts::{Strut, solve, through_strut_no_clear};
use crate::style::values::value::Len;
mod flow;
pub(super) use flow::collapse_flow_margins;
mod kids;
use kids::{collapse_kid_margins, emit_margin_struts};

/// Схлопывание вертикальных отступов соседних блоков.
///
/// В CSS нижний отступ одного блока и верхний отступ следующего не
/// складываются, а сливаются в больший из двух. Движок раскладки под нами
/// складывает их, и документ становится длиннее браузерного — расхождение
/// накапливается сверху вниз и было поймано сравнением с Chrome.
/// `abs_parent` — родитель абсолютно позиционирован: по §10.6.7 его
/// автовысота ВКЛЮЧАЕТ плавающих детей, и правило «блок из одних флоатов
/// высотой ноль» (§10.6.3) к его детям не применяется.
pub(crate) fn collapse_margins(nodes: &[Node], abs_parent: bool) -> Vec<Node> {
    let mut out: Vec<Node> = nodes.to_vec();
    margin_inline_boxes::prepare(&mut out);
    // CSS 2.1 §10.6.3: floats do not contribute to ordinary auto height.
    // Margin collapse proves zero in-flow height for an open empty block;
    // The contextual proof also handles borders, padding and white-space.
    // Formatting contexts retain floats (§10.6.7). Keep the absolute-parent
    // guard: its float containment currently depends on the child's height.
    for node in out.iter_mut().filter(|_| !abs_parent) {
        let Node::Element(e) = node else { continue };
        let has_float = e
            .children
            .iter()
            .any(|n| matches!(n, Node::Element(c) if c.style.float.is_some_and(|f| f != 0)));
        if has_float && through_strut_no_clear(e).is_some() {
            e.style.height = Some(Len::Px(0.0));
        }
    }
    // Отступ первого ребёнка «протекает» наружу, если родителя от него не
    // отделяют ни рамка, ни внутренний отступ: в CSS это один и тот же отступ,
    // а не два. Без этого блок уезжает вниз на величину детского отступа.
    collapse_kid_margins(&mut out);
    // Струна примыкающих полей соседей (§8.3.1). `emitted` — сколько точек уже
    // ЗАПИСАНО в стили этого зазора: раскладка складывает поля сама, и
    // верхнему полю следующего блока достаётся только разница.
    let mut strut: Option<Strut> = None;
    let mut emitted = 0.0f32;
    // Последняя коробка прогона с клиренсом: её остаток поля остаётся ВНУТРИ
    // родителя и наружу не уходит.
    let mut cleared_run: Option<usize> = None;
    emit_margin_struts(&mut out, &mut strut, &mut emitted, &mut cleared_run);
    // Прогон кончился на коробке с клиренсом: остаток слитого поля пишется ей
    // самой — родителя он растит, но наружу не выходит.
    if let (Some(i), Some(s)) = (cleared_run, strut) {
        let rest = solve(s) - emitted;
        if rest > 0.0
            && let Some(Node::Element(e)) = out.get_mut(i)
        {
            e.style.margin.bottom = Some(Len::Px(rest));
        }
    }
    out
}

thread_local! {
    /// Кегль РОДИТЕЛЯ на разбираемом уровне: единицы шрифта в отступах
    /// меряются от кегля элемента, а он к моменту схлопывания ещё не
    /// унаследован — наследование живёт ниже по пути (`inline::inherit`).
    /// Значение ставит `blocks()` вокруг вызова `collapse_margins` и
    /// возвращает на место после него.
    pub(crate) static COLLAPSE_FONT_PX: std::cell::Cell<f32> = const { std::cell::Cell::new(16.0) };
    /// Ширина содержащего блока уровня схлопывания (для процентных полей).
    pub(crate) static COLLAPSE_CB_WIDTH_PX: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
    /// Определена ли высота содержащего блока уровня схлопывания (§10.5):
    /// доля высоты ребёнка при неопределённой ведёт себя как `auto`.
    pub(crate) static COLLAPSE_CB_HEIGHT_DEF: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The next `blocks` call lays out a table cell's or caption's content:
    /// both are block formatting context roots (CSS 2.1 §9.4.1) and contain
    /// their floats (§10.6.7), even as a `td`/`caption` without `display`.
    pub(crate) static CELL_BFC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
