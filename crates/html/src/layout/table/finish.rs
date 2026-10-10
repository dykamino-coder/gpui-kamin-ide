//! Хвост раскладки таблицы: сетка ячеек, дорожки, рамка и подписи (table_finish).

use crate::dom::{Element, Node};
use crate::layout::block::containing::CB_WIDTH;
use crate::layout::block::margins::CELL_BFC;
use crate::layout::positioned::predicates::edge_set;
use crate::layout::table::columns::track_list_collapsed;
use crate::layout::table::is_cell;
use crate::paint::effects::transform::transformed;
use crate::render::{RenderOpts, blocks, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, FlexDir};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::too_many_arguments, clippy::ptr_arg)]
pub(super) fn table_finish(
    under: Vec<AnyElement>,
    paint_layers: bool,
    cell_bgs: std::rc::Rc<std::cell::RefCell<Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)>>>,
    cells: Vec<AnyElement>,
    table_edges: std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    cells_over: Vec<AnyElement>,
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    row_elements: Vec<&Element>,
    win_edges: std::collections::HashMap<u64, [f32; 4]>,
    table_font: f32,
    table_family: &String,
    spacing: (f32, f32),
    cols: u16,
    from_cols: Vec<Option<f32>>,
    mut col_widths: Vec<(Option<f32>, Option<f32>)>,
    cols_collapsed: Vec<bool>,
    cols_pct: Vec<Option<f32>>,
    bw: [f32; 4],
    outer_win: [f32; 4],
    have_rows: bool,
    px_of: impl Fn(Option<Len>) -> f32,
) -> AnyElement {
    let mut grid_children = under;
    if paint_layers {
        grid_children.push(
            crate::layout::table::paint::CellBgPainter::new(cell_bgs.clone()).into_any_element(),
        );
    }
    grid_children.extend(cells);
    if paint_layers {
        grid_children.push(
            crate::layout::table::paint::EdgePainter::new(table_edges.clone()).into_any_element(),
        );
    }
    grid_children.extend(cells_over);
    let cells = grid_children;
    // Заголовок таблицы живёт ВНЕ коробки таблицы (CSS 2.1 §17.4:
    // анонимная обёртка держит заголовок и коробку) — рамка и обрезка
    // таблицы его не трогают; `caption-side: bottom` ставит его под сетку.
    let mut caps_top: Vec<AnyElement> = Vec::new();
    let mut caps_bot: Vec<AnyElement> = Vec::new();
    collect_captions(e, inherited, opts, &mut caps_top, &mut caps_bot);

    // Оси таблицы ЛОГИЧЕСКИЕ, как и у сетки: колонки идут вдоль строки. При
    // вертикальном письме строка идёт сверху вниз, и дорожки колонок
    // становятся физическими рядами. Своей ветки у таблицы не было, и её
    // сетка строилась физической — мимо уже переставленных осей.
    // Ширины колонок фиксированной раскладки — из ПЕРВОГО ряда
    // (CSS 2.1 §17.5.2.1): ячейка с шириной держит её, остальные делят
    // остаток поровну.
    let first_row_widths = first_row_widths_of(
        e,
        &row_elements,
        win_edges,
        table_font,
        table_family,
        spacing,
        cols,
    );
    // `<col>`-ширины старше ячеек первого ряда (§17.5.2.1) и действуют и в
    // авто-раскладке: колонка с шириной держит её (как ширина ячейки).
    for (i, w) in from_cols.iter().enumerate() {
        if let (Some(w), Some(slot)) = (w, col_widths.get_mut(i)) {
            slot.0 = Some(slot.0.map_or(*w, |old| old.max(*w)));
        }
    }
    let first_row_widths: Vec<Option<f32>> = (0..cols as usize)
        .map(|i| {
            from_cols
                .get(i)
                .copied()
                .flatten()
                .or_else(|| first_row_widths.get(i).copied().flatten())
        })
        .collect();
    let tracks = track_list_collapsed(
        cols,
        e.style.table_fixed == Some(true),
        &first_row_widths,
        &col_widths,
        &cols_collapsed,
        &cols_pct,
        // Место под КОЛОНКИ, а не вся ширина стола: §17.5.2.1 считает долю
        // от ширины таблицы БЕЗ её рамок и без зазоров между ячейками
        // (`fixed-table-layout-022` расписывает это прямо в тексте: 533 − 58
        // рамок − 75 зазоров = 400).
        match match e.style.width {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => CB_WIDTH.get().filter(|v| *v > 0.0).map(|cb| cb * k),
            _ => None,
        } {
            Some(v) => {
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(w)) => w,
                    _ => 0.0,
                };
                let bs = e.style.borders();
                let gap = if e.style.border_collapse == Some(true) {
                    0.0
                } else {
                    match e.style.border_spacing {
                        Some((Some(Len::Px(g)), _)) => g,
                        _ => 0.0,
                    }
                };
                Some(v - side(bs.left) - side(bs.right) - gap * (cols as f32 + 1.0))
            }
            _ => None,
        },
    );
    // Таблица ЗАДАННОЙ высоты раздаёт лишнее место рядам БЕЗ своей высоты
    // (CSS 2.1 §17.5.3): ряд с высотой (своей или ячеек) держит её, остальные
    // делят остаток. Без этого средний ряд решётки 64/auto/64 в таблице 224px
    // схлопывался по содержимому, и вся середина уезжала.
    // Считается и от min-height: минимум так же растягивает таблицу, и
    // остаток обязан достаться безвысотным рядам.
    let table_tall = matches!(e.style.height, Some(Len::Px(_)))
        || matches!(e.style.min_height, Some(Len::Px(_)))
        // Доля высоты, которая РАЗРЕШАЕТСЯ (содержащий блок определён), —
        // такая же заданная высота: css-tables-3 «a 'height' property with a
        // value other than auto … will eventually be distributed to the height
        // of the rows». Неразрешимая доля — это `auto` (CSS 2.1 §10.5), её не
        // берём. Растянутую раскладкой коробку (элемент гибкого контейнера или
        // сетки со своей долей) тоже не берём: там доля в taffy падает в `auto`,
        // а ряды-доли в неопределённой высоте выравниваются по самому высокому.
        // Эталоны `fr-unit-ref`, `display-grid-ref`: стол `height:100%` в
        // абсолюте 400×100 держал ряды по тексту вместо 30/70.
        || (matches!(e.style.height, Some(Len::Pct(_)))
            && inherited.cb_height_def
            && !inherited.stretched);
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v135, `scout-fonts-tables-2026-09.md`
    // план 2): строить дорожки рядов и без `table_tall`. Срез 8557 общих:
    // +3 (`table-as-item-cell-percentage-001/003/004`) при −18 —
    // `margin-applies-to-001…007` (0.00 → 1.00),
    // `margin-bottom-applies-to-001…007` (0.03 → 3.00),
    // `table-cell-overflow-explicit-height-001/002` (0.00 → 8.77),
    // `percentage-sizing-of-table-cell-children-004` («красное видно»),
    // `subpixel-table-cell-height-001`. Ряд без заданной высоты обязан
    // остаться авто-дорожкой ТОЛЬКО в контексте, где стол не растянут.
    let grid_box = table_grid_box(e, inherited, row_elements, tracks, table_tall);
    // Сросшиеся рамки: у таблицы нет паддинга, а кромка между её рамкой и
    // краевыми ячейками одна — ячейки накрывают ВНУТРЕННЮЮ ПОЛОВИНУ рамки
    // (CSS 2.1 §17.6.2). Рамка при этом рисуется ПОВЕРХ фонов ячеек, как и
    // все сросшиеся кромки: обычная рамка коробки красится под детьми и
    // закрашивалась бы их фоном. Поэтому у самой коробки рамка снимается,
    // её место держит паддинг, сетка выезжает на его половину, а красит
    // рамку кольцевой квад ПОСЛЕ сетки.
    let collapse = e.style.border_collapse == Some(true)
        || (e.style.border_collapse.is_none() && e.attr("rules").is_some());
    // Минимумы таблицы меряются ПОЛНОЙ коробкой с рамкой и паддингом
    // (css-tables-3 §computing-the-table-height, CSSWG #5336): пороги
    // пересчитываются в контентные, компенсацию вернёт общий слой.
    let min_fix = |len: Option<Len>, edges: f32| match len {
        // Тегу `<table>` вычитать нельзя: ниже он получает `border_box`
        // (UA-правило), и taffy вычтет рамки с паддингом ВТОРОЙ раз —
        // `min-height-table`: 312 → 300 → содержимое 288 вместо 300. С явным
        // `box-sizing: content-box` порог по спеке и так контентный
        // (`min-max-size-table-content-box`). Зелёная `min-height-table-2`
        // (тот же тег, порог долей) держится ровно на `border_box` без вычета.
        Some(Len::Px(v)) if e.style.border_box != Some(true) && e.tag != "table" => {
            Some(Len::Px((v - edges).max(0.0)))
        }
        other => other,
    };
    let pad = &e.style.padding;
    let pad_px = [
        px_of(pad.top),
        px_of(pad.right),
        px_of(pad.bottom),
        px_of(pad.left),
    ];
    let min_h = min_fix(e.style.min_height, bw[0] + bw[2] + pad_px[0] + pad_px[2]);
    // ПРОБОВАЛИ И ОТКАТИЛИ: поднимать нижнюю грань ширины таблицы до суммы
    // дорожек с зазорами (§17.5.2: «the used width is the greater of the value
    // of width and MIN»). Проба по 19 парам семей `separated-border-model-*` и
    // `fixed-table-layout-02*`: флипов ноль, `separated-border-model-004d`
    // 1.90 -> 1.75. Минимум доезжает, но заданную ширину не перебивает —
    // упирается ниже, в раздачу дорожек внутри сетки.
    let min_w = min_fix(e.style.min_width, bw[1] + bw[3] + pad_px[1] + pad_px[3]);
    // У ТЕГА `<table>` ширина считается по BORDER-BOX (UA-правило
    // css-tables-3: `table { box-sizing: border-box }`), у `display: table` на
    // прочих тегах — контентная. Тесты пишут это прямо: «the width of an
    // HTML/XHTML table is the distance between the left and right table border
    // edges» против «the width of a CSS table … excluding table padding and
    // table borders».
    let table_border_box = e.tag == "table" && e.style.border_box.is_none();
    // Фиксированная раскладка: ширина стола — БОЛЬШЕЕ из `width` и суммы
    // колонок с зазорами (CSS 2.1 §17.5.2.1, «the greater of the value of the
    // 'width' property … and the sum of the column widths (plus cell spacing
    // or borders)»). Коробка брала заявленную ширину всегда, сетка
    // вылезала из неё: `separated-border-model-004d` — фон 9 точек при
    // сетке 42+16+42, `-004c` — 70 при 20·3+20·2. Считается только когда
    // ВСЕ колонки первого ряда известны в точках: иначе сумма не определена.
    // Сросшаяся модель не берётся — её края лежат паддингом `outer_win`.
    let fixed_floor: Option<f32> = match e.style.width {
        Some(Len::Px(w))
            if e.style.table_fixed == Some(true)
                && !collapse
                && e.style.vertical != Some(true)
                && !first_row_widths.is_empty()
                && first_row_widths.iter().all(|c| c.is_some()) =>
        {
            let edges = if table_border_box || e.style.border_box == Some(true) {
                bw[1] + bw[3] + pad_px[1] + pad_px[3]
            } else {
                0.0
            };
            let floor = first_row_widths.iter().flatten().sum::<f32>()
                + spacing.0 * (f32::from(cols) + 1.0)
                + edges;
            (floor > w + 0.5).then_some(floor)
        }
        _ => None,
    };
    // §17.4: `position` и края — свойства ОБЁРТКИ таблицы, а не её сетки;
    // ширину сетки решает §17.5.2.2 (сжатие по содержимому). Пока коробка
    // одна, абсолютная таблица с ОБОИМИ краями инлайн-оси получала ширину от
    // краёв, и колонки расползались: сжатие у нас выражено только
    // `align_self`, а его у абсолютной коробки с двумя краями не спрашивают.
    let split_wrapper = matches!(
        inherited.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) && edge_set(inherited.inset.left)
        && edge_set(inherited.inset.right)
        && e.style.width.is_none();
    // Стиль СЕТКИ — без позиционирования и краёв: их заберёт обёртка.
    let grid_style = split_wrapper.then(|| {
        let mut c = inherited.clone();
        c.position = None;
        c.inset = Default::default();
        c.z_index = None;
        c
    });
    let inherited: &Computed = grid_style.as_ref().unwrap_or(inherited);
    let needs_clone = collapse
        || table_border_box
        || min_h != e.style.min_height
        || min_w != e.style.min_width
        || fixed_floor.is_some();
    let host_style;
    let mut outer = if needs_clone {
        let mut c = inherited.clone();
        if collapse {
            c.border_width = Default::default();
            c.border_visible = [None; 4];
            // §17.6.2: внутрь таблицы уходит ПОЛОВИНА её кромки. Паддингом,
            // а не рамкой: проба кромок — абсолютный ребёнок по паддинг-боксу,
            // и рамка утащила бы линию сетки внутрь на свою величину.
            c.padding = crate::style::computed::Sides {
                top: Some(Len::Px(outer_win[0] / 2.0)),
                right: Some(Len::Px(outer_win[1] / 2.0)),
                bottom: Some(Len::Px(outer_win[2] / 2.0)),
                left: Some(Len::Px(outer_win[3] / 2.0)),
            };
        }
        // Пол фиксированной раскладки (см. `fixed_floor`) — сама ширина
        // коробки: сетка внутри ровно такой ширины.
        if let Some(f) = fixed_floor {
            c.width = Some(Len::Px(f));
        }
        c.min_height = min_h;
        c.min_width = min_w;
        if table_border_box {
            c.border_box = Some(true);
            // Вертикальное письмо: `height` — это ИНЛАЙН-размер стола
            // (Blink: `ComputeTableInlineSize` читает `style.LogicalWidth()`,
            // а при `vertical-*` это физическое `height`), и наш конвейер УЖЕ
            // потратил её как КОНТЕНТНУЮ величину: предел ортогонального
            // потока сеется из `e.style.height` без вычета краёв (блок
            // `merged.ortho_limit` выше), и повёрнутый абзац рвёт строку
            // ровно по нему. Значит второй раз, коробкой, та же величина
            // обязана лечь по content-box, иначе краи вычитаются дважды и
            // стол выходит короче содержимого на свои рамки
            // (`row-progression-vrl-002`: 140.0 вместо 180.0 при неизменной
            // туши). Флаг `border_box` один на обе оси, поэтому его НЕ
            // снимаем — иначе content-box получила бы и `width`, то есть
            // БЛОЧНАЯ ось, где border-box верен; вместо этого краи инлайн-оси
            // добавляются к самой величине. Гейт тот же, что у
            // транспонирования решётки и у `spacing_phys`.
            if e.style.vertical == Some(true)
                && let Some(Len::Px(h)) = c.height
            {
                // Сросшийся стол несёт свои краи не рамкой, а паддингом в
                // половину победившей кромки (см. ветку `collapse` выше) —
                // берём ровно то, что легло в коробку.
                let inline_edges = if collapse {
                    (outer_win[0] + outer_win[2]) / 2.0
                } else {
                    bw[0] + bw[2] + pad_px[0] + pad_px[2]
                };
                c.height = Some(Len::Px(h + inline_edges));
            }
        }
        host_style = c;
        styled_div_with(e, &host_style).flex().flex_col()
    } else {
        styled_div_with(e, inherited).flex().flex_col()
    };
    // Таблица без заданной ширины СЖИМАЕТСЯ по содержимому, а не растягивается
    // на родителя (CSS 2.1 §17.5.2, shrink-to-fit). Пока она растягивалась,
    // две короткие колонки разъезжались к противоположным краям — видно на
    // `shaping-tatweel-002`, где одинаковые знаки стояли по краям окна.
    //
    // Заданный `align-self` приём отменяет: у элемента СЕТКИ эта ось —
    // блочная (css-align-3 §6.2, `taffy: grid/alignment.rs:145`), сжатие по
    // строчной оси ведёт `justify-self`, и прижим к началу здесь только
    // отбирал у стола высоту дорожки. Метку ставит
    // `dom::grid_table_items_keep_stretch`; авторский `align-self` она же и
    // пропускает вперёд.
    if e.style.width.is_none() && e.style.align_self.is_none() {
        outer.style().align_self = Some(gpui::AlignItems::FlexStart);
    }
    // Пол GRIDMIN (css-tables-3 §3.9): гибкая раскладка не ужимает стол по
    // главной оси ниже min-content его решётки — `vendor/taffy` `flexbox.rs`,
    // признак `item_is_table` (`table-as-item-auto-min-width`, `-wide-content`).
    outer.style().item_is_table = Some(true);
    // Table baselines come from rows, never the empty grid/caption shim.
    outer.style().baseline_unavailable = Some(!have_rows);
    // An enclosing `inline-block` takes no baseline from a table at any block
    // depth (CSS 2.1 §10.8.1 counts line boxes only; Blink
    // `block_layout_algorithm.cc` `PropagateBaselineFromBlockChild`: "table's
    // don't contribute any baselines"). The wrappers below carry the same mark.
    outer.style().no_inline_block_baseline = Some(true);
    // КОРНЕВОЙ стол (`<html display: table>`): родитель — блок стенда, где
    // `align-self` не работает, и стол растягивался на всё окно. Гибкая
    // обёртка возвращает сжатие по содержимому и центрирование `margin: auto`.
    let root_table = matches!(e.tag.as_str(), "html" | "body") && e.style.width.is_none();
    // `border-spacing` задан по ЛОГИЧЕСКИМ осям таблицы: первое значение —
    // зазор между КОЛОНКАМИ (инлайн-ось), второе — между РЯДАМИ (блочная
    // ось). css-writing-modes-3 §7.2 «Dimension Mapping» переносит их на
    // физические оси письмом ТАБЛИЦЫ: в вертикальном письме инлайн-ось
    // вертикальна, поэтому первое значение становится физическим
    // ВЕРТИКАЛЬНЫМ зазором, второе — горизонтальным. Решётка выше уже
    // транспонирована (`grid_box`: дорожки колонок легли в
    // `grid_template_rows`, ряды пошли колонками), а зазоры оставались
    // физическими — стол выходил перекошенным зеркально
    // (`border-spacing-vrl-002`: 160.0×70.0 вместо 100×100). Гейт — тот же
    // `e.style.vertical`, что и у решётки: иначе они разъедутся.
    // Стол БЕЗ рядов не несёт блочных зазоров, без рядов и колонок — и
    // строчных: пустой `<table>` у Blink/Gecko — только рамки. У нас
    // оставалась коробка 4×4 (умолчание 2px), и в
    // `caption-relative-positioning` между двумя подписями светила красная
    // полоса. Столы с рядами не меняются.
    let spacing = (
        if have_rows || !from_cols.is_empty() {
            spacing.0
        } else {
            0.0
        },
        if have_rows { spacing.1 } else { 0.0 },
    );
    let spacing_phys = if e.style.vertical == Some(true) {
        (spacing.1, spacing.0)
    } else {
        spacing
    };
    let mut outer = outer.child(
        grid_box
            // `border-spacing: 2px` — умолчание браузера для таблицы с
            // раздельными рамками. Без него строки идут плотнее, и
            // расхождение копится вниз по таблице.
            .gap_x(px(spacing_phys.0))
            .gap_y(px(spacing_phys.1))
            // Зазор действует и МЕЖДУ краем таблицы и крайними ячейками
            // (CSS 2.1 §17.6.1), не только между ячейками. Эталоны
            // гасят его отрицательным полем на таблице.
            .px(px(spacing_phys.0))
            .py(px(spacing_phys.1))
            .children(cells)
            .into_any_element(),
    );
    if collapse {
        // Граница СЕТКИ выдаётся ВСЕГДА: слой кромок обязан знать, где
        // кончаются дорожки, даже когда своей рамки у таблицы нет. Кромок
        // эта проба не несёт — только координаты.
        //
        // ЗАМЕРЕНО И ОТКАЧЕНО: подменять ею пробу КРОМОК таблицы (нулевые
        // ширины в общем разборе) — CSS2 +21/-25: у таблицы без рамки её
        // коробка совпадает с внешними краями ячеек, и те переставали
        // центрироваться.
        outer = outer.child(crate::layout::table::paint::grid_probe(
            table_edges.clone(),
            bw,
        ));
    }
    outer = paint_table_border(table_edges, e, bw, outer_win, collapse, inherited, outer);
    let shrink_wrap = root_table
        || (e.style.width.is_none()
            // Заданная высота или её порог приходят от РАСКЛАДКИ родителя:
            // обёртка рвёт эту связь (★ ЗАМЕРЕНО: без отсечки
            // `min-height-table-2` 0.00 -> 19.24).
            && e.style.height.is_none()
            && e.style.min_height.is_none()
            && !inherited.stretched
            && e.style.flex_basis.is_none()
            && e.style.align_self.is_none()
            && e.style.grid_col.is_none()
            && e.style.grid_row.is_none());
    let mut outer = outer;
    // Сжатие по содержимому — `min(max-content, доступное)` (CSS 2.1
    // §17.5.2.2: «the used width is the greater of W and MIN» при W =
    // ширине контейнера, если таблица шире): в ряду-обёртке стол обязан
    // ужиматься. Блоку потока сжатие выключено (`flex_shrink = 0` в
    // `collapsed`), и стол с длинным текстом вылезал из узкого родителя
    // на всю max-content ширину. Пол GRIDMIN держит `item_is_table`.
    if shrink_wrap && caps_top.is_empty() && caps_bot.is_empty() && !split_wrapper {
        outer.style().flex_shrink = Some(1.0);
    }
    let outer = outer;
    // Обёртка «заголовок + коробка»: заголовок вне рамки и обрезки.
    let outer = wrap_with_captions(e, caps_top, caps_bot, inherited, outer);
    // Вторая половина §17.4: сама обёртка. Гибкий ряд возвращает сетке сжатие
    // по содержимому — тот же приём, что у корневого стола ниже.
    if split_wrapper {
        let mut wrap = Computed::default();
        wrap.position = e.style.position;
        wrap.inset = e.style.inset;
        wrap.z_index = e.style.z_index;
        let mut wrap = crate::style::apply::apply(div(), &wrap).flex().flex_row();
        wrap.style().no_inline_block_baseline = Some(true);
        return wrap.child(outer).into_any_element();
    }
    // Стол с `width: auto` СЖИМАЕТСЯ по содержимому (§17.5.2): у нас это
    // делает гибкий ряд-обёртка. Приём `align_self: FlexStart` выше работает
    // только когда родитель — гибкая колонка нашей сборки; под `body` со
    // сброшенными полями путь другой, и стол растягивался во всю ширину
    // (`html-display-table`, `root-box-002`). Обёртка снимает зависимость от
    // родителя. Элемент гибкого контейнера, сетки и ячейки не заворачивается:
    // там стол — сам элемент раскладки, и обёртка забрала бы его свойства.
    if shrink_wrap {
        let mut wrap = div().flex().flex_row();
        if root_table {
            wrap = wrap.w_full();
        }
        wrap.style().no_inline_block_baseline = Some(true);
        return wrap.child(outer).into_any_element();
    }
    outer.into_any_element()
}

fn collect_captions(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    caps_top: &mut Vec<AnyElement>,
    caps_bot: &mut Vec<AnyElement>,
) {
    for c in &e.children {
        if let Node::Element(cap) = c
            && (cap.tag == "caption" || cap.style.is_caption == Some(true))
        {
            let cm = inherit(inherited, &cap.style);
            // Сторона — с самого заголовка, при пустоте — от таблицы
            // (наследование caption-side).
            let cap_side_bottom = cap.style.caption_bottom.or(e.style.caption_bottom) == Some(true);
            // CSS 2.1 §9.4.1: a table caption is a block container that
            // establishes a block formatting context, so its auto height
            // contains its floats (§10.6.7), like a cell (`CELL_BFC`).
            CELL_BFC.with(|c| c.set(true));
            let inside = blocks(&cap.children, &cm, opts);
            CELL_BFC.with(|c| c.set(false));
            let built = styled_div_with(cap, &cm)
                .flex()
                .flex_col()
                .children(inside)
                .into_any_element();
            // A caption is a transformable block box (css-transforms-1
            // §transformable-element); its `transform` was dropped
            // (`transform-transformed-caption-contains-fixed-position`).
            let built = transformed(built, &cap.style, inherited);
            // ВСЕ подписи, а не первая. Прежний `break` ронял вторую целиком:
            // у таблицы с верхней И нижней подписью рисовалась только верхняя
            // (`table-border-002/003/004`, снимок `table-border-004`: зелёное
            // теста обрывается на y = 253, то есть на 110 + 20 + 20 = 150
            // css-пикселях, а нижняя подпись в 250 пикселей не нарисована
            // ВООБЩЕ). CSS 2.1 §17.4 и css-tables-3 §terminology кладут в
            // обёртку ВСЕ подписи; Blink — двумя петлями по всем подписям
            // своей стороны (`table_layout_algorithm.cc:988` «Add all the top
            // captions», `:1584` «Add all the bottom captions»).
            if cap_side_bottom {
                caps_bot.push(built);
            } else {
                caps_top.push(built);
            }
        }
    }
}

fn first_row_widths_of(
    e: &Element,
    row_elements: &Vec<&Element>,
    win_edges: std::collections::HashMap<u64, [f32; 4]>,
    table_font: f32,
    table_family: &str,
    spacing: (f32, f32),
    cols: u16,
) -> Vec<Option<f32>> {
    let first_row_widths: Vec<Option<f32>> = row_elements
        .first()
        .map(|row| {
            let mut out = vec![];
            for c in &row.children {
                if let Node::Element(cell) = c
                    && is_cell(cell)
                {
                    let span = cell
                        .attr("colspan")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    // Колонка = width + горизонтальные паддинги и рамки
                    // ячейки (§17.5.2.1, content-box); в сросшейся модели
                    // рамка входит ПОЛОВИНОЙ.
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(p)) => p,
                        _ => 0.0,
                    };
                    let extra = if cell.style.border_box == Some(true) {
                        0.0
                    } else {
                        let b = cell.style.borders();
                        let border = side(b.left) + side(b.right);
                        side(cell.style.padding.left)
                            + side(cell.style.padding.right)
                            + if e.style.border_collapse == Some(true) {
                                // Половина ПОБЕДИВШЕЙ линии — та же, что
                                // легла в паддинг ячейки (§17.6.2.1).

                                win_edges
                                    .get(&cell.node_id)
                                    .map(|w| (w[1] + w[3]) / 2.0)
                                    .unwrap_or(border / 2.0)
                            } else {
                                border
                            }
                    };
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разрешать ширину ячейки в
                    // единицах ШРИФТА (`width: 1em` доезжает сюда
                    // неразрешённой — `resolve_em` живёт в наследовании, а
                    // ширины колонок считаются раньше). Срез из 10 пар
                    // `separated-border-model-*`: 8 зелёных до и после, ни
                    // один вердикт не сдвинулся. Одной этой половины мало:
                    // цель (`-004c/-004d`) требует ещё нижней грани ширины
                    // стола по сумме дорожек — возвращать вместе с ней.
                    match cell.style.width {
                        Some(Len::Px(v)) if span == 1 => out.push(Some(v + extra)),
                        // Ширина в единицах шрифта — кеглем САМОЙ ячейки
                        // (ряд → таблица): без неё колонка `width: 1em`
                        // уходила в безразмерные, и пол ширины стола ниже
                        // (§17.5.2.1) не складывался (`separated-border-model-004c`).
                        Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) if span == 1 => {
                            let size = match cell.style.font_size.or(row.style.font_size) {
                                Some(Len::Px(v)) => v,
                                _ => table_font,
                            };
                            let family = cell
                                .style
                                .font_family
                                .clone()
                                .unwrap_or_else(|| table_family.to_string());
                            let v = crate::text::metrics::spacing_px(Some(l), &family, size);
                            out.push((v > 0.0).then_some(v + extra));
                        }
                        // Доля считается от места, отдаваемого дорожкам:
                        // ширина таблицы за вычетом зазоров (§17.5.2.1,
                        // «a percentage value ... of the table width»).
                        // Известна, только когда ширина таблицы в точках.
                        Some(Len::Pct(p)) if span == 1 => match e.style.width {
                            Some(Len::Px(tw)) => {
                                let gaps = spacing.0 * (f32::from(cols) + 1.0);
                                out.push(Some((tw - gaps).max(0.0) * p + extra));
                            }
                            _ => out.push(None),
                        },
                        _ => out.extend(std::iter::repeat_n(None, span)),
                    }
                }
            }
            out
        })
        .unwrap_or_default();
    first_row_widths
}

fn table_grid_box(
    e: &Element,
    inherited: &Computed,
    row_elements: Vec<&Element>,
    tracks: Vec<gpui::GridTrack>,
    table_tall: bool,
) -> gpui::Div {
    let row_tracks: Option<Vec<gpui::GridTrack>> = match table_tall {
        true if e.style.vertical != Some(true) => Some(
            row_elements
                .iter()
                .map(|row| {
                    let cell_h = |c: &Node| match c {
                        Node::Element(cell) if is_cell(cell) => match cell.style.height {
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        },
                        _ => None,
                    };
                    let own = match row.style.height {
                        Some(Len::Px(v)) => Some(v),
                        _ => None,
                    };
                    match own
                        .into_iter()
                        .chain(row.children.iter().filter_map(cell_h))
                        .fold(None::<f32>, |a, v| Some(a.map_or(v, |x| x.max(v))))
                    {
                        Some(h) => gpui::GridTrack::Pixels(px(h)),
                        None => gpui::GridTrack::Fraction(1.0),
                    }
                })
                .collect(),
        ),
        _ => None,
    };
    // Стол БЕЗ заданной высоты, но с рядами заданной высоты: дорожка такого
    // ряда — `minmax(h, auto)` (CSS 2.1 §17.5.3: высота ряда — большее из
    // заданной и нужной ячейкам), прочие — `auto`. Без дорожек высота ряда
    // не доезжала до сетки вовсе: `tr {height: 50px}` с пустыми ячейками
    // давал ряд в 2 точки паддинга (`table-as-item-cell-percentage-001/003/
    // 004`: стол 100×4 вместо 100×100). Это НЕ откатанный вариант «дорожки
    // рядов и без table_tall» (★ выше): там авто-ряды становились долями
    // `1fr` с `flex_grow`, и ряды растягивались на высоту растянутого стола;
    // здесь авто-ряд остаётся `auto`, а пол — только у ряда с высотой.
    let row_floors: Option<Vec<gpui::GridTrack>> = (row_tracks.is_none()
        && e.style.vertical != Some(true)
        && row_elements
            .iter()
            .any(|r| matches!(r.style.height, Some(Len::Px(h)) if h > 0.0)))
    .then(|| {
        row_elements
            .iter()
            .map(|row| match row.style.height {
                Some(Len::Px(h)) if h > 0.0 => gpui::GridTrack::MinMax(Box::new((
                    gpui::GridTrack::Pixels(px(h)),
                    gpui::GridTrack::Auto,
                ))),
                _ => gpui::GridTrack::Auto,
            })
            .collect()
    });

    if e.style.vertical == Some(true) {
        // Ряд таблицы — КОЛОНКА сетки: заполнение идёт сверху вниз, ряд за
        // рядом поперёк (css-writing-modes-3 §8, table-progression-*).
        let mut g = div().grid().grid_template_rows(tracks);
        g.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
        g
    } else {
        let mut g = div().grid().grid_template_cols(tracks);
        if let Some(rt) = row_tracks {
            // Сетка обязана занять ВСЮ высоту таблицы: доли рядов считаются
            // от её остатка, а auto-высота ребёнка гибкой колонки — ноль.
            g = g.grid_template_rows(rt).flex_grow_1();
        } else if let Some(rt) = row_floors {
            // Полы рядов (см. `row_floors`); растяжение элемента гибкого
            // контейнера — как в ветке ниже.
            g = g.grid_template_rows(rt);
            if inherited.flex_item {
                g = g.flex_grow_1();
            }
        } else if inherited.flex_item {
            // Стол — элемент гибкого контейнера: высоту, данную ему ростом
            // или растяжением, делят ряды (CSS 2.1 §17.5.3; у сетки
            // `align-content: normal` = stretch тянет auto-ряды), иначе ячейки
            // оставались по содержимому (`table-as-item-stretch-cross-size-2`).
            g = g.flex_grow_1();
        }
        g
    }
}

fn paint_table_border(
    table_edges: std::rc::Rc<std::cell::RefCell<Vec<super::paint::EdgeCell>>>,
    e: &Element,
    bw: [f32; 4],
    outer_win: [f32; 4],
    collapse: bool,
    inherited: &Computed,
    mut outer: gpui::Div,
) -> gpui::Div {
    if collapse && (bw.iter().any(|w| *w > 0.0) || e.style.border_side_styles.contains(&Some(1))) {
        // Рамка самой таблицы — участник разбора конфликтов: её кромки
        // уходят в тот же слой (EdgePainter), линии — внутренние края
        // рамочного места, победившая кромка рисуется наружу.
        let black = crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let side_colour = |i: usize| {
            e.style.border_colors[i]
                .or(e.style.border_color)
                .or(inherited.color)
                .unwrap_or(black)
        };
        let colors = [
            side_colour(0),
            side_colour(1),
            side_colour(2),
            side_colour(3),
        ];
        let side_style =
            |i: usize| e.style.border_side_styles[i].unwrap_or(if bw[i] > 0.0 { 9 } else { 0 });
        let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
        // Линия рамки СТОЛА — та же ЛИНИЯ СЕТКИ, на которой стоят кромки
        // краевых ячеек (§17.6.2: «borders are centered on the grid lines»).
        // Коробка стола вжата внутрь на ПОЛОВИНУ ПОБЕДИВШЕЙ кромки — ровно
        // `outer_win/2` лёг выше в её паддинг, — поэтому и проба вжимается на
        // неё, а не на собственную толщину `bw`. Прежний вжим на `bw` разводил
        // кромку стола и кромки ячеек по РАЗНЫМ группам линий (при
        // `outer_win == bw` — ровно на `bw/2`, то есть на любой рамке от 1.5
        // точек), и разбор конфликта §17.6.2.1 между ними не применялся ни
        // разу: полосы совпадали на экране, а цвет решал порядок рисования —
        // ячейка красилась поверх стола. Снимок `border-conflict-resolution`:
        // нижняя полоса y 191..196 приборных у нас `G67@13 R232@80 G1@312`
        // при `G300@13` у эталона, и 3278 + 1392 = 4670 — смещённых точек нет.
        let half = [
            outer_win[0] / 2.0,
            outer_win[1] / 2.0,
            outer_win[2] / 2.0,
            outer_win[3] / 2.0,
        ];
        outer = outer.child(crate::layout::table::paint::edge_probe(
            table_edges.clone(),
            bw,
            colors,
            styles,
            0,
            e.node_id as u32,
            half,
        ));
    }
    outer
}

fn wrap_with_captions(
    e: &Element,
    caps_top: Vec<AnyElement>,
    caps_bot: Vec<AnyElement>,
    inherited: &Computed,
    outer: gpui::Div,
) -> AnyElement {
    if caps_top.is_empty() && caps_bot.is_empty() {
        outer.into_any_element()
    } else {
        // `caption-side: top/bottom` — стороны block-start/block-end стола
        // (css-writing-modes-4 §6, особое исключение для caption-side): в
        // вертикальном письме заголовок стоит СБОКУ — справа при `vertical-rl`,
        // слева при `vertical-lr` (`caption-side-vrl-002`).
        let vertical = e.style.vertical == Some(true);
        let mut wrap = if vertical {
            div().flex().flex_row()
        } else {
            div().flex().flex_col()
        };
        // Элемент гибкого контейнера у стола с подписями — ОБЁРТКА
        // (css-flexbox-1 §4: «the table wrapper box becomes the flex item, and
        // the order and align-self properties apply to it … the flex item's
        // final size is calculated … as if the distance between the table
        // wrapper box's edges and the table box's content edges were all part
        // of the table box's border+padding area»). Рост и сжатие уходят на
        // обёртку, стол внутри неё забирает остаток и растягивается по её
        // ширине — подписи и стол одной ширины. Основа остаётся на столе: в
        // колонке `wrap` его главная ось та же, что у контейнера-колонки
        // (`table-as-item-inflexible-in-column-2`). Прижим `FlexStart` —
        // только вне гибкого контейнера: там он даёт сжатие по содержимому
        // (§17.5.2), а в гибком контейнере отбирал растяжение
        // (`table-as-item-stretch-cross-size*`, `-flex-cross-size`).
        let mut outer = outer;
        if inherited.flex_item && !vertical {
            let s = outer.style();
            let grow = s.flex_grow.take();
            let shrink = s.flex_shrink.take();
            let own_align = s.align_self.take();
            s.flex_grow = Some(1.0);
            let w = wrap.style();
            w.flex_grow = grow;
            w.flex_shrink = shrink;
            w.align_self = if e.style.align_self.is_some() {
                own_align
            } else {
                None
            };
            // Основа в РЯДУ: главная ось контейнера — строчная ось обёртки,
            // и основа, оставленная на столе внутри колонки `wrap`, там не
            // действует (у колонки это поперечная ось). Переносится на
            // обёртку вместе с рамкой и отбивкой стола content-box —
            // css-flexbox-1 §4: «as if the distance between the table wrapper
            // box's edges and the table box's content edges were all part of
            // the table box's border+padding area»
            // (`table-as-item-inflexible-in-row-2`: `flex: 0 0 80px; border:
            // 10px solid` — стол выходил 20 точек вместо 100).
            if !matches!(
                inherited.flex_dir,
                Some(FlexDir::Col) | Some(FlexDir::ColReverse)
            ) && let Some(Len::Px(b)) = e.style.flex_basis
            {
                s.flex_basis = None;
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                let bd = e.style.borders();
                let edges = if e.style.border_box == Some(true) {
                    0.0
                } else {
                    side(bd.left)
                        + side(bd.right)
                        + side(e.style.padding.left)
                        + side(e.style.padding.right)
                };
                w.flex_basis = Some(gpui::Length::Definite(px(b + edges).into()));
            }
        } else {
            wrap.style().align_self = Some(gpui::AlignItems::FlexStart);
        }
        // Инлайн-размер ОБЁРТКИ — инлайн-размер САМОГО стола, а не наоборот.
        // CSS 2.1 §17.4: «The width of the table wrapper box is the border-edge
        // width of the table grid box inside it … Percentages on 'width' and
        // 'height' on the table are relative to the table wrapper box's
        // containing block, NOT the table wrapper box itself» (то же
        // css-tables-3 §fixup-algorithm). Доля коробки рядов опиралась на
        // обёртку, а обёртка — гибкий КОРЕНЬ копии фрагмента
        // (`flow.rs` `layout_as_root`), и taffy тянет по доступному месту
        // только БЛОЧНЫЙ корень (`vendor/taffy/src/compute/mod.rs:68`
        // `if style.is_block()`): стол с `inline-size: 100%` и подписью
        // схлопывался до ширины содержимого — 20 css у
        // `table-grid-paint-htb-ltr` (две рамки) и 34.4 у `table-row-paint-htb-ltr`
        // (один `border-spacing`). Blink держит один размер на стол и обёртку:
        // `table_layout_algorithm.cc:139` `available_size = {table_inline_size,
        // kIndefiniteSize}` и `:71` `builder.SetAvailableSize(available_size)`.
        // Берём только точки и долю: `em`/`ch` у обёртки считались бы по ЧУЖОМУ
        // шрифту (`apply::len_to_gpui` ветка запасных величин), а стол с
        // `width: auto` обязан остаться сжатым по содержимому (§17.5.2).
        match if vertical {
            e.style.height
        } else {
            e.style.width
        } {
            Some(Len::Px(v)) if vertical => wrap = wrap.h(px(v)),
            Some(Len::Px(v)) => wrap = wrap.w(px(v)),
            Some(Len::Pct(v)) if vertical => wrap = wrap.h(gpui::relative(v)),
            Some(Len::Pct(v)) => wrap = wrap.w(gpui::relative(v)),
            _ => {}
        }
        // В ряду второй ребёнок сжимался в ноль — каждому своя ширина.
        let own_width = |x: AnyElement| -> AnyElement {
            if vertical {
                div().flex_shrink_0().child(x).into_any_element()
            } else {
                x
            }
        };
        // Порядок обёртки — верхние подписи, коробка рядов, нижние подписи
        // (css-tables-3 §terminology; Blink `table_layout_algorithm.cc:988`
        // и `:1584`). Тот же порядок мерит `table_shape`.
        let mut cap_wrap: Vec<AnyElement> = Vec::new();
        for cap_el in caps_top {
            cap_wrap.push(own_width(cap_el));
        }
        // Стол не уже подписи (css-tables-3 §computing-the-table-width:
        // «the used min-width of a table is the greater of the resolved
        // min-width, CAPMIN, and GRIDMIN»). Обёртка сжата по содержимому,
        // значит она шириной с самую широкую из подписи и стола, и растяжка
        // стола по ней даёт ровно max(стол, CAPMIN) — но только когда CAPMIN
        // известен, то есть у каждой подписи ширина в точках. Подпись
        // текстом мерилась бы max-content, а не min-content, и распирала бы
        // колонки (`anonymous-table-box-width-001`: пустая подпись в 100
        // точек, рамка `border-bottom: 100px` у стола нулевой ширины).
        let caps_px = e.children.iter().all(|n| match n {
            Node::Element(c) if c.tag == "caption" || c.style.is_caption == Some(true) => {
                matches!(c.style.width, Some(Len::Px(_)))
            }
            _ => true,
        });
        let mut outer = outer;
        if !vertical && caps_px && e.style.width.is_none() && e.style.align_self.is_none() {
            outer.style().align_self = Some(gpui::AlignItems::Stretch);
        }
        cap_wrap.push(own_width(outer.into_any_element()));
        for cap_el in caps_bot {
            cap_wrap.push(own_width(cap_el));
        }
        // Заголовок ПЕРЕД коробкой по порядку детей = у начала оси: для vrl
        // начало блочной оси — правый край, а ряд идёт слева направо, значит
        // переворачивается ВЕСЬ порядок обёртки. Прежнее
        // `cap_first = caption_bottom == (vertical && vertical_rl)` — тот же
        // разворот, записанный для ОДНОЙ подписи.
        if vertical && e.style.vertical_rl == Some(true) {
            cap_wrap.reverse();
        }
        wrap.style().no_inline_block_baseline = Some(true);
        wrap.children(cap_wrap).into_any_element()
    }
}
