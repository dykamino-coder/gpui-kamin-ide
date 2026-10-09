//! Раскладка: display/flex/позиционирование/размеры (apply_layout).

use crate::style::apply::*;
use crate::style::computed::{
    Align, Computed, Display, FlexDir, Justify, Overflow, Placement, Position,
};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px};

pub(super) fn apply_layout(mut d: Div, c: &Computed) -> Div {
    // Вертикальная `-webkit-box` с действующим `continue` (`line-clamp`,
    // `-webkit-line-clamp`) вычисляется в `flow-root` (css-overflow-4
    // §line-clamp, «the computed value becomes flow-root and the box
    // establishes a BFC»): это блочный контейнер, и `align-items` /
    // `justify-content` гибкой коробки к нему не применяются
    // (`line-clamp-017/018`, `webkit-line-clamp-045`: анонимная строка
    // вставала по центру).
    let legacy_block;
    let c = if c.webkit_box == Some(true)
        && c.webkit_box_vertical == Some(true)
        && (c.line_clamp.is_some() || c.clamp_auto == Some(true))
        && (c.align_items.is_some() || c.justify_content.is_some())
    {
        let mut b = c.clone();
        b.align_items = None;
        b.justify_content = None;
        legacy_block = b;
        &legacy_block
    } else {
        c
    };
    d.style().block_flow = Some(grid_flow_axes::block(c));
    match c.display {
        // Блок в GPUI — дефолт; отдельного вызова не требует.
        Some(Display::Flex) | Some(Display::InlineFlex) => d = d.flex(),
        // Инлайновая коробка в строке не растягивается по ширине родителя.
        // Базовая `inline-block` — ПОСЛЕДНЕЙ строки (CSS 2.1 §10.8.1: «the
        // baseline of its last line box in the normal flow»; css-inline-3
        // `baseline-source: auto` → `last` у `inline-block`). При обрезке —
        // нижний край margin-бокса: адаптер подавляет содержательную базовую
        // и родитель синтезирует её, сохраняя нижнее поле.
        Some(Display::InlineBlock) => {
            d = d.flex_shrink_0();
            // Элемент гибкого контейнера и сетки блокифицирован (css-display-3
            // §2.7): его базовая — первая, как у блока.
            // Руби и строчная коробка, сыгранная `inline-block`
            // (`inline_display`), — не атомы: их базовая — базовая основы
            // (★ ЗАМЕРЕНО: `initial-letter-block-position-raise-over/under-ruby`
            // 0.24 → 1.25 / 0.25 → 0.60).
            // Keep the inline-block boundary flag for non-visible overflow too:
            // native output adaptation then suppresses its content baseline.
            if !c.parent_flex_grid && c.ruby_role.is_none() && c.inline_display != Some(true) {
                d.style().baseline_from_last = Some(true);
            }
        }
        Some(Display::InlineGrid) => {
            d = d.flex_shrink_0();
            d = grid_style(d, c);
        }
        Some(Display::TableRow) => d = d.flex().flex_row(),
        // Ячейка ведёт себя как блок; саму решётку строит контейнер.
        Some(Display::TableCell) => d = d.flex().flex_col(),
        // Контейнер таблицы собирается отдельной веткой сборки дерева.
        Some(Display::Table) | Some(Display::InlineTable) => {}
        Some(Display::Grid) => d = grid_style(d, c),
        // `display: none` отсеивается ещё при разборе дерева: узел не строится.
        _ => {}
    }
    // `direction: rtl` переворачивает главную ось и выравнивание по
    // умолчанию: ряд идёт справа налево, текст прижимается вправо.
    if c.rtl == Some(true) {
        // Разворот главной оси — дело ТОЛЬКО гибкого ряда: обычный блок
        // собирается колонкой, и разворот переставлял его детей снизу вверх.
        // Ряд с явно заданным направлением разворачивается ниже, вместе с
        // остальными случаями.
        let default_row = c.flex_dir.is_none()
            && matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex));
        if default_row {
            d = d.flex_row_reverse();
        }
        if c.text_align.is_none() {
            d = d.text_right();
        }
    }
    // При вертикальном письме оси меняются местами: `row` — это ось СТРОКИ,
    // а она идёт сверху вниз; `column` — ось потока, справа налево
    // (`vertical-rl`) или слева направо (`vertical-lr`).
    let dir = match (c.flex_dir, c.vertical == Some(true)) {
        // Ось СТРОКИ при вертикальном письме идёт сверху вниз, а
        // `direction: rtl` разворачивает её снизу вверх — как и в обычном
        // письме он разворачивает строку справа налево
        // (`flexbox-writing-mode-005`).
        (Some(d), true) => Some(match (d, c.vertical_rl == Some(true)) {
            (FlexDir::Row, _) if c.rtl == Some(true) => FlexDir::ColReverse,
            (FlexDir::RowReverse, _) if c.rtl == Some(true) => FlexDir::Col,
            (FlexDir::Row, _) => FlexDir::Col,
            (FlexDir::RowReverse, _) => FlexDir::ColReverse,
            (FlexDir::Col, true) => FlexDir::RowReverse,
            (FlexDir::Col, false) => FlexDir::Row,
            (FlexDir::ColReverse, true) => FlexDir::Row,
            (FlexDir::ColReverse, false) => FlexDir::RowReverse,
        }),
        // Умолчание `flex-direction: row` в стиле НЕ записано, а ось менять
        // всё равно надо: в вертикальном письме строка идёт сверху вниз,
        // значит главная ось гибкого ряда — вертикальная. Пока сюда попадало
        // `None`, контейнер оставался горизонтальным (замерено пробой: ряд в
        // `vertical-rl` против колонки с прижимом вправо — 5.42%).
        (None, true) if matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex)) => {
            if c.rtl == Some(true) {
                Some(FlexDir::ColReverse)
            } else {
                Some(FlexDir::Col)
            }
        }
        (d, _) => d,
    };
    // `sideways-lr`: строчная ось идёт СНИЗУ вверх (css-writing-modes-4
    // §block-flow) — вертикальные результаты перевода осей разворачиваются.
    // Горизонтальные (из `column`) не трогаются: ось блока у slr обычная,
    // слева направо.
    let dir = if c.vertical == Some(true) && c.sideways == Some(true) && c.vertical_rl != Some(true)
    {
        dir.map(|d| match d {
            FlexDir::Col => FlexDir::ColReverse,
            FlexDir::ColReverse => FlexDir::Col,
            other => other,
        })
    } else {
        dir
    };
    // Разворот по `direction: rtl` — только для обычного письма: при
    // вертикальном он уже учтён в переводе осей выше, и второй раз
    // переворачивать нельзя (`flexbox-writing-mode-005`: колонки в
    // `vertical-rl` шли слева направо).
    let rtl_row = c.rtl == Some(true) && c.vertical != Some(true);
    match dir {
        Some(FlexDir::Row) if rtl_row => d = d.flex_row_reverse(),
        Some(FlexDir::RowReverse) if rtl_row => d = d.flex_row(),
        Some(FlexDir::Row) => d = d.flex_row(),
        Some(FlexDir::RowReverse) => d = d.flex_row_reverse(),
        Some(FlexDir::Col) => d = d.flex_col(),
        Some(FlexDir::ColReverse) => d = d.flex_col_reverse(),
        None => {}
    }
    // Однострочная гибкая КОЛОНКА при `direction: rtl` (горизонтальное
    // письмо): cross-start — inline-start письма, то есть ПРАВЫЙ край
    // (css-flexbox-1 §2, §9.6). Раскладка поперёк идёт от физического
    // начала, разворот ей сообщает флаг (`flex_cross_reverse`, taffy
    // `compute_constants`); многострочной колонке то же делает `flip` ниже.
    // Вертикальное письмо сюда не входит: там прижим держит `items_end`
    // ниже (записи откатов 04.09).
    if c.rtl == Some(true)
        && c.vertical != Some(true)
        && c.flex_wrap != Some(true)
        && c.webkit_box != Some(true)
        && matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex))
        && matches!(dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
    {
        d.style().flex_cross_reverse = Some(true);
    }
    if c.flex_wrap == Some(true) {
        d = d.flex_wrap();
        // При `vertical-rl` поперечная ось строки идёт справа налево — обратно
        // тому, как переносит колонку раскладка. Перенос разворачивается,
        // иначе вторая строка уходит не в ту сторону.
        // Поперечная ось — та, что осталась. У РЯДА (ось строки) это ось
        // потока: она разворачивается при `vertical-rl`. У КОЛОНКИ (ось
        // потока) поперечная — ось строки, и её разворачивает `direction: rtl`
        // (`flexbox-writing-mode-004/005`: контейнеры-колонки шли зеркально).
        // `sideways-lr` — единственное письмо, где строчная ось идёт СНИЗУ
        // ВВЕРХ (css-writing-modes-4 §3.1), поэтому поперечная ось колонки
        // разворачивается им так же, как `direction: rtl`.
        let inline_reversed =
            (c.rtl == Some(true)) != (c.sideways == Some(true) && c.vertical_rl != Some(true));
        let flip = if matches!(c.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse)) {
            inline_reversed
        } else {
            c.vertical_rl == Some(true)
        };
        if (c.flex_wrap_reverse == Some(true)) != flip {
            d.style().flex_wrap = Some(gpui::FlexWrap::WrapReverse);
        }
        // `flex-wrap: balance` — балансировщик строк в раскладке;
        // `flex-line-count` (умолчание 1) — минимум строк. У legacy
        // `-webkit-box` balance не действует (Blink `IsDeprecatedFlexbox`).
        if c.flex_balance == Some(true) && c.webkit_box != Some(true) {
            d.style().flex_balance_lines = Some(c.flex_line_count.unwrap_or(1).max(1));
        }
    }
    // `calc-size()` (css-values-5 §calc-size): выражение над размером основы
    // применяет раскладка (`vendor/taffy` — `Style::calc_size`).
    if c.calc_size.iter().any(Option::is_some) {
        d.style().calc_size = Some(c.calc_size);
    }
    // `margin-trim` гибкого контейнера и сетки (css-box-4 §margin-trim):
    // раскладка физическая, поэтому логические края переводятся ЗДЕСЬ, по
    // письму самого контейнера. Биты раскладки: 1 верх, 2 право, 4 низ,
    // 8 лево (`flex-*-trimmed-only`, `flex-*-multiline`, `grid-*-start`).
    if c.margin_trim != 0
        && matches!(
            c.display,
            Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::Grid)
                | Some(Display::InlineGrid)
        )
    {
        let vertical = c.vertical == Some(true);
        let (block_start, block_end) = match (vertical, c.vertical_rl == Some(true)) {
            (false, _) => (1u8, 4u8),
            (true, true) => (2, 8),
            (true, false) => (8, 2),
        };
        let (inline_start, inline_end) = match (vertical, c.rtl == Some(true)) {
            (false, false) => (8u8, 2u8),
            (false, true) => (2, 8),
            (true, false) => (1, 4),
            (true, true) => (4, 1),
        };
        let mut physical = 0u8;
        for (bit, side) in [
            (1u8, block_start),
            (2, block_end),
            (4, inline_start),
            (8, inline_end),
        ] {
            if c.margin_trim & bit != 0 {
                physical |= side;
            }
        }
        d.style().margin_trim = Some(physical);
    }
    if let Some(names) = grid_line_names(c) {
        d.style().grid_line_names = Some(Box::new(names));
    }
    if c.grid_col.is_some() || c.grid_row.is_some() {
        let span = |p: Option<(Placement, Placement)>| {
            let (a, b) = p.unwrap_or((Placement::Auto, Placement::Auto));
            to_placement(a)..to_placement(b)
        };
        // Грани элемента ЛОГИЧЕСКИЕ (css-grid-2 §8.3: `grid-row` — блочная
        // ось сетки), а дорожки вертикальной сетки уже переставлены
        // (`grid_style`, `flip`): ряды — физические колонки.
        let (row, column) = if placement_flip(c) {
            (span(c.grid_col), span(c.grid_row))
        } else {
            (span(c.grid_row), span(c.grid_col))
        };
        d.style().grid_location = Some(gpui::GridLocation { row, column });
    }
    if let Some(g) = c.flex_grow {
        // `flex_grow()` в GPUI ставит жёсткую единицу, а `flex: 2` встречается —
        // пишем значение в стиль напрямую.
        d.style().flex_grow = Some(g);
    }
    if let Some(shrink) = c.flex_shrink {
        // Вес сжатия — число, а не флаг: при `flex-shrink: 1` и `3` соседи
        // ужимаются в отношении 1:3. Через `flex_shrink()` доезжала единица,
        // и оба сжимались поровну (поймано сравнением с Chrome).
        d.style().flex_shrink = Some(shrink);
    }
    // The legacy cross-end default projects onto X only for physical columns.
    // A logical vertical column has a Y cross axis and retains its stretch default.
    if flex_cross_default::ends_on_x(c, dir) {
        d = d.items_end();
    }
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (повторно, теперь узко): зеркало поперечной оси у
    // гибкой КОЛОНКИ при `direction: rtl` (css-flexbox-1 §5.1: cross-start
    // колонки — inline-start письма, то есть правый край) — умолчание
    // `items_end` и зеркало явных `start/end` у `align-items` и `align-self`
    // детей (флаг `cross_mirror` из `inline::inherit`). Срез из 15 пар
    // (`flexbox_rtl-direction`, `flexbox-align-self-vert-rtl-002..005`,
    // `flexbox-align-self-vert-002`, `flexbox-align-self-horiz-002`,
    // `gap-001-rtl` + заложники `text-orientation-*-100`): 6 -> 6, ноль
    // сдвигов. Те пары держит другое (у `flexbox_rtl-direction` расходятся
    // поля и высота коробки, а не сторона прижима).
    // Возвращено 02.10 иначе — флагом раскладки `flex_cross_reverse` выше
    // (разворачивает и растяжение/базовую линию, а не только `start/end`),
    // вместе с гашением авторского `align-self` у блока (`dom.rs`), словом
    // `inherit`, комментариями XHTML и физическим рядом флоатов: срез 10511
    // пар против свода v229 — +11/−1 (`flexbox_rtl-direction`,
    // `-align-self-vert-001/002`, `-vert-rtl-001` и др.; потеря
    // `flexbox-writing-mode-013` — у эталона блок `span` с шириной в rtl
    // стоит слева, сам тест теперь верен).
    match c.align_items {
        Some(Align::Center) => d = d.items_center(),
        Some(Align::Start) => d = d.items_start(),
        Some(Align::End) => d = d.items_end(),
        // `last baseline` — своя группа с прижимом к концу (css-align-3 §9.3).
        // Не у лунок: их дорожки — гибкие ряды движка, и прижим к концу уводил
        // лунки целиком (★ ЗАМЕРЕНО: `row-grid-lanes-item-baseline-001/003`
        // 0.00 → 8.02/7.56, `column-fill-reverse-justify-items-002` 0.00 → 3.87).
        Some(Align::Baseline)
            if c.align_items_last && c.display != Some(Display::GridLanes) && !c.parent_lanes =>
        {
            d.style().align_items = Some(gpui::AlignItems::LastBaseline);
        }
        Some(Align::Baseline) => d = d.items_baseline(),
        // `anchor-center` у `align-items` спекой не предусмотрен — как не задано.
        Some(Align::Stretch) | Some(Align::AnchorCenter) | None => {}
    }
    // Приставка `safe` (css-align-3 §4.4): при переполнении области
    // выравнивание падает к началу, иначе содержимое уезжает за край и
    // становится недоступным. Раскладка читает это из стиля
    // (`flexbox-safe-overflow-position-*`).
    // Лунки решают `safe` сами (`render.rs`: раздача не ставится, когда лунка
    // переполнена) — второй заход в раскладке им мешает (★ ЗАМЕРЕНО:
    // `grid-lanes-justify-content-001` 0.00 -> 1.37).
    if c.display != Some(Display::GridLanes)
        && (c.align_items_safe
            || c.align_self_safe
            || c.align_content_safe
            || c.justify_content_safe
            || c.justify_items_safe
            || c.justify_self_safe)
    {
        d.style().safe_alignment = Some((
            c.align_items_safe,
            c.align_self_safe,
            c.align_content_safe,
            c.justify_content_safe,
        ));
        d.style().safe_justify_alignment = Some((c.justify_items_safe, c.justify_self_safe));
    }
    // `align-self` — про САМ элемент, а не про его детей. Раньше оба свойства
    // писались в одно поле, и элемент выравнивал содержимое вместо себя.
    if let Some(a) = c.align_self {
        d.style().align_self = Some(self_align(a, c.align_self_last));
    }
    // `flex-basis: auto` — это ОТСУТСТВИЕ основы, а не «во всю ширину»:
    // без отсева `flex: none` растягивал кнопку на всю строку.
    if let Some(b) = c.flex_basis.filter(|b| *b != Len::Auto) {
        d = d.flex_basis(len_to_gpui(b));
    }
    if let Some(j) = c.justify_content {
        // `left`/`right` (css-align-3 §5.2): вдоль строчной оси — как
        // `start`/`end` письма (в ряду); «if the property's axis is not
        // parallel with the inline axis, this value behaves as start» —
        // в КОЛОНКЕ это `flex-start` (`flexbox_justifycontent-right-002`).
        // Только горизонтальное письмо: в вертикальном оси уже переставлены
        // поворотом, и `left/right` там держит прежний путь (★ ЗАМЕРЕНО:
        // без этой отсечки `flexbox-justify-content-wmvert-001` 0.00 -> 1.12).
        let main_vertical =
            c.vertical.is_none() && matches!(dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
        let j = match j {
            // Не вдоль строчной оси — `start` ПИСЬМА, а не `flex-start`: у
            // `column-reverse` они смотрят в разные стороны, а спека требует
            // именно начало письма (`flexbox_justifycontent-left-002`:
            // «boxes … in the top left corner … top-to-bottom order»).
            Justify::Left | Justify::Right if main_vertical => Justify::WmStart,
            Justify::Left => Justify::WmStart,
            Justify::Right => Justify::WmEnd,
            other => other,
        };
        // `start`/`end` — начало и конец ОСИ ПИСЬМА (css-align-3 §4), а
        // `AlignContent::Start`/`End` у раскладки физические: смещение всегда
        // считается от `padding_border.main_start`, разворот выражен только
        // обратным обходом. Ось выше уже переведена в физическую (`rtl_row`
        // разворачивает ряд), поэтому при письме справа налево начало и конец
        // надо поменять местами (`flexbox_justifycontent-start-rtl`).
        let j = if rtl_row {
            match j {
                Justify::WmStart => Justify::WmEnd,
                Justify::WmEnd => Justify::WmStart,
                other => other,
            }
        } else {
            j
        };
        d.style().justify_content = Some(to_content(j));
    }
    // `align-content` — распределение СТРОК, когда их несколько: без него
    // перенесённые строки прижимались к началу вместо заданного распределения.
    if let Some(a) = c.align_content {
        d.style().align_content = Some(to_content(a));
    }
    // `baseline` на ИНЛАЙН-оси НАСТОЯЩЕЙ сетки не действует: элементы не
    // разделяют колоночный baseline-контекст (css-align §9.1; тест-ассерт
    // grid-self-baseline-horiz-001: «only align-self should apply») —
    // применение как items двигало содержимое вправо. У ЛУНОК инлайн-ось
    // живёт своим каналом (column-grid-lanes-item-baseline-002 полагается).
    let real_grid = matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid));
    // Теперь фильтр уже: у ГОРИЗОНТАЛЬНОЙ сетки `baseline` доходит до
    // раскладки — ортогональные (вертикальные) элементы образуют группы по
    // оси x (css-align-3 §9.1), а параллельные его не видят (бит 8 ниже;
    // `grid-justify-baseline-001`: одиночные группы `vertical-rl`/`-lr` берут
    // запасное `safe self-start` — правый и левый край, а не растяжение).
    if let Some(a) = c
        .justify_items
        .filter(|a| *a != Align::Baseline || !real_grid || c.vertical != Some(true))
    {
        d.style().justify_items = Some(self_align(a, c.justify_items_last));
    }
    if let Some(a) = c.justify_self {
        d.style().justify_self = Some(self_align(a, c.justify_self_last));
    }
    alignment_axes::abspos_normal(d.style(), c);
    alignment_axes::grid_self(d.style(), c);
    alignment_axes::project(
        d.style(),
        real_grid && c.vertical == Some(true),
        c.parent_grid >= 2,
    );
    // Биты базовой по оси x для элемента сетки (css-align-3 §9.1; Blink
    // baseline_utils.h `DetermineBaselineWritingMode`/`DetermineBaselineGroup`):
    // письмо базовой — своё у вертикального элемента, у горизонтального —
    // письмо вертикальной сетки (у горизонтальной сетки — `vertical-lr`).
    // Группа у правого края — когда это письмо `vertical-rl`. Синтез
    // центральный, когда у сетки вертикальное письмо не `sideways` (Blink
    // `parent_grid_font_baseline` = `GetFontBaseline()` сетки).
    d.style().baseline_x_flags = item_metadata::baseline_x_flags(c);
    if let Some(r) = c.aspect_ratio
        && !ratio_as_auto_min(c)
    {
        d.style().aspect_ratio = Some(r);
    }
    if let Some((row, col)) = c.gap {
        // При вертикальном письме ось блока горизонтальна: `row-gap` — зазор
        // ПО ГОРИЗОНТАЛИ, `column-gap` — по вертикали. Главную ось выше уже
        // переставили, зазор обязан ехать за ней. Сокращение `gap: 20px`
        // пишет оба поля одинаково и ошибку маскировало.
        let (down, across) = if c.vertical == Some(true) {
            (col, row)
        } else {
            (row, col)
        };
        // `calc(доля + точки)` в зазоре: доля — от своей стороны КОНТЕНТ-бокса
        // (css-gaps-1 §gap-percent: «'gap' always resolves percentages against
        // the corresponding size of the content box»). gpui знает «точки ИЛИ
        // доля», поэтому смесь разрешается здесь, когда сторона в точках
        // (`grid-gutters-011`: `calc(15% + 7px)` от 220 = 40). Сторона не
        // известна — зазор не ставится, как и прежде, когда запись
        // отбрасывалась целиком (`gap-010-ltr` держит ровно это).
        let px_or_0 = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let edges = c.borders();
        let content = |size: Option<Len>, sides: [Option<Len>; 4]| match size {
            Some(Len::Px(v)) if c.border_box == Some(true) => {
                Some((v - sides.iter().map(|s| px_or_0(*s)).sum::<f32>()).max(0.0))
            }
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let basis_y = content(
            c.height,
            [c.padding.top, c.padding.bottom, edges.top, edges.bottom],
        );
        let basis_x = content(
            c.width,
            [c.padding.left, c.padding.right, edges.left, edges.right],
        );
        let gap_len = |l: Len, basis: Option<f32>| -> Option<gpui::DefiniteLength> {
            // CSS Gaps 1 gap-percent: grid intrinsic sizing uses a zero
            // percentage basis, then layout resolves against the content box.
            // Preserve calc's constant term and percentage for those two phases.
            if real_grid && basis.is_none() {
                return Some(len_to_gpui(l));
            }
            if let Len::Calc(i) = l
                && let Some((k, add)) = crate::style::values::value::calc_get(i).pct_px()
            {
                return basis.map(|b| px((add + k * b).max(0.0)).into());
            }
            Some(len_to_gpui(l))
        };
        if let Some(r) = down.and_then(|r| gap_len(r, basis_y)) {
            d = d.gap_y(r);
        }
        if let Some(cg) = across.and_then(|cg| gap_len(cg, basis_x)) {
            d = d.gap_x(cg);
        }
    }

    // Движок раскладки всегда трактует размер как `border-box`, а CSS по
    // умолчанию — как `content-box`: заданная ширина не включает отступы и
    // рамку. Без компенсации блок с рамкой 4px выходил на 8 точек уже, чем в
    // браузере, и всё правее него уезжало (поймано сравнением с Chrome).
    let content_box = c.border_box != Some(true);
    // Доля отступа (`padding: 20%`) в точки здесь не переводится — её база,
    // ширина содержащего блока, известна только раскладке. Тогда пересчёт
    // `content-box` отдаётся ей целиком (`taffy::BoxSizing::ContentBox`):
    // прежде такая коробка оставалась border-box, и её содержимое ужималось
    // на ширину отступов (`padding-percentage-inherit-001`: 30 → 6 точек у
    // ребёнка).
    let native_content_box = intrinsic_size::native_content_box(c);
    if native_content_box {
        d.style().content_box = Some(true);
    }
    let extra = |sides: &[Option<Len>]| -> f32 {
        if !content_box || native_content_box {
            return 0.0;
        }
        sides
            .iter()
            .filter_map(|s| match s {
                Some(Len::Px(v)) => Some(*v),
                _ => None,
            })
            .sum()
    };
    let bw = c.borders();
    let pad_x = extra(&[c.padding.left, c.padding.right, bw.left, bw.right]);
    let pad_y = extra(&[c.padding.top, c.padding.bottom, bw.top, bw.bottom]);

    // Природный размер холста под порогами — таблица CSS 2.1 §10.4 для
    // замещаемых с соотношением сторон. Обе оси `<canvas>` пришли из
    // атрибутов (`attr_sized`), то есть это natural size, а не заданный
    // автором размер (HTML §4.12.5; холста нет среди «dimension attributes»
    // HTML Rendering §15.3.10): нарушенный порог одной оси переносится на
    // другую через соотношение, а не просто режет свою ось. Раньше
    // `<canvas width=200 height=200 style="max-height: 100px">` выходил
    // 200×100 вместо 100×100 (`percent-height-replaced-in-percent-cell-002`).
    // Пороги и размеры здесь — content-box, отбивки добавит цикл ниже.
    let natural_fit: Option<(f32, f32)> = if c.attr_sized.0
        && c.attr_sized.1
        && let (Some(Len::Px(w)), Some(Len::Px(h))) = (c.width, c.height)
        && w > 0.0
        && h > 0.0
    {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let min_w = px_of(c.min_width).unwrap_or(0.0);
        let min_h = px_of(c.min_height).unwrap_or(0.0);
        // §10.4: «max-width/max-height … less than min-* is treated as min-*».
        let max_w = px_of(c.max_width).unwrap_or(f32::INFINITY).max(min_w);
        let max_h = px_of(c.max_height).unwrap_or(f32::INFINITY).max(min_h);
        let r = w / h;
        let fit = if w > max_w && h > max_h {
            if max_w / w <= max_h / h {
                (max_w, (max_w / r).max(min_h))
            } else {
                ((max_h * r).max(min_w), max_h)
            }
        } else if w < min_w && h < min_h {
            if min_w / w <= min_h / h {
                ((min_h * r).min(max_w), min_h)
            } else {
                (min_w, (min_w / r).min(max_h))
            }
        } else if w < min_w && h > max_h {
            (min_w, max_h)
        } else if w > max_w && h < min_h {
            (max_w, min_h)
        } else if w > max_w {
            (max_w, (max_w / r).max(min_h))
        } else if w < min_w {
            (min_w, (min_w / r).min(max_h))
        } else if h > max_h {
            ((max_h * r).max(min_w), max_h)
        } else if h < min_h {
            ((min_h * r).min(max_w), min_h)
        } else {
            (w, h)
        };
        (fit != (w, h) && fit.0.is_finite() && fit.1.is_finite()).then_some(fit)
    } else {
        None
    };
    // Positioned boxes cannot use the intrinsic grid wrapper: it would change
    // their containing block. Preserve authored keywords for native measurement.
    if matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed)) {
        d.style().sizing_keywords = Some(intrinsic_size::keywords(c));
    }
    // `max-width`/`max-height: min-content | max-content` у ГИБКОГО ЭЛЕМЕНТА
    // (css-sizing-3 §3.2: the keyword «as a maximum size» — the box's
    // min-/max-content size in that axis). Длиной ключевое слово не
    // выражается, и цикл ниже его пропускал: предел терялся вовсе
    // (`flex-item-max-height-min-content`, `flex-item-max-width-min-content`).
    // Раскладка гибкого контейнера меряет его сама (taffy `flexbox.rs`).
    // Только когда предпочтительный размер оси не в точках: такую пару уже
    // переставили (`content_limit_swapped`) или сделали пределом ниже.
    if c.flex_item {
        let kw = |l: Option<Len>| match l {
            Some(Len::MinContent) => Some(gpui::CssSizingKeyword::MinContent),
            Some(Len::MaxContent) => Some(gpui::CssSizingKeyword::MaxContent),
            _ => None,
        };
        let px_size = |l: Option<Len>| matches!(l, Some(Len::Px(_)));
        let keys = [
            kw(c.max_width).filter(|_| !px_size(c.width)),
            kw(c.max_height).filter(|_| !px_size(c.height)),
        ];
        if keys.iter().any(Option::is_some) {
            d.style().max_sizing_keywords = Some(keys);
        }
    }
    for (val, f) in [
        (natural_fit.map(|f| Len::Px(f.0)).or(c.width), 0u8),
        (natural_fit.map(|f| Len::Px(f.1)).or(c.height), 1),
        (c.min_width, 2),
        (c.min_height, 3),
        (c.max_width, 4),
        (c.max_height, 5),
    ] {
        let Some(l) = val else { continue };
        // Размер по содержимому длиной не выражается: его ставит
        // обёртка-сетка (`render::content_sized`). Здесь он обязан
        // ПРОПУСКАТЬСЯ, иначе доходит до общей ветки и становится долей
        // родителя в сто процентов — то есть ровно обратным по смыслу.
        if matches!(
            l,
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
        ) {
            continue;
        }
        let Some(l) = size_percent::resolve(c, l, f) else {
            continue;
        };
        // Доли считаются от родителя и компенсации не требуют.
        let l = match l {
            Len::Px(v) if f % 2 == 0 => Len::Px(v + pad_x),
            Len::Px(v) => Len::Px(v + pad_y),
            other => other,
        };
        // Ключевое слово содержимого в `min-height`/`max-height` (css-sizing-3
        // §4.1): в блочной оси min-content = max-content = высота содержимого,
        // поэтому `min-height: max-content` даёт used = max(H, содержимое),
        // а `max-height: max-content` — min(H, содержимое). Заданная высота
        // становится соответствующим пределом, сама ось — auto
        // (`block-size-with-min-or-max-content-*`).
        let kw = |l: Option<Len>| {
            matches!(
                l,
                Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
            )
        };
        if f == 1
            && let Len::Px(h) = l
        {
            if kw(c.min_height) {
                d = d.min_h(px(h));
                continue;
            }
            if kw(c.max_height) {
                d = d.max_h(px(h));
                continue;
            }
        }
        // Смесь «доля ± точки»: поправка `content-box` едет в точечную часть,
        // доля считается от родителя и поправки не требует. Новая пара НЕ
        // кладётся в арену (`calc_store` на каждом кадре раздувал бы её).
        let g = match l {
            Len::Calc(i) => match crate::style::values::value::calc_get(i).pct_px() {
                Some((pct, add)) => {
                    gpui::DefiniteLength::Calc(add + if f % 2 == 0 { pad_x } else { pad_y }, pct)
                }
                None => len_to_gpui(l),
            },
            _ => len_to_gpui(l),
        };
        d = match f {
            0 => d.w(g),
            1 => d.h(g),
            2 => d.min_w(g),
            3 => d.min_h(g),
            4 => d.max_w(g),
            _ => d.max_h(g),
        };
    }
    // Автоминимум по содержимому в ratio-зависимой оси (css-sizing-4 §5.2:
    // «its min-content size capped by its maximum size»): размер из
    // соотношения идёт МИНИМУМОМ этой оси, сам размер остаётся auto — used =
    // max(ratio-размер, содержимое). Отношение в раскладку при этом не
    // отдаётся, иначе taffy зафиксировал бы ось (`block-aspect-ratio-009/…`,
    // `flex-aspect-ratio-040/…`).
    if ratio_as_auto_min(c)
        && let Some(r) = c.aspect_ratio
    {
        // Определённая ось сперва зажимается своими min/max (§5.1 «size
        // transfers»: `block-aspect-ratio-033`); при `box-sizing: border-box`
        // отношение считается по border-box (§5.1), размеры здесь —
        // content-box, поэтому отбивки прибавляются до переноса и
        // вычитаются после (`intrinsic-size-012`).
        let clamp = |v: f32, lo: Option<Len>, hi: Option<Len>| {
            let v = match lo {
                Some(Len::Px(l)) => v.max(l),
                _ => v,
            };
            match hi {
                Some(Len::Px(h)) => v.min(h),
                _ => v,
            }
        };
        let bb = c.border_box == Some(true);
        match (c.width, c.height) {
            (Some(Len::Px(w)), _) => {
                let w = clamp(w, c.min_width, c.max_width);
                let mh = if bb { (w + pad_x) / r - pad_y } else { w / r };
                let mh = clamp(mh.max(0.0), None, c.max_height);
                d = d.min_h(px(mh + pad_y));
                d.style().aspect_ratio_preferred_size = Some([None, Some(mh + pad_y)]);
            }
            (_, Some(Len::Px(h))) => {
                let h = clamp(h, c.min_height, c.max_height);
                let mw = if bb { (h + pad_y) * r - pad_x } else { h * r };
                let mw = clamp(mw.max(0.0), None, c.max_width);
                d = d.min_w(px(mw + pad_x));
                d.style().aspect_ratio_preferred_size = Some([Some(mw + pad_x), None]);
            }
            _ => {}
        }
    }
    d
}

/// Идёт ли `aspect-ratio` автоминимумом (css-sizing-4 §5.2): незамещаемая
/// коробка без прокрутки, ровно одна ось задана в точках, а минимум
/// ratio-зависимой оси не задан явно.
fn set_len(l: Option<Len>) -> bool {
    matches!(l, Some(x) if x != Len::Auto)
}

fn ratio_as_auto_min(c: &Computed) -> bool {
    let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
    let px_w = matches!(c.width, Some(Len::Px(_)));
    let px_h = matches!(c.height, Some(Len::Px(_)));
    c.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0)
        // `calc-size()` считает раскладка от размера, выведенного
        // соотношением (`vendor/taffy` — `calc_size_derived`, автоминимум):
        // эмуляция явным минимумом спрятала бы соотношение от неё
        // (`calc-size-aspect-ratio-001…004`).
        && c.calc_size.iter().all(Option::is_none)
        && visible(c.overflow_x)
        && visible(c.overflow_y)
        && !c.scroller
        && !c.flex_item_ratio
        // Абсолют с краями по ОБЕИМ сторонам зависимой оси растягивается
        // краями, и отношение обязано победить растяжку (`abspos-005/006`)
        // — ему отношение остаётся в раскладке; без краёв автоминимум
        // действует как у блока (`abspos-012/013`).
        && !(matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed))
            && (if px_w {
                set_len(c.inset.top) && set_len(c.inset.bottom)
            } else {
                set_len(c.inset.left) && set_len(c.inset.right)
            }))
        && (px_w != px_h)
        && (if px_w {
            !matches!(c.min_height, Some(Len::Px(_)))
        } else {
            !matches!(c.min_width, Some(Len::Px(_)))
        })
        // Процентный предел зависимой оси эмуляцией не выразить: явный
        // минимум из соотношения сильнее любого максимума (CSS 2.1 §10.4), а
        // доля предела во вкладе должна игнорироваться и решаться только в
        // раскладке (css-sizing-3 §5.2.1; `intrinsic-percent-non-replaced-007`:
        // `height:100px; 2/1; max-width:50%` в `max-content` — 100×100, а не
        // 200×100). Такой коробке соотношение остаётся в раскладке.
        && (if px_w {
            !matches!(c.max_height, Some(Len::Pct(_)))
        } else {
            !matches!(c.max_width, Some(Len::Pct(_)))
        })
}
