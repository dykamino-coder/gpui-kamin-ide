//! apply_layout, этапы display и гибких осей: вид коробки, направление, перенос, margin-trim, имена
//! линий и размещение в сетке, flex-grow/shrink.

use super::*;

pub(super) fn layout_flex_axes(
    mut d: Div,
    c: &BoxStyle,
    dir: Option<FlexDir>,
    rtl_row: bool,
) -> Div {
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
    if c.wm().rtl()
        && !c.wm().vertical()
        && c.reset_flex.flex_wrap != Some(true)
        && c.reset_flex.webkit_box != Some(true)
        && matches!(
            c.reset_box_layout.display,
            Some(Display::Flex) | Some(Display::InlineFlex)
        )
        && matches!(dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
    {
        d.style().flex_cross_reverse = Some(true);
    }
    if c.reset_flex.flex_wrap == Some(true) {
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
        let inline_reversed = (c.wm().rtl()) != c.wm().sideways_lr();
        let flip = if matches!(
            c.reset_flex.flex_dir,
            Some(FlexDir::Col) | Some(FlexDir::ColReverse)
        ) {
            inline_reversed
        } else {
            c.wm().block_rl()
        };
        if (c.reset_flex.flex_wrap_reverse == Some(true)) != flip {
            d.style().flex_wrap = Some(gpui::FlexWrap::WrapReverse);
        }
        // `flex-wrap: balance` — балансировщик строк в раскладке;
        // `flex-line-count` (умолчание 1) — минимум строк. У legacy
        // `-webkit-box` balance не действует (Blink `IsDeprecatedFlexbox`).
        if c.reset_flex.flex_balance == Some(true) && c.reset_flex.webkit_box != Some(true) {
            d.style().flex_balance_lines = Some(c.reset_flex.flex_line_count.unwrap_or(1).max(1));
        }
    }
    // `margin-trim` гибкого контейнера и сетки (css-box-4 §margin-trim):
    // раскладка физическая, поэтому логические края переводятся ЗДЕСЬ, по
    // письму самого контейнера. Биты раскладки: 1 верх, 2 право, 4 низ,
    // 8 лево (`flex-*-trimmed-only`, `flex-*-multiline`, `grid-*-start`).
    if c.reset_fragmentation.margin_trim != 0
        && matches!(
            c.reset_box_layout.display,
            Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::Grid)
                | Some(Display::InlineGrid)
        )
    {
        let vertical = c.wm().vertical();
        let (block_start, block_end) = match (vertical, c.wm().block_rl()) {
            (false, _) => (1u8, 4u8),
            (true, true) => (2, 8),
            (true, false) => (8, 2),
        };
        let (inline_start, inline_end) = match (vertical, c.wm().rtl()) {
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
            if c.reset_fragmentation.margin_trim & bit != 0 {
                physical |= side;
            }
        }
        d.style().margin_trim = Some(physical);
    }
    if let Some(names) = grid_line_names(c) {
        d.style().grid_line_names = Some(Box::new(names));
    }
    if c.reset_grid.grid_col.is_some() || c.reset_grid.grid_row.is_some() {
        let span = |p: Option<(Placement, Placement)>| {
            let (a, b) = p.unwrap_or((Placement::Auto, Placement::Auto));
            to_placement(a)..to_placement(b)
        };
        // Грани элемента ЛОГИЧЕСКИЕ (css-grid-2 §8.3: `grid-row` — блочная
        // ось сетки), а дорожки вертикальной сетки уже переставлены
        // (`grid_style`, `flip`): ряды — физические колонки.
        let (row, column) = if placement_flip(c) {
            (span(c.reset_grid.grid_col), span(c.reset_grid.grid_row))
        } else {
            (span(c.reset_grid.grid_row), span(c.reset_grid.grid_col))
        };
        d.style().grid_location = Some(gpui::GridLocation { row, column });
    }
    if let Some(g) = c.reset_flex.flex_grow {
        // `flex_grow()` в GPUI ставит жёсткую единицу, а `flex: 2` встречается —
        // пишем значение в стиль напрямую.
        d.style().flex_grow = Some(g);
    }
    if let Some(shrink) = c.reset_flex.flex_shrink {
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
    d
}

pub(super) fn layout_display(mut d: Div, c: &BoxStyle) -> Div {
    d.style().block_flow = Some(grid_flow_axes::block(c));
    match c.reset_box_layout.display {
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
            // (`initial-letter-block-position-raise-over/under-ruby`).
            // Keep the inline-block boundary flag for non-visible overflow too:
            // native output adaptation then suppresses its content baseline.
            if !c.context.parent_flex_grid
                && c.reset_text_box.ruby_role.is_none()
                && c.reset_context.inline_display != Some(true)
            {
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
        // `display: flow-root` is a block container that establishes a new
        // block formatting context (css-display-3 §2.2): the native block
        // layout must not collapse its children's margins through it
        // (CSS 2.1 §8.3.1). Blink: `CreatesNewFormattingContext()`.
        Some(Display::Block) | Some(Display::ListItem)
            if c.reset_context.flow_root == Some(true) =>
        {
            d.style().display = Some(gpui::Display::FlowRoot);
        }
        // `display: none` отсеивается ещё при разборе дерева: узел не строится.
        _ => {}
    }
    // `direction: rtl` переворачивает главную ось и выравнивание по
    // умолчанию: ряд идёт справа налево, текст прижимается вправо.
    if c.wm().rtl() {
        // Разворот главной оси — дело ТОЛЬКО гибкого ряда: обычный блок
        // собирается колонкой, и разворот переставлял его детей снизу вверх.
        // Ряд с явно заданным направлением разворачивается ниже, вместе с
        // остальными случаями.
        let default_row = c.reset_flex.flex_dir.is_none()
            && matches!(
                c.reset_box_layout.display,
                Some(Display::Flex) | Some(Display::InlineFlex)
            );
        if default_row {
            d = d.flex_row_reverse();
        }
        if c.inh.text_flow.text_align.is_none() {
            d = d.text_right();
        }
    }
    d
}
