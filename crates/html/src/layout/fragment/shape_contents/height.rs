//! Высота коробки и её поля по стопке мер детей (`shaped_height`) для `shape_contents`:
//! заданная высота (у меры потока — не обрезающая, css-break-3 §3), высота по содержимому с
//! нижним полем последнего ребёнка (CSS 2.1 §8.3.1, §10.6.3, §10.6.7; корень BFC усекает его
//! разрывом, css-break-3 §5.2), сетка по явным дорожкам в точках, пустая коробка; прочие
//! сетки фрагментирует разложенная копия (css-grid-2 §Fragmenting Grid Layout). `max-height` ниже содержимого отделяет поле ребёнка (CSS 2.2 §10.7).

use crate::dom::{Element, Node};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::fragment::grid_bands::grid_rows_px;
use crate::layout::fragment::{ShapeCx, fragment_size};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
// Heights are computed from the same fragment records used by the cutting pass.
#[allow(clippy::type_complexity)]
pub(super) fn shaped_height(
    c: &Element,
    px_or: impl Fn(&Option<Len>, bool) -> Option<f32>,
    mt: f32,
    mb: f32,
    unclamp: bool,
    cx: ShapeCx,
    top: f32,
    bot: f32,
    kids: Vec<&Node>,
    cuts: &mut Vec<(f32, f32)>,
    stacked: Option<(f32, f32, f32)>,
) -> Result<(f32, f32, f32), Option<crate::layout::fragment::Shape>> {
    let sizes = BoxDimensions::for_element(c, &c.style.used());
    let (h, mt, mb) = match c
        .style
        .used()
        .height
        .as_ref()
        .map(|_| px_or(&c.style.used().height, true))
    {
        // Мера потока (`unclamp`) заданной высотой не обрезается:
        // css-break-3 §3 «parallel flows» — переполнение продолжается в
        // следующем фрагментаинере само по себе, и его протяжённость нужна
        // укладке колонок. Высота КОРОБКИ при этом не меняется: её даёт
        // обычная мера (`unclamped: false`), и именно она остаётся шагом для
        // соседа. Ниже по функции тем же `h` усекаются `cuts`/`forced`/
        // `solid` — в мере потока они остаются полными, и это ровно то, что
        // нужно: точки разреза и монолиты хвоста.
        Some(Some(v)) => (
            if unclamp {
                // Отрицательное поле первого ребёнка, схлопнутое сквозь верх
                // коробки (`through`), поднимает всё содержимое: его низ —
                // `end + through` от верха коробки (`css-break/float-001`:
                // коробка `height: 0` с ребёнком 40px и `margin-top: -40px`
                // — содержимое кончается на её верху, а мера давала поток
                // 40, и коробка с края колонки уезжала в следующую).
                fragment_size::border_size(v, &sizes, top + bot)
                    .max(stacked.map_or(0.0, |s| s.0 + s.1.min(0.0)) + bot)
            } else {
                fragment_size::border_size(v, &sizes, top + bot)
            },
            mt,
            mb,
        ),
        Some(None) => return Err(None),
        None => match stacked {
            // Корень блочного контекста (`display: flow-root`, `overflow` не
            // `visible`, `fieldset`…) полей детей наружу не пропускает (CSS 2.1
            // §8.3.1: «margins of elements that establish new block formatting
            // contexts do not collapse with their in-flow children»): нижнее
            // поле последнего ребёнка входит в его высоту (§10.6.7). Разрыв
            // после ребёнка законен, и поле на нём усекается (css-break-3
            // §5.2) — Blink `block_layout_algorithm.cc` кончает коробку в
            // этом фрагментаинере (`trailing-child-margin-000/002`).
            Some((end, _, last_mb))
                if crate::layout::box_kind::bfc_root(c) && bot == 0.0 && last_mb > 0.0 =>
            {
                cuts.push((end, end + last_mb));
                (end + last_mb, mt, mb)
            }
            Some((end, through, last_mb)) => (
                // Нижнее поле последнего ребёнка при собственном нижнем
                // отступе/рамке коробки НЕ схлопывается наружу и входит в
                // высоту (CSS 2.1 §10.6.3, §8.3.1): `table-fragmentation-
                // 001c-ref` — `.table { padding }` + `.td { margin: .25in 0 }`,
                // мера 360 при рисунке 384, нижняя рамка не влезала в копию.
                // У страниц; колонки не трогаются (отдельный замер).
                end + if bot > 0.0 && cx.paged { last_mb } else { 0.0 } + bot,
                mt.max(through),
                if bot == 0.0 && !max_lowers(c, end + bot, top + bot) {
                    mb.max(last_mb)
                } else {
                    mb
                },
            ),
            // Сетка без заданной высоты: её высоту знают
            // ЯВНЫЕ дорожки рядов (`grid-template-rows:
            // 200px`) с зазорами между ними. Без этой
            // оценки укладка колонок отказывалась от всей
            // коробки, и многоколоночник с сеткой внутри
            // уходил в запасную сетку целиком.
            // Дорожки идут ПЕРЕД пустотой: у сетки БЕЗ детей высота всё
            // равно есть — её задают дорожки (css-grid-1 §7.1: дорожка
            // существует независимо от того, занята ли она). Прежде
            // `kids.is_empty()` заслонял эту ветку, и
            // `grid-container-fragmentation-002` (`grid-template-rows:
            // 200px`, детей нет) мерился нулём: квадрат 100×100 красен
            // ЦЕЛИКОМ (снимок: `x 10..134, y 67..191`). Проба с той же
            // сеткой блоком 200px = 0.00.
            None => match grid_rows_px(&c.style.used()) {
                Some(v) => (v + top + bot, mt, mb),
                // Прочие сетки (дорожки не все в точках) оценка не выражает:
                // ряды, размещение и продолжения элементов даёт разложенная
                // копия (`flow::blocks::grid`), а оценка отказывает.
                None if kids.is_empty() => (top + bot, mt, mb),
                None => return Err(None),
            },
        },
    };
    Ok((h, mt, mb))
}

/// `max-height` ниже содержимого отделяет нижнее поле последнего ребёнка от
/// поля коробки (CSS 2.2 §10.7: высота и поля пересчитываются с
/// ограничением; Blink `block_layout_algorithm.cc:1459-1466` — поле
/// отделяется, когда итоговый размер отличен от собственного; раскладка —
/// `margin_height::lowers`). Поле ребёнка наружу не идёт
/// (`parallel-flow-trailing-margin-001`: `max-height: 0`, поле 80 сдвигало
/// соседа).
fn max_lowers(c: &Element, end: f32, edges: f32) -> bool {
    let sizes = BoxDimensions::for_element(c, &c.style.used());
    matches!(sizes.maximum[1], Some(Len::Px(m))
        if fragment_size::border_size(m, &sizes, edges) < end - 0.01)
}
