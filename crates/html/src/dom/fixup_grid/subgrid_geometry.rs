//! Subgrid geometry for fixup_grid; split out to keep the owning module within 250 lines.

use crate::style::computed::{Computed, Position};
use crate::style::values::value::Len;

/// Ребёнок гибкого контейнера или сетки не плавает и не очищает.
///
/// css-flexbox-1 §4: «`float` and `clear` do not create floating or clearance
/// for flex item», то же в css-grid-2 §6 для элемента сетки — обе величины
/// вычисляются в `none` у элемента В ПОТОКЕ (абсолютный ребёнок элементом
/// контейнера не является и правило его не касается).
///
/// Без этого правило §10.6.3 «блок из одних флоатов высотой ноль» считало
/// гибкий контейнер пустым и обнуляло его высоту (`flex-box-wrap` и родня).
/// Начало и длина пролёта подсетки в дорожках родителя.
///
/// Только явные формы: у подсетки без размещения среза нет, и трогать её
/// нельзя — авто-размещение считает уже раскладка.
pub(super) fn subgrid_slot(
    place: &Option<(
        crate::style::computed::Placement,
        crate::style::computed::Placement,
    )>,
    count: usize,
) -> Option<(usize, usize)> {
    use crate::style::computed::Placement;
    let line = |n: i16| -> usize {
        if n > 0 {
            (n as usize - 1).min(count.saturating_sub(1))
        } else {
            count.saturating_sub((-n) as usize)
        }
    };
    // Конечная линия — это КРАЙ, а не дорожка: у сетки из N дорожек линий
    // N+1, и потолок у неё `count`, а не `count - 1`. С общим потолком
    // подсетка, упирающаяся в последнюю линию родителя, теряла дорожку:
    // `grid-column: 2 / 5` при четырёх колонках давало пролёт 2 вместо 3, а
    // проверка длины среза этого не ловит.
    let edge = |n: i16| -> usize {
        if n > 0 {
            (n as usize - 1).min(count)
        } else {
            count.saturating_sub(((-n) as usize).saturating_sub(1))
        }
    };
    match place {
        Some((Placement::Line(a), Placement::Line(b))) => {
            let (s, t) = (line(*a), edge(*b));
            Some((s.min(t), (t as i32 - s as i32).unsigned_abs() as usize))
        }
        Some((Placement::Line(a), Placement::Span(k))) => Some((line(*a), *k as usize)),
        Some((Placement::Line(a), Placement::Auto)) => Some((line(*a), 1)),
        Some((Placement::Span(k), Placement::Line(b))) => {
            Some((edge(*b).saturating_sub(*k as usize), *k as usize))
        }
        _ => None,
    }
}

/// Пролёт подсетки, когда НАЧАЛЬНОЙ линии нет: `span k`, голое `auto` и
/// отсутствие записи вовсе. Начало такой подсетки знает только
/// авто-размещение, а сколько дорожек она занимает — видно сразу
/// (css-grid-2 §subgrid-size-contribution: число дорожек авто-размещённой
/// подсетки берётся из её пролёта).
pub(super) fn subgrid_span(
    place: &Option<(
        crate::style::computed::Placement,
        crate::style::computed::Placement,
    )>,
) -> usize {
    use crate::style::computed::Placement;
    match place {
        Some((Placement::Span(k), Placement::Auto))
        | Some((Placement::Auto, Placement::Span(k))) => (*k as usize).max(1),
        _ => 1,
    }
}

/// Поправка среза на РАЗНИЦУ зазоров (css-grid-2 §subgrids).
///
/// Свой зазор у подсетки остаётся, но дорожка получает половину разницы
/// зазоров с каждой стороны, обращённой к ВНУТРЕННЕМУ стыку среза: три
/// эталона WPT выписывают результат числами (`grid-gap-larger-001-ref`
/// `70px 130px 70px` при родительских 100/190/100).
pub(super) fn subgrid_gap_slice(
    slice: &mut [crate::style::computed::TrackSize],
    parent: Option<Len>,
    own: Option<Len>,
) {
    use crate::style::computed::{Track, TrackSize};
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let d = px(parent) - px(own);
    let n = slice.len();
    if d == 0.0 || n < 2 {
        return;
    }
    for (i, t) in slice.iter_mut().enumerate() {
        let sides = u8::from(i > 0) + u8::from(i + 1 < n);
        if let TrackSize::Single(Track::Px(w)) = t {
            // Разница зазоров — «extra layer of (potentially negative)
            // margin» (css-grid-2 §subgrid-item-gaps): дорожка бывает и
            // ОТРИЦАТЕЛЬНОЙ (`grid-gap-011-ref`: 25 / −50 / 25); Blink
            // `accumulated_gutter_size_delta_` пола не имеет.
            *w += d / 2.0 * f32::from(sides);
        }
    }
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v164/v165, `scout-grid-2026-09g.md`, 6 хунков):
/// поосевые признаки `subgrid_rows`/`subgrid_cols` в `Computed`/`dom.rs`/
/// `render.rs` + мост `TaffyLayoutEngine::grid_track_sizes` (KaminIDE patch).
/// Обещание +2…+4. Срез из 63 пар (v165, только этот патч и `<canvas>`):
/// +2 (`column-line-names-014`, `row-line-names-014`) / −3
/// (`column-auto-placed-subgrid-inherited-tracks-001` и
/// `-nested-subgrid-inherited-tracks-001` → «красное видно»,
/// `-inherited-tracks-003` 0.00 → 2.08). Поосевой признак без второго прохода
/// по разрешённым дорожкам ломает наследование дорожек у авто-размещённой
/// подсетки. Возвращать только вместе со вторым проходом (GRID-SUBGRID-TRACKS).
/// Дорожки родительской сетки — вниз, в ПОДСЕТКУ (css-grid-2 §subgrids).
///
/// Своих дорожек в подсеточной оси у подсетки нет: она берёт СРЕЗ
/// родительских по своему пролёту. У раскладки лунок это уже сделано
/// (`render::lanes`), а обычная сетка разбирала `grid-template-columns:
/// subgrid` как «одну колонку» (`count_tracks` считает слово дорожкой), и
/// тест с эталоном гоняли одно свойство разным кодом.
///
/// Обход СВЕРХУ ВНИЗ: вложенная подсетка обязана увидеть уже проставленные
/// дорожки внешней.
/// Подсеточность ЗАПРЕЩЕНА независимым контекстом форматирования.
///
/// css-grid-2 §subgrid-listing: «If there is no parent grid, or if the grid
/// container is otherwise forced to establish an independent formatting
/// context (for example, due to layout containment [CSS-CONTAIN-2] or
/// absolute positioning [CSS-POSITION-3]), the used value is the initial
/// value, `none`, and the grid container is not a subgrid.»
///
/// Список ровно тот же, что у Blink (`chromium-blink/third_party/blink/
/// renderer/core/layout/grid/grid_item.cc:189-191`): обособление РАСКЛАДКИ,
/// обособление ОТРИСОВКИ и контейнер запросов размера — плюс внепоточность.
/// `contain: strict` и `contain: content` сюда попадают сами: разбор
/// `computed.rs` раскрывает их в `layout`+`paint`.
///
/// Чего в списке НЕТ и быть не должно:
/// * `overflow: hidden|scroll` — css-grid-2 §subgrid-overflow прямо разрешает
///   прокручиваемую подсетку (`overflow-hidden-does-not-prohibit-subgrid`,
///   две пары корпуса);
/// * `contain: size` и `contain: style` в одиночку — случаи 8 и 9
///   `independent-formatting-context.html` требуют, чтобы подсетка ОСТАЛАСЬ;
/// * `<fieldset>` и `<button>` — `independent-formatting-context-fieldset`
///   (0.00) проверяет, что они ГОДНЫЕ подсетки.
pub(crate) fn subgrid_inhibited(style: &Computed) -> bool {
    style.contain_layout == Some(true)
        || style.contain_paint == Some(true)
        || style.container_size_query
        || matches!(
            style.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        )
}
