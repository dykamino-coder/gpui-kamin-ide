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
    holds: impl Fn(f32, f32) -> bool,
    overflow_to: Option<f32>,
) -> Option<(f32, f32)> {
    if let Some(b) = overflow_to {
        Some(at(b))
    } else if mono || room <= 0.01 {
        None
    } else if paged && k.solid.iter().any(|&(a, b)| holds(a, b)) {
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
        let outer = k
            .solid
            .iter()
            .filter(|&&(a, b)| holds(a, b))
            .map(|&(a, _)| a)
            .fold(f32::MAX, f32::min);
        if outer > from + 0.01 {
            Some(at(outer))
        } else if placed {
            None
        } else {
            k.solid
                .iter()
                .filter(|&&(a, b)| holds(a, b) && a > from + 0.01)
                .map(|&(a, _)| a)
                .fold(None::<f32>, |m, a| Some(m.map_or(a, |x| x.min(a))))
                .map(at)
        }
    } else if let Some(&(a, _)) = k.solid.iter().find(|&&(a, b)| holds(a, b)) {
        // Колонки — как прежде: первый содержащий диапазон. Его
        // начало само может лежать ВНУТРИ другого диапазона —
        // закрытой запретом границы (`shape_full`, `blk_avoid`):
        // тогда разрыв уходит к началу и того (`break-between-
        // avoid-007`: край в монолите c, перед c граница с `break-
        // before: avoid` — разрыв между a и b, а не перед c).
        let mut a = a;
        for _ in 0..4 {
            match k
                .solid
                .iter()
                .find(|&&(s0, s1)| s0 < a - 0.01 && a < s1 - 0.01 && s0 > from + 0.01)
            {
                Some(&(s0, _)) => a = s0,
                None => break,
            }
        }
        if a > from + 0.01 { Some(at(a)) } else { None }
    } else {
        Some(at(edge))
    }
}
