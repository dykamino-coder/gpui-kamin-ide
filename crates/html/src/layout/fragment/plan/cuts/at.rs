//! Место разреза ребёнка в колонке (cut_at): монолиты, вырезы и запреты.

use crate::layout::fragment::types::Kid;

#[allow(clippy::too_many_arguments)]
pub(crate) fn cut_at(
    paged: bool,
    placed: bool,
    k: &Kid,
    from: f32,
    room: f32,
    edge: f32,
    mono: bool,
    at: impl Fn(f32) -> (f32, f32),
    overflow_to: Option<f32>,
    shortage: &mut f32,
    tokens: &mut super::inline::Tokens,
    inline: &crate::layout::fragment::flow::inline::Geometry,
) -> Result<Option<(f32, f32)>, crate::layout::fragment::token::Error> {
    let candidate = if paged && !mono && room > 0.01 && overflow_to.is_none() {
        super::inline::cut(tokens, inline, from, room, placed, shortage)?
    } else {
        None
    };
    let edge = match candidate {
        Some(Some((point, _))) => point,
        Some(None) => return Ok(None),
        None => edge,
    };
    let solid = if candidate.is_some() {
        &inline.barriers
    } else {
        &k.solid
    };
    let holds = |a: f32, b: f32| a < edge - 0.01 && edge < b - 0.01;
    let cut = if let Some(b) = overflow_to {
        Some(at(b))
    } else if mono || room <= 0.01 {
        None
    } else if paged && solid.iter().any(|&(a, b)| holds(a, b)) {
        // Страницы: край внутри монолитных диапазонов, а они бывают
        // ВЛОЖЕНЫ (`avoid` ряда/группы объемлет монолиты ячеек,
        // `table_shape`). Беречь — самый внешний из тех, что
        // начинаются ниже `from`. Если и внешний уже начат
        // (`a <= from`), его не сберечь: с непустой страницы —
        // перенос целиком (`None if placed`), с верха пустой —
        // ближайшая внутренняя точка, а не срез по краю (Blink
        // `FinishFragmentation`: срез = `kBreakAppealLastResort`,
        // `HasEarlyBreak` → `kNeedsEarlierBreak`; css-break-4
        // §unforced-breaks: «the UA may use the avoids … to weigh
        // the appropriateness of the new breakpoints»;
        // `row-page-break-inside-avoid-1`: «3» на третьем листе в
        // обеих сторонах пары).
        let outer = solid
            .iter()
            .filter(|&&(a, b)| holds(a, b))
            .map(|&(a, _)| a)
            .fold(f32::MAX, f32::min);
        if outer > from + 0.01 {
            Some(at(outer))
        } else if placed {
            None
        } else {
            solid
                .iter()
                .filter(|&&(a, b)| holds(a, b) && a > from + 0.01)
                .map(|&(a, _)| a)
                .fold(None::<f32>, |m, a| Some(m.map_or(a, |x| x.min(a))))
                .map(&at)
        }
    } else if let Some(&(a, _)) = solid.iter().find(|&&(a, b)| holds(a, b)) {
        // Колонки — как прежде: первый содержащий диапазон. Его
        // начало само может лежать ВНУТРИ другого диапазона —
        // закрытой запретом границы (`shape_full`, `blk_avoid`):
        // тогда разрыв уходит к началу и того (`break-between-
        // avoid-007`: край в монолите c, перед c граница с `break-
        // before: avoid` — разрыв между a и b, а не перед c).
        let mut a = a;
        // Начало строго убывает по конечному списку диапазонов.
        while let Some(&(start, _)) = solid
            .iter()
            .chain(&k.avoid)
            .find(|&&(start, end)| start < a - 0.01 && a < end - 0.01 && start > from + 0.01)
        {
            a = start;
        }
        if a > from + 0.01 { Some(at(a)) } else { None }
    } else if !paged {
        // Сначала допустимая точка разрыва, затем перенос целиком.
        // Срез последней возможности выбирает place_cut лишь для коробки,
        // которая выше целой колонки (css-break-3 §4.4).
        k.cuts
            .iter()
            .map(|&(point, _)| point)
            .filter(|&point| point > from + 0.01 && point <= edge + 0.01)
            .max_by(f32::total_cmp)
            .map(&at)
    } else {
        Some(at(edge))
    };
    let Some((mut point, mut next)) = cut else {
        return Ok(None);
    };
    // Inline-селектор уже перепроверил строки после отката avoid.
    let keep: &[(f32, f32)] = if candidate.is_some() { &[] } else { &k.keep };
    for _ in 0..keep.len() {
        let start = keep
            .iter()
            .filter(|&&(a, b)| a < point - 0.01 && point < b - 0.01 && a >= from - 0.01)
            .map(|p| p.0)
            .min_by(f32::total_cmp);
        let Some(start) = start else { break };
        if start <= from + 0.01 {
            if placed {
                return Ok(None);
            }
            break;
        }
        (point, next) = at(start);
    }
    tokens.commit(point);
    Ok(Some((point, next)))
}
