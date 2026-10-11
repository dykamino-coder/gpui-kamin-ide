//! Высота коробки по формам детей и её поля.

use crate::dom::{Element, Node};
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::grid_bands::{grid_auto_row_bands, grid_rows_px};
use crate::layout::fragment::{ShapeCx, fragment_size};
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
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
) -> Result<(f32, f32, f32), Option<(f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)>> {
    let (h, mt, mb) = match c
        .style
        .height
        .as_ref()
        .map(|_| px_or(&c.style.height, true))
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
                fragment_size::border_size(v, &c.style, top + bot)
                    .max(stacked.map_or(0.0, |s| s.0 + s.1.min(0.0)) + bot)
            } else {
                fragment_size::border_size(v, &c.style, top + bot)
            },
            mt,
            mb,
        ),
        Some(None) => return Err(None),
        None => match stacked {
            Some((end, through, last_mb)) => (
                // Нижнее поле последнего ребёнка при собственном нижнем
                // отступе/рамке коробки НЕ схлопывается наружу и входит в
                // высоту (CSS 2.1 §10.6.3, §8.3.1): `table-fragmentation-
                // 001c-ref` — `.table { padding }` + `.td { margin: .25in 0 }`,
                // мера 360 при рисунке 384, нижняя рамка не влезала в копию.
                // У страниц; колонки не трогаются (отдельный замер).
                end + if bot > 0.0 && cx.paged { last_mb } else { 0.0 } + bot,
                mt.max(through),
                if bot == 0.0 { mb.max(last_mb) } else { mb },
            ),
            // Сетка без заданной высоты: её высоту знают
            // ЯВНЫЕ дорожки рядов (`grid-template-rows:
            // 200px`) с зазорами между ними. Без этой
            // оценки укладка колонок отказывалась от всей
            // коробки, и многоколоночник с сеткой внутри
            // уходил в запасную сетку целиком
            // (`scout-break-2026-09b.md`, корень C1).
            // Дорожки идут ПЕРЕД пустотой: у сетки БЕЗ детей высота всё
            // равно есть — её задают дорожки (css-grid-1 §7.1: дорожка
            // существует независимо от того, занята ли она). Прежде
            // `kids.is_empty()` заслонял эту ветку, и
            // `grid-container-fragmentation-002` (`grid-template-rows:
            // 200px`, детей нет) мерился нулём: квадрат 100×100 красен
            // ЦЕЛИКОМ (снимок `target/wpt-shots/_fg-grid-container-
            // fragmentation-002.png`, `x 10..134, y 67..191`). Проба
            // `target/probe-fg/p-fg-002.html` (та же сетка блоком 200px)
            // = 0.00.
            None => match grid_rows_px(&c.style) {
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
                None => match grid_auto_row_bands(c, depth, cx) {
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
                                solid.push((start, start + s.0));
                                continue;
                            }
                            let (true, Some(&(r0, _))) = (*plain, b.get(*row)) else {
                                continue;
                            };
                            let start = top + r0 + kmt;
                            for (need, nf) in &s.3 {
                                cuts.push((start + need, start + nf));
                            }
                            for f in &s.4 {
                                forced.push(start + f);
                            }
                            for (a, e) in &s.5 {
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
