//! apply_box, этап после полей и рамок: обрезка по clip-path circle(), contain: layout/paint, вид позиционирования (sticky выходит сразу); дальше — box_overflow.

use super::*;

pub(super) fn box_clip_position(mut d: Div, c: &Computed) -> Div {
    // `clip-path: circle()` — обрезка содержимого по кругу. Прямоугольная
    // обрезка со скруглением — единственная в конвейере, но для круга и
    // эллипса она точна.
    if let Some(round) = c
        .clip_round
        .filter(|_| !crate::paint::effects::grouped::rounded_rect_clip(c))
    {
        let base = match (c.width, c.height) {
            (Some(Len::Px(w)), Some(Len::Px(h))) => w.min(h),
            (Some(Len::Px(w)), _) => w,
            (_, Some(Len::Px(h))) => h,
            _ => 0.0,
        };
        // Доля без известного размера — это «половина стороны», то есть
        // заведомо большое значение: растеризатор обрежет его сам. Раньше
        // 0.5 понималось как полпикселя, и круг выходил квадратом.
        let radius = if round <= 1.0 {
            if base > 0.0 {
                round * base
            } else {
                9999.0 * round
            }
        } else {
            round
        };
        // Обрезка НЕ отменяет собственное скругление: берётся более сильное
        // из двух, иначе `border-radius` рядом с `clip-path` пропадал.
        let own = match c.radius.tl {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        d = d.rounded(px(radius.max(own))).overflow_hidden();
    }
    // `contain: paint` — содержимое не выходит за коробку.
    // `contain: layout` (и `strict`/`content`, которые раскрываются в него):
    // коробка «is treated as having no baseline» (css-contain-2 §3.2 п.7).
    // Родитель — строка, flex, grid — синтезирует её от края коробки. Прежде
    // базовая линия текста внутри уходила наружу: `inline-block` с «a»
    // вставал выше пустого соседа (`contain-layout-baseline-001..003`).
    if c.contain_layout == Some(true) {
        d.style().hides_baseline = Some(true);
    }
    if c.contain_paint == Some(true) {
        // Обрезка по краю БЕЗ контейнера прокрутки (css-contain-2 §3.3
        // paint containment: «contents … clipped to the overflow clip
        // edge»; вычисленный `overflow` остаётся `visible`) — это `clip`, а
        // не `hidden`. `hidden` в taffy — контейнер прокрутки, а у него
        // базовой линии нет (`compute/block.rs` `hides_baseline`), и
        // `inline-block` с `contain: paint` садился на строку низом полей
        // (`contain-paint-independent-formatting-context-002`). Ось с уже
        // заданным `overflow` не трогается.
        let o = &mut d.style().overflow;
        if o.x.is_none_or(|x| x == gpui::Overflow::Visible) {
            o.x = Some(gpui::Overflow::Clip);
        }
        if o.y.is_none_or(|y| y == gpui::Overflow::Visible) {
            o.y = Some(gpui::Overflow::Clip);
        }
    }

    match c.position {
        // Слой окна для `fixed` создаёт сборщик дерева; внутри него элемент
        // размещается так же, как абсолютный.
        Some(Position::Fixed) | Some(Position::Absolute) => {
            d = d.absolute();
            // Абсолютный элемент, у которого задан только один край, не имеет
            // определённой ширины — раскладка сжимает его до самого узкого
            // содержимого, и текст встаёт столбиком по букве. В браузере такой
            // элемент занимает ширину содержимого без переносов; повторяем это.
            // Явный `auto` краем не считается (CSS 2.1 §9.3.2).
            let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
            let horizontal = edge(c.inset.left) && edge(c.inset.right);
            if !horizontal && c.width.is_none() {
                d = d.flex_shrink_0().whitespace_nowrap();
            }
        }
        Some(Position::Relative) => d = d.relative(),
        // Липкий остаётся в потоке: край для него — порог прилипания, а не
        // сдвиг, поэтому вставки ниже к нему не применяются.
        Some(Position::Sticky) => return d.relative(),
        // `static` в GPUI недостижим: элемент всегда участвует в потоке
        // относительно родителя, что соответствует `relative`.
        _ => {}
    }
    box_overflow(d, c)
}
