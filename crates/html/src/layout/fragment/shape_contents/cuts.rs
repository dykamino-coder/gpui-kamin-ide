//! Вырезы формы: финальная обрезка по высоте, нижняя кромка, сквозной верх.

use crate::dom::{Element, Node};
use crate::layout::fragment::grid_bands::{grid_row_forced, grid_row_gaps};
use crate::layout::fragment::probe::forced_opaque;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::fragment::{ShapeCx, fragment_size};
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::positioned::predicates::OOF_OWN;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_shape_cuts(
    c: &Element,
    unclamp: bool,
    cx: ShapeCx,
    top: f32,
    bot: f32,
    mut cuts: Vec<(f32, f32)>,
    mut forced: Vec<f32>,
    mut solid: Vec<(f32, f32)>,
    stacked: Option<(f32, f32, f32)>,
    oof_reach: f32,
    h: f32,
    mt: f32,
    mb: f32,
) -> Option<(f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)> {
    let own_h = h;
    let h = if crate::text::inline::establishes_cb(&c.style) {
        h.max(oof_reach + bot)
    } else {
        h
    };
    let h = fragment_size::constrain(h, &c.style, top + bot, cx.viewport, unclamp);
    // Дотяг меняет меру фрагментации, но не размер коробки: сосед в потоке
    // встаёт под КОНЦОМ коробки, а абсолют продолжается параллельным потоком
    // (css-position-3 §abspos-breaking; Blink ведёт OOF во фрагментаинере
    // отдельно от потока). Стопка берёт собственный размер из `OOF_OWN`.
    {
        let own = fragment_size::constrain(own_h, &c.style, top + bot, cx.viewport, unclamp);
        OOF_OWN.with(|m| {
            let mut m = m.borrow_mut();
            if own < h - 0.01 {
                m.insert(c.node_id, (own, h));
            } else {
                m.remove(&c.node_id);
            }
        });
    }
    // css-gaps-1 §fragmentation / css-align-3 §column-row-gap: «the gap
    // disappears when it coincides with a fragmentation break»; Blink
    // `GridLayoutAlgorithm` `MaybeSuppressLastGap`: зазор рядов, в который
    // попал край фрагментаинера (внутрь, на начало или на конец), снимается —
    // следующий ряд начинается с верха следующего фрагмента, линейка в таком
    // зазоре не рисуется (`grid-gap-decorations-fragmentation-001…010`).
    // В стопке колонок это точка класса A с усечением: край внутри
    // `solid`-диапазона зазора уводит разрез к его началу (`fill`, `at(a)`),
    // а `cuts` с тем же `need` продолжает копию с КОНЦА зазора.
    // Keep the cut at the actual gutter start: fill_at's at(edge) handles
    // an exact start boundary. Moving it by a tolerance shortens the painted
    // fragment and can discard a device row. Only extend the end interval
    // for Blink's inclusive last_gap_end_offset >= fragmentainer_space check.
    for (a, b) in grid_row_gaps(&c.style, h - top - bot) {
        cuts.push((top + a, top + b));
        solid.push((top + a, top + b + 0.05));
    }
    // Принудительный разрыв элемента сетки — на границу его РЯДА
    // (css-grid-2 §Fragmenting Grid Layout; Blink `grid_layout_algorithm.cc`
    // `row_break_between`). Сетка без `grid_stack` спуска не знает, и до
    // этого места её `forced` был пуст ВСЕГДА: разрез шёл по краю колонки
    // (`flow.rs: fill_at`, ветка `Some(at(edge))`) —
    // `grid-item-fragmentation-044` резался на 100 при границе ряда 50
    // (снимок `target/wpt-shots/_fg-grid-item-fragmentation-044.png`:
    // красный прямоугольник `x 73..134, y 129..191`). Проба
    // `target/probe-fg/p-fg-044.html` (та же геометрия блоками, разрыв на
    // границе ряда) = 0.00.
    let (row_forced, row_mono) = grid_row_forced(c);
    for f in row_forced {
        forced.push(top + f);
    }
    // Монолитный ряд — целиком (`grid_row_forced`); разрез у его начала
    // растит предыдущую дорожку (`grow_grid_track`), и переполнение
    // элементов предыдущего ряда остаётся в колонке, как у Blink.
    for (a, e) in row_mono {
        solid.push((top + a, top + e));
    }
    // Внутренние принудительные разрывы монолита фрагментации не видны
    // (`forced_opaque`): `fill_at` проверяет `forced` РАНЬШЕ монолитности и
    // разрезал бы `contain: size`-коробку по разрыву её потомка.
    if forced_opaque(c) {
        forced.clear();
    }
    cuts.retain(|&(need, _)| need > 0.01 && need < h - 0.01);
    forced.retain(|&f| f > 0.01 && f < h - 0.01);
    // Монолит ребёнка за ЗАДАННОЙ высотой коробки — параллельный поток, а
    // не запрет разреза в её потоке (css-break-4 §parallel-flows; Blink
    // `FinishFragmentation`, `fragmentation_utils.cc`: «If the block-size is
    // constrained / fixed … we know that we're at the end» — сосед
    // продолжает в той же колонке; `BoxFragmentBuilder::
    // MustStayInCurrentFragmentainer`: «any first piece of child content
    // also needs to stay in the current fragmentainer, even if this causes
    // fragmentainer overflow»). Начатый ниже низа — вычёркивается, начатый
    // выше — бережётся лишь до низа. Без этого `contain: size` в
    // переполняющем ребёнке выталкивал коробку целиком (`single-line-
    // column-flex-fragmentation-051`, `tall-content-inside-constrained-
    // block-*`). У коробки с высотой auto диапазоны и так внутри `h`.
    solid.retain(|&(a, _)| a < h - 0.01);
    for r in solid.iter_mut() {
        r.1 = r.1.min(h);
    }
    bottom_edge_cuts(bot, &mut solid, stacked, h);
    Some((h, mt, mb, cuts, forced, solid))
}

pub(super) fn bottom_edge_cuts(
    bot: f32,
    solid: &mut Vec<(f32, f32)>,
    stacked: Option<(f32, f32, f32)>,
    h: f32,
) {
    if bot > 0.0 {
        // Между последним поточным ребёнком и нижней отбивкой/рамкой БЕЗ зазора
        // точки разрыва нет: класс C css-break-4 §possible-breaks существует лишь
        // «if there is a (non-zero) gap between them», а Blink держит там только
        // «a last-resort breakpoint before trailing border and padding»
        // (`fragmentation_utils.cc`, `FinishFragmentation`). Край колонки, упавший
        // на начало или внутрь нижней отбивки, обязан увести разрыв к НАЧАЛУ
        // монолита, который к ней примыкает (строка, `inline-block`,
        // `break-inside: avoid`), а не оставить отбивку одну в следующей колонке;
        // рост до низа колонки делает `grow_pushed` (срез на начале монолита).
        // `break-at-end-container-edge-000/001/004`: строки `inline-block` и
        // `padding-bottom` 50/70/60 — последняя строка уходит вместе с отбивкой;
        // `fieldset-005`: две коробки `break-inside: avoid` по 60 и `border-bottom:
        // 40px`. Диапазон из нуля — верхняя рамка самой коробки, его не клеим
        // (пустая коробка с отбивками стала бы монолитом). Нижнее поле последнего
        // ребёнка у колонок в `h` не входит (выше, `cx.paged`), при нём зазор есть
        // — тогда не клеим.
        let end_edge = h - bot;
        let gapless = stacked.is_none_or(|s| s.2.abs() < 0.01);
        let glue = if gapless {
            solid
                .iter()
                .filter(|&&(a, b)| a > 0.01 && (b - end_edge).abs() < 0.01)
                .map(|&(a, _)| a)
                .fold(end_edge, f32::min)
        } else {
            end_edge
        };
        solid.push((glue, h));
    }
}

/// Поле, которое мера схлопнула СКВОЗЬ верх коробки (`shape_full`: у первого
/// поточного блока без верхней отбивки и рамки `through = kmt`, и при высоте
/// `auto` оно уходит в `mt` коробки). Стопка кладёт это поле сама — `lead` в
/// `fill_at`, с усечением на разрыве (css-break-4 `margin-break: auto`: «any
/// margins adjoining the break … are truncated to zero after the break»). Копия
/// же ставится КОРНЕМ
/// (`layout_as_root`), а у корня taffy поля с детьми не схлопывает
/// (`vendor/taffy/src/compute/block.rs:186-192`, `vertical_margins_are_collapsible`
/// у корня ложно): поле внука вставало ВТОРОЙ раз внутри копии. На разрыве это
/// сдвигало содержимое на всё поле вниз: `margin-at-break-001/002` — `margin-top:
/// 60px` у первого внука, в колонке 2 зелёное 110..160 вместо 50..100 (снимок
/// `target/wpt-shots/margin-at-break-001.png`). Снимается ровно цепочка меры:
/// первый непустой ребёнок — поточный блок, у коробки нет верхней отбивки/рамки и
/// высота `auto`. Сетка, гибкий контейнер, таблица, вложенный многоколоночник
/// идут в мере иначе (`grid_rows_stack`, `row_nowrap`, `table_shape`) — не
/// трогаем. Внепоточный первым — у меры нулевая запись, `through` не рождается.
pub(crate) fn strip_through_top(c: &mut Element, depth: u8) {
    if depth == 0
        || c.style.height.is_some()
        || table_box(c)
        || multicol_container(&c.style)
        || c.style.webkit_box == Some(true)
        || !matches!(
            c.style.display,
            None | Some(Display::Block) | Some(Display::ListItem)
        )
    {
        return;
    }
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let b = c.style.borders();
    if px(&c.style.padding.top) + px(&b.top) != 0.0 {
        return;
    }
    let Some(Node::Element(k)) = c.children.iter_mut().find(|n| !is_blank(n)) else {
        return;
    };
    if k.inline
        || out_of_flow(&k.style)
        || !matches!(
            k.style.position,
            None | Some(crate::style::computed::Position::Relative)
        )
    {
        return;
    }
    k.style.margin.top = None;
    strip_through_top(k, depth - 1);
}
