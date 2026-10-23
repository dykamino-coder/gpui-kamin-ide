//! Высота коробки и её поля по стопке мер детей (`shaped_height`) для `shape_contents`:
//! заданная высота (у меры потока — не обрезающая, css-break-3 §3), высота по содержимому с
//! нижним полем последнего ребёнка (CSS 2.1 §8.3.1, §10.6.3, §10.6.7; корень BFC усекает его
//! разрывом, css-break-3 §5.2), сетка по явным дорожкам или по автоматическим рядам с
//! элементами как параллельными потоками (css-grid-2 §Fragmenting Grid Layout), пустая
//! коробка. `max-height` ниже содержимого отделяет поле ребёнка (CSS 2.2 §10.7).

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::grid_bands::{grid_auto_row_bands, grid_rows_px};
use crate::layout::fragment::{ShapeCx, fragment_size};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
// Heights are computed from the same fragment records used by the cutting pass.
#[allow(clippy::type_complexity)]
pub(super) fn shaped_height(
    c: &Element,
    depth: u8,
    px_or: impl Fn(&Option<Len>, bool) -> Option<f32>,
    mt: f32,
    mb: f32,
    unclamp: bool,
    cx: ShapeCx,
    top: f32,
    bot: f32,
    kids: Vec<&Node>,
    cuts: &mut Vec<(f32, f32)>,
    forced: &mut Vec<f32>,
    solid: &mut Vec<(f32, f32)>,
    stacked: Option<(f32, f32, f32)>,
    vcx: ValueCx<'_>,
) -> Result<(f32, f32, f32), Option<crate::layout::fragment::Shape>> {
    let (h, mt, mb) = match c
        .style
        .computed()
        .height
        .as_ref()
        .map(|_| px_or(&c.style.computed().height, true))
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
                fragment_size::border_size(v, &c.style.computed(), top + bot)
                    .max(stacked.map_or(0.0, |s| s.0 + s.1.min(0.0)) + bot)
            } else {
                fragment_size::border_size(v, &c.style.computed(), top + bot)
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
            None => match grid_rows_px(&c.style.computed()) {
                Some(v) => (v + top + bot, mt, mb),
                // Сетка, у которой дорожки рядов НЕ все в точках (несколько
                // колонок, ряд `auto`, неявный ряд), до сих пор отдавала
                // `return None`. `None` тут стоит дорого: в `blocks()`
                // (:13391) `stackable` собирается через
                // `collect::<Option<Vec<_>>>()`, и одна неизмеримая сетка
                // отменяет укладку колонок ВСЕГО многоколоночника вместе с
                // её соседями — колонка 1 переполнена, остальные пусты
                // (`grid-item-fragmentation-003`: сетка `auto auto` с
                // элементом 200px, ряд 200, ничего не фрагментируется).
                // Высота сетки с `height: auto` — сумма размеров дорожек
                // рядов (css-grid-2 §Fragmenting Grid Layout, шаг 4
                // «Sample Fragmentation Algorithm»; Blink
                // `grid_layout_algorithm.cc:370`
                // `Rows().CalculateSetSpanSize()`). Точек разреза эта ветка
                // не добавляет — см. `grid_auto_row_bands`.
                None => match grid_auto_row_bands(c, depth, cx, vcx) {
                    Some((b, spots)) => {
                        // Элементы ряда — параллельные потоки (css-break-3 §3;
                        // Blink `PlaceGridItems`: у каждого свой break token):
                        // точки и монолиты — ОБЪЕДИНЕНИЕМ, со смещением ряда,
                        // как у ряда flex без переноса выше. Рост элемента,
                        // чья строка ушла в следующую колонку, ставит
                        // `grow_pushed` через спуск `pushed_box_at`; после него
                        // строка стоит на краю, и общий срез совпадает с
                        // раздельными (css-grid-2 §12.1 шаг 3: ряд растёт).
                        // Границ РЯДОВ по-прежнему нет — см. выше.
                        for (ix, row, kmt, s, plain) in &spots {
                            // Страницы: монолитный элемент (`contain: size`,
                            // замещаемый…) — сплошной диапазон во всю его
                            // высоту, и край листа внутри него уводит разрыв
                            // к началу ряда (css-grid-2 §fragmenting: «a grid
                            // container may break between rows»; Blink
                            // `IsMonolithic` → разрыв перед рядом;
                            // `grid-fragmentation-between-rows-001-print`:
                            // второй ряд `contain: size` резался краем листа).
                            if cx.paged
                                && !*plain
                                && let (Some(&(r0, _)), Some(Node::Element(k))) =
                                    (b.get(*row), c.children.get(*ix))
                                && solid_box(k)
                            {
                                let start = top + r0 + kmt;
                                solid.push((start, start + s.height));
                                continue;
                            }
                            let (true, Some(&(r0, _))) = (*plain, b.get(*row)) else {
                                continue;
                            };
                            let start = top + r0 + kmt;
                            for (need, nf) in &s.cuts {
                                cuts.push((start + need, start + nf));
                            }
                            for f in &s.forced {
                                forced.push(start + f);
                            }
                            for (a, e) in &s.solid {
                                solid.push((start + a, start + e));
                            }
                        }
                        (b.last().map_or(0.0, |r| r.1) + top + bot, mt, mb)
                    }
                    None if kids.is_empty() => (top + bot, mt, mb),
                    None => return Err(None),
                },
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
    matches!(c.style.computed().max_height, Some(Len::Px(m))
        if fragment_size::border_size(m, &c.style.computed(), edges) < end - 0.01)
}
