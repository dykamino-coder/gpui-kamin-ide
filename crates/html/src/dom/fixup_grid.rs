//! Правки сетки: подсетка (дорожки родителя), fr в точки, лунки как сетка, абсолютные в сетке.

use crate::dom::*;
use crate::style::computed::{Computed, Display, Position};
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
fn subgrid_slot(
    place: &Option<(crate::style::computed::Placement, crate::style::computed::Placement)>,
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
fn subgrid_span(place: &Option<(crate::style::computed::Placement, crate::style::computed::Placement)>) -> usize {
    use crate::style::computed::Placement;
    match place {
        Some((Placement::Span(k), Placement::Auto)) | Some((Placement::Auto, Placement::Span(k))) => {
            (*k as usize).max(1)
        }
        _ => 1,
    }
}

/// Поправка среза на РАЗНИЦУ зазоров (css-grid-2 §subgrids).
///
/// Свой зазор у подсетки остаётся, но дорожка получает половину разницы
/// зазоров с каждой стороны, обращённой к ВНУТРЕННЕМУ стыку среза: три
/// эталона WPT выписывают результат числами (`grid-gap-larger-001-ref`
/// `70px 130px 70px` при родительских 100/190/100).
fn subgrid_gap_slice(
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
        || matches!(style.position, Some(Position::Absolute) | Some(Position::Fixed))
}

pub(super) fn subgrid_takes_parent_tracks(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        if matches!(
            el.style.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) {
            for row_dir in [false, true] {
                let raw = if row_dir {
                    el.style.grid_rows.clone()
                } else {
                    el.style.grid_tracks.clone()
                }
                .unwrap_or_default();
                // Доли `fr` при точечном размере родителя — в точки ДО нарезки
                // (`fr_tracks_to_px`). Такой срез годен только оси, где ребёнок
                // вправду подсеточный (проверка ниже, у ребёнка).
                let fr_px = fr_tracks_to_px(&el.style, &raw, row_dir);
                let from_fr = fr_px.is_some();
                let tracks = fr_px.unwrap_or(raw);
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v125/v126,
                // `scout-subgrid-2026-09.md` шаг 1): расширить гейт с «все
                // дорожки `Px`» до «все нарезаемы» (симметрично здесь и в
                // `render.rs`). Срез css-grid+css-gaps+css-contain 2123 общих:
                // +11/−8 с `fr` и +9/−7 без `fr`, причём потери грубые
                // (`subgrid-gap-decorations-003` 0.00 → 99.00 с `fr`,
                // `auto-track-sizing-001` → 12.78,
                // `row-subgrid-orthogonal-writing-mode-002` → 11.38). Сначала
                // нужен шаг 2 отчёта — снять фазовый разрыв между лунками и
                // обычной сеткой: в `grid-subgridded-to-grid-lanes/**` тест и
                // эталон отличаются одним словом разметки и идут разными
                // путями, поэтому односторонняя правка разводит пару.
                if tracks.is_empty()
                    || !tracks.iter().all(|t| {
                        matches!(
                            t,
                            crate::style::computed::TrackSize::Single(crate::style::computed::Track::Px(_))
                        )
                    })
                {
                    continue;
                }
                // Курсор авто-размещения (§8.5, разрежённая укладка): у
                // подсетки без начальной линии срез всё равно ЕСТЬ — она
                // встаёт в следующее свободное место своего пролёта. Курсор
                // ведут ВСЕ дети, а не только подсеточные: место занимает
                // каждый.
                let mut cur = 0usize;
                for child in el.children.iter_mut() {
                    let Node::Element(child) = child else { continue };
                    let place = if row_dir {
                        &child.style.grid_row
                    } else {
                        &child.style.grid_col
                    };
                    let slot = match subgrid_slot(place, tracks.len()) {
                        Some((at, span)) => {
                            cur = (at + span).min(tracks.len());
                            Some((at, span))
                        }
                        None => {
                            let span = subgrid_span(place);
                            if cur + span > tracks.len() {
                                cur = 0;
                            }
                            let at = cur;
                            cur = (cur + span).min(tracks.len());
                            (at + span <= tracks.len()).then_some((at, span))
                        }
                    };
                    if !child.style.subgrid {
                        continue;
                    }
                    // css-grid-2 §subgrid-listing: использованное значение у
                    // такого элемента — НАЧАЛЬНОЕ `none`, то есть не «срез не
                    // выдали», а «явных дорожек нет вовсе». Иначе слово
                    // `subgrid` доживает до раскладки счётной дорожкой:
                    // `count_tracks` считает его за одну.
                    if subgrid_inhibited(&child.style) {
                        if row_dir {
                            child.style.grid_rows = None;
                        } else {
                            child.style.grid_tracks = None;
                            child.style.grid_cols = None;
                        }
                        continue;
                    }
                    // Переведённые доли режутся только в ПАРАЛЛЕЛЬНУЮ ось, где
                    // написано `subgrid`: своя ось подсетки остаётся своей
                    // (`subgrid-gap-decorations-003`: ряды `subgrid`, колонки
                    // `repeat(2, 1fr)` — прежний откат с сырой долей давал 99.00).
                    let parallel = child.style.vertical.unwrap_or(false)
                        == el.style.vertical.unwrap_or(false);
                    if !subgrid_axes::linked(&el.style, &child.style, row_dir)
                        || (from_fr && !parallel)
                    {
                        continue;
                    }
                    let Some((at, span)) = slot else {
                        continue;
                    };
                    let slice: Vec<crate::style::computed::TrackSize> = (at..at + span)
                        .filter_map(|i| tracks.get(i).cloned())
                        .collect();
                    if slice.len() != span || span == 0 {
                        continue;
                    }
                    // Свои края подсетки вычитаются из первой и последней
                    // дорожки куска — ровно как в раскладке лунок.
                    let px = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let bs = child.style.borders();
                    let (lead, trail) = if row_dir {
                        (
                            px(child.style.margin.top) + px(bs.top) + px(child.style.padding.top),
                            px(child.style.margin.bottom)
                                + px(bs.bottom)
                                + px(child.style.padding.bottom),
                        )
                    } else {
                        (
                            px(child.style.margin.left) + px(bs.left) + px(child.style.padding.left),
                            px(child.style.margin.right)
                                + px(bs.right)
                                + px(child.style.padding.right),
                        )
                    };
                    let mut slice = slice;
                    if let Some(crate::style::computed::TrackSize::Single(crate::style::computed::Track::Px(
                        w,
                    ))) = slice.first_mut()
                    {
                        *w = (*w - lead).max(0.0);
                    }
                    if let Some(crate::style::computed::TrackSize::Single(crate::style::computed::Track::Px(
                        w,
                    ))) = slice.last_mut()
                    {
                        *w = (*w - trail).max(0.0);
                    }
                    // ЗАМЕРЕНО И ОТКАЧЕНО: брать зазор подсеточной оси у
                    // РОДИТЕЛЯ. Полный свод CSS3: приобретено 0, потеряно 3 —
                    // `grid-lanes-subgrid-001c` 0.03 -> 0.53, `-002c`
                    // 0.50 -> 0.56, `row-subgrid-grid-gap-005` 0.32 -> 0.56.
                    // Причина ОДНОСТОРОННОСТЬ, а не двойной счёт: гейт выше
                    // пропускает только обычную сетку, и во всех трёх парах
                    // тест написан на ЛУНКАХ (свой срез — `render.rs`), а
                    // эталон на сетке — стороны разъехались. Само правило
                    // тоже иное: при разнице зазоров дорожка получает половину
                    // разницы с каждой стороны внутреннего стыка, а свой зазор
                    // остаётся. Возвращаться симметрично обоим путям.
                    let (prow, pcol) = el.style.gap.unwrap_or((None, None));
                    let (crow, ccol) = child.style.gap.unwrap_or((None, None));
                    let par = if row_dir { prow } else { pcol };
                    let own = if row_dir { crow } else { ccol };
                    // Незаданный зазор подсетки — это `normal`, а он по
                    // css-grid-2 §subgrid-gaps значит «такие же зазоры, как у
                    // родителя», то есть разница НОЛЬ. Пока `None` считался
                    // нулём, разница выходила равной родительскому зазору и
                    // дорожки раздувались на его половину.
                    // Проверяется СВОЯ ось: первый проход (колонки) уже записал
                    // зазор колонок в `child.style.gap`, и прежнее условие «обе
                    // оси пусты» во втором проходе ложно — ряды подсетки
                    // оставались без зазора (`subgrid-gap-decorations-007`: ряды
                    // 0/100/200 вместо 0/110/220 при эталоне `grid-010-ref`).
                    let unset = own.is_none();
                    let own = own.or(par);
                    if own != Some(Len::Px(0.0)) && unset {
                        child.style.gap = Some(if row_dir {
                            (par, ccol)
                        } else {
                            (crow, par)
                        });
                        if !row_dir {
                            child.style.column_gap = par;
                        }
                    }
                    subgrid_gap_slice(&mut slice, par, own);
                    // В подсеточной оси SELF-выравнивание не действует:
                    // подсетка держит всю дорожку.
                    // css-grid-2 §subgrid-box-alignment: «The subgrid is
                    // always stretched in its subgridded dimension(s): the
                    // align-self/justify-self properties on it are ignored,
                    // as are any specified width/height constraints.»
                    //
                    // Гейт ПООСЕВОЙ (`subgrid_rows`/`subgrid_cols`), потому
                    // что срез приходит в ОБЕ оси, а гасить размер положено
                    // только в той, где вправду написано `subgrid`: иначе
                    // уходят пять зелёных `standalone-axis-size-*`.
                    //
                    // Ортогональную подсетку правило пропускает: `grid-template-
                    // rows` у неё — ось СВОЯ, и физическое свойство другое.
                    // Это отдельный корень (`scout-subgrid-orthogonal-2026-09`),
                    // трогать его здесь нельзя — три зелёных
                    // `row-subgrid-orthogonal-writing-mode-001/002/003`.
                    // ОРТОГОНАЛЬНАЯ подсетка: оси родителя и подсетки
                    // скрещены. `Computed` хранит дорожки ЛОГИЧЕСКИ
                    // (`apply::grid_style` переставляет их через `flip`), а
                    // подсеточная ось называется по шаблону САМОЙ подсетки
                    // (css-grid-2 §subgrid-listing): колонки родителя у
                    // подсетки с другим письмом — это её РЯДЫ, ряды родителя
                    // — её колонки. Blink пишет то же (`grid/grid_item.cc`:
                    // `has_subgridded_columns = is_parallel_with_root_grid ?
                    // GridTemplateColumns() : GridTemplateRows()`). Прежде
                    // срез колонок ложился в колонки подсетки (после `flip` —
                    // в ФИЗИЧЕСКИЕ ряды), а §subgrid-box-alignment
                    // («always stretched … any specified width/height
                    // constraints» игнорируются) у ортогональной подсетки не
                    // делался вовсе: вторая половина `subgrid/subgrid-stretch`
                    // (восемь коробок `vrl`, 16.23) держала свои 50/150 вместо
                    // дорожки 100. Размер гасится ФИЗИЧЕСКИЙ: ряды
                    // горизонтального родителя — высота, колонки — ширина.
                    // Разница зазоров по-прежнему пишется в оси родителя —
                    // отдельный шаг.
                    if !parallel {
                        let own = if row_dir {
                            child.style.subgrid_cols
                        } else {
                            child.style.subgrid_rows
                        };
                        let vertical_axis = row_dir != el.style.vertical.unwrap_or(false);
                        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (b47ecf2): класть СРЕЗ в скрещенную
                        // ось (колонки родителя → `grid_rows` подсетки): +1/−2,
                        // ушли `grid-subgridded-to-grid-lanes/track-sizing/
                        // {column,row}-subgrid-auto-fill-007` — тест там на
                        // ЛУНКАХ (срез режет `render.rs`, оси не скрещивает),
                        // эталон — та же разметка на `inline grid` (режет этот
                        // проход). Скрещиваются только признак и растяжка ниже;
                        // `subgrid-stretch` срезу безразличен (обе оси по 100).
                        // Возвращать вместе со скрещиванием в `render.rs` (Blink
                        // `grid_item.cc:192-207`) и замером лунковых пар.
                        if row_dir {
                            child.style.grid_rows = Some(slice);
                            child.style.align_self = None;
                        } else {
                            child.style.grid_tracks = Some(slice);
                            child.style.grid_cols = Some(span as u16);
                            child.style.justify_self = None;
                        }
                        if own {
                            if vertical_axis {
                                child.style.height = None;
                                child.style.max_height = None;
                                child.style.min_height = Some(Len::Px(0.0));
                            } else {
                                child.style.width = None;
                                child.style.max_width = None;
                                child.style.min_width = Some(Len::Px(0.0));
                            }
                            let stretch = Some(crate::style::computed::Align::Stretch);
                            if row_dir {
                                child.style.align_self = stretch;
                            } else {
                                child.style.justify_self = stretch;
                            }
                        }
                    } else if row_dir {
                        child.style.grid_rows = Some(slice);
                        child.style.align_self = None;
                        if parallel && child.style.subgrid_rows {
                            child.style.height = None;
                            child.style.max_height = None;
                            // Не `None`, а НОЛЬ: `None` вернул бы автоминимум
                            // элемента сетки, и подсетка раздулась бы шире
                            // своей области. Спека требует «размер
                            // игнорируется», а не «минимум по содержимому».
                            child.style.min_height = Some(Len::Px(0.0));
                            child.style.align_self = Some(crate::style::computed::Align::Stretch);
                        }
                    } else {
                        child.style.grid_tracks = Some(slice);
                        child.style.grid_cols = Some(span as u16);
                        child.style.justify_self = None;
                        if parallel && child.style.subgrid_cols {
                            child.style.width = None;
                            child.style.max_width = None;
                            child.style.min_width = Some(Len::Px(0.0));
                            child.style.justify_self = Some(crate::style::computed::Align::Stretch);
                        }
                    }
                }
            }
        }
        subgrid_takes_parent_tracks(&mut el.children);
    }
}

/// Доли `fr` родительской сетки в точках — для среза в ПОДСЕТКУ.
///
/// css-grid-2 §subgrids: подсетка получает ИСПОЛЬЗОВАННЫЕ размеры дорожек
/// родителя. Сырую долю резать нельзя: у подсетки она разрешается заново
/// против её собственного неопределённого размера (откат v125/v126 в
/// `subgrid_takes_parent_tracks`). Здесь доля переводится в точки по размеру
/// САМОГО родителя — css-grid-1 §12.7.1 «Find the Size of an fr»: остаток
/// после точечных дорожек и зазоров делится на сумму долей, но не меньше
/// единицы. Гейт: размер оси и зазор — точки (или зазор не задан), все
/// дорожки — точки или доли, хотя бы одна доля; иначе `None`. Рост доли под
/// содержимое (`minmax(auto, 1fr)`) здесь не виден — у пар семьи элементы пустые.
fn fr_tracks_to_px(
    style: &Computed,
    tracks: &[crate::style::computed::TrackSize],
    row_dir: bool,
) -> Option<Vec<crate::style::computed::TrackSize>> {
    use crate::style::computed::{Track, TrackSize};
    let mut fr_sum = 0.0f32;
    let mut px_sum = 0.0f32;
    for t in tracks {
        match t {
            TrackSize::Single(Track::Fr(f)) => fr_sum += *f,
            TrackSize::Single(Track::Px(v)) => px_sum += *v,
            _ => return None,
        }
    }
    if fr_sum <= 0.0 {
        return None;
    }
    let size = match if row_dir { style.height } else { style.width } {
        Some(Len::Px(v)) => v,
        _ => return None,
    };
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        None => Some(0.0),
        _ => None,
    };
    // `box-sizing: border-box` — заданный размер включает поля и рамку.
    let inner = if style.border_box == Some(true) {
        let b = style.borders();
        let (p0, p1, b0, b1) = if row_dir {
            (style.padding.top, style.padding.bottom, b.top, b.bottom)
        } else {
            (style.padding.left, style.padding.right, b.left, b.right)
        };
        size - px(p0)? - px(p1)? - px(b0)? - px(b1)?
    } else {
        size
    };
    let (grow, gcol) = style.gap.unwrap_or((None, None));
    let gap = px(if row_dir { grow } else { gcol })?;
    let n = tracks.len() as f32;
    let leftover = (inner - px_sum - gap * (n - 1.0)).max(0.0);
    let per = leftover / fr_sum.max(1.0);
    Some(
        tracks
            .iter()
            .map(|t| match t {
                TrackSize::Single(Track::Fr(f)) => TrackSize::Single(Track::Px(f * per)),
                other => other.clone(),
            })
            .collect(),
    )
}

/// Абсолютный ПОТОМОК сетки размещается по её линиям, а не по статической
/// позиции: если содержащий блок такого элемента — сама сетка, то `grid-row`
/// и `grid-column` задают ему прямоугольник области (css-grid-2 §9). Раскладка
/// знает только ПРЯМЫХ детей сетки, поэтому потомок поднимается к ней. Стиль к
/// этому моменту уже вычислен, и переезд по дереву его не меняет.
/// Ось лунок контейнера: `true` — лунки РЯДАМИ (ось решётки — ряды).
///
/// Без явного `grid-lanes-direction` направление выдаёт ТА ОСЬ, по которой
/// объявлены дорожки — то же правило, что у `render::lanes`.
pub(crate) fn lanes_row_dir(s: &Computed) -> bool {
    let row_tracks = s.grid_rows.is_some() || s.auto_repeat_rows.is_some() || s.grid_auto_fill_row.is_some();
    let col_tracks = s.grid_tracks.is_some() || s.auto_repeat_cols.is_some() || s.grid_auto_fill_min.is_some();
    s.lanes_row.unwrap_or(row_tracks && !col_tracks)
}

/// Контейнер лунок — на путь СЕТКИ, раскладку лунками делает taffy
/// (`vendor/taffy/src/compute/grid/lanes.rs`, css-grid-3).
///
/// css-grid-3 §grid-lanes-track-templates (Overview.bs:414-433): по оси
/// решётки «the full power of grid layout is available» — шаблоны, линии,
/// области, явная и неявная сетка «formed in the same way as for a regular
/// grid container», а дорожки размеряются алгоритмом css-grid-2 §12
/// (Overview.bs:619-669). Поэтому контейнер становится обычной сеткой с
/// пометкой `lanes_taffy`: шаблоны, зазоры, выравнивание и дети идут ТЕМ ЖЕ
/// путём, что у сетки-эталона (`grid-subgridded-to-grid-lanes/**` — та же
/// разметка на `inline-grid`), а не рукописной оценкой `render::lanes`.
///
/// Все контейнеры лунок идут сюда: вертикальное письмо — осями из
/// `apply.rs` (`grid_style`, `placement_flip`), `rtl` — как у сетки-эталона
/// (зеркала строчной оси у сетки taffy нет, и эталоны `inline-grid` с `rtl`
/// рисуются тем же путём; ★ ЗАМЕРЕНО: 57 пар лунок с `rtl` +3/−0). Подсетки среди
/// детей идут тем же путём: срез им режет `subgrid_takes_parent_tracks`
/// ровно как у сетки-эталона; интрин-дорожки в `repeat(auto-*)` считает
/// taffy (css-grid-3 §7.2.1).
pub(super) fn lanes_as_grid(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        lanes_as_grid(&mut el.children);
        if el.style.display != Some(Display::GridLanes) {
            continue;
        }
        lanes_to_grid(&mut el.style);
    }
}

/// Перевод контейнера лунок на путь сетки (см. `lanes_as_grid`).
pub(crate) fn lanes_to_grid(style: &mut Computed) {
    style.display = Some(if style.lanes_inline {
        Display::InlineGrid
    } else {
        Display::Grid
    });
    style.lanes_taffy = true;
}

pub(super) fn hoist_grid_abspos(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        hoist_grid_abspos(&mut el.children);
        if !is_grid(&el.style) || !own_containing_block(&el.style) {
            continue;
        }
        let mut taken = vec![];
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if own_containing_block(&child.style) || is_grid(&child.style) {
                continue;
            }
            steal_placed(&mut child.children, &mut taken);
        }
        el.children.extend(taken);
    }
}

/// Забрать из поддерева абсолютные элементы с заданными линиями сетки.
fn steal_placed(children: &mut Vec<Node>, out: &mut Vec<Node>) {
    let mut kept = Vec::with_capacity(children.len());
    for mut node in children.drain(..) {
        if let Node::Element(el) = &mut node {
            let placed = el.style.grid_col.is_some() || el.style.grid_row.is_some();
            if el.style.position == Some(Position::Absolute) && placed {
                out.push(node);
                continue;
            }
            // Свой содержащий блок — дальше уже чужие абсолютные элементы.
            // Останавливает и ПОДСЕТКА: она размещает своих абсолютных детей
            // сама, и счёт с конца (`grid-column: 3 / -1`) идёт по её
            // собственному числу дорожек (`subgrid/abs-pos-001`). А вот обычная
            // вложенная сетка без своего отсчёта помехой не служит: её потомок
            // по-прежнему принадлежит внешней сетке (css-grid-2 §9).
            if !own_containing_block(&el.style) && !el.style.subgrid {
                steal_placed(&mut el.children, out);
            }
        }
        kept.push(node);
    }
    *children = kept;
}

/// Контейнер сетки — это и `inline-grid`: разница только в том, как коробка
/// встаёт в поток снаружи.
fn is_grid(c: &Computed) -> bool {
    matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid))
}
