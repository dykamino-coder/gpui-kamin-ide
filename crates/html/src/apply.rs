//! Вычисленный стиль → элемент GPUI. Здесь и проходит граница охвата.
//!
//! Правило одно: если свойство выразимо примитивами GPUI — применяем; если нет
//! — не применяем НИЧЕГО вместо него. Приблизительная замена (нарисовать
//! `filter: blur` полупрозрачностью, `inset`-тень внешней) выглядит как рабочая
//! поддержка и стоит дороже честного пропуска: расхождение всплывает у
//! пользователя, а не в тесте.

use crate::computed::{
    Align, AutoFlow, Computed, Display, FlexDir, Gradient, Justify, Overflow, Placement, Position,
    Sides, TextAlign, Track, TrackSize,
};
use crate::value::Len;
use gpui::{Div, InteractiveElement, Styled, px, relative};

/// Ширина/высота/отступ: доля родителя или пиксели.
fn len_to_gpui(l: Len) -> gpui::DefiniteLength {
    match l {
        Len::Px(v) => px(v).into(),
        Len::Pct(v) => relative(v),
        // Сюда `em` доходит только у узлов вне наследования (элементы форм,
        // корень) — считаем от базового кегля, как браузер от `:root`.
        Len::Em(k) => px(k * 16.0).into(),
        Len::EmPx(k, add) => px(k * 16.0 + add).into(),
        // Тем же путём доходят `ch` и `ex`: семейство шрифта здесь неизвестно,
        // поэтому работает запасное значение спецификации.
        Len::Ch(k) => px(k * crate::metrics::ch_ex_px("", 16.0).0).into(),
        Len::Ic(k) => px(k * crate::metrics::ic_px("", 16.0)).into(),
        Len::Ex(k) => px(k * crate::metrics::ch_ex_px("", 16.0).1).into(),
        // Неразрешённый `lh` — от базовой строки в 1.2 кегля.
        Len::Lh(k) => px(k * 1.2 * 16.0).into(),
        Len::LhPx(k, add) => px(k * 1.2 * 16.0 + add).into(),
        // Единицы окна разрешает сборщик дерева; сюда они доходят только у
        // узлов вне его — доля родителя ближе всего по смыслу.
        Len::Vw(k) | Len::Vh(k) => relative(k),
        // `auto` в размере значит «пусть решает раскладка» — это отсутствие
        // ограничения, а не значение; вызывающий такие поля не применяет.
        // Размер по содержимому — то же самое: его ставит обёртка-сетка
        // (`render::content_sized`), а не длина.
        Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => relative(1.0),
    }
}

/// Дорожка сетки в терминах GPUI. Нижняя грань всегда `min-content`: без неё
/// колонка на узкой панели схлопывается в ноль и содержимое обрезается.
/// Одна грань дорожки.
fn bound(t: &Track) -> gpui::GridTrack {
    match t {
        Track::Px(v) => gpui::GridTrack::Pixels(px(*v)),
        Track::Auto => gpui::GridTrack::Auto,
        Track::MinContent => gpui::GridTrack::MinContent,
        Track::MaxContent => gpui::GridTrack::MaxContent,
        Track::Fr(f) => gpui::GridTrack::Fraction(*f),
        Track::Pct(p) => gpui::GridTrack::Percent(*p),
    }
}

/// Дорожка сетки: одиночная либо пара граней.
///
/// Одиночная переносится как есть — оборачивать её в `minmax` нельзя, иначе
/// нижняя грань разрешает колонке вырасти сверх заданного (поймано сравнением
/// с Chrome: колонка 120px выходила 200). Исключение — доля свободного места:
/// `1fr` в CSS и есть `minmax(auto, 1fr)`, иначе она схлопывается под
/// содержимым.
fn track(t: &TrackSize) -> gpui::GridTrack {
    match t {
        TrackSize::MinMax(lo, hi) => gpui::GridTrack::MinMax(Box::new((bound(lo), bound(hi)))),
        TrackSize::Single(Track::Fr(f)) => gpui::GridTrack::MinMax(Box::new((
            gpui::GridTrack::Auto,
            gpui::GridTrack::Fraction(*f),
        ))),
        TrackSize::Single(one) => bound(one),
    }
}

/// Стиль наведения: `.btn:hover { … }`.
///
/// Отдельная функция, потому что GPUI принимает состояние наведения не
/// цепочкой методов, а правкой стиля в замыкании. Поддержано подмножество,
/// которое и встречается в наведении: цвет, фон, рамка, прозрачность, вес и
/// начертание шрифта. Отступы и размеры в наведении менять нельзя — это
/// сдвинуло бы раскладку под курсором.
pub fn apply_hover(d: Div, hover: &Computed) -> Div {
    let h = hover.clone();
    d.hover(move |mut s| {
        if let Some(bg) = h.background {
            s.background = Some(gpui::Fill::Color(bg.to_hsla().into()));
        }
        if let Some(g) = &h.gradient {
            s.background = Some(gpui::Fill::Color(fill(g)));
        }
        if let Some(bc) = h.border_color {
            s.border_color = Some(bc.to_hsla());
        }
        if let Some(o) = h.opacity {
            s.opacity = Some(o);
        }
        if let Some(col) = h.color {
            s.text.get_or_insert_with(Default::default).color = Some(col.to_hsla());
        }
        if let Some(w) = h.font_weight {
            s.text.get_or_insert_with(Default::default).font_weight =
                Some(gpui::FontWeight(w as f32));
        }
        if h.italic == Some(true) {
            s.text.get_or_insert_with(Default::default).font_style = Some(gpui::FontStyle::Italic);
        }
        s
    })
}

/// `justify-content`/`align-content` → распределение GPUI.
fn to_content(j: Justify) -> gpui::AlignContent {
    match j {
        Justify::Center => gpui::AlignContent::Center,
        Justify::Start => gpui::AlignContent::FlexStart,
        Justify::End => gpui::AlignContent::FlexEnd,
        // Начало и конец ОСИ ПИСЬМА: у раскладки это отдельные значения, и
        // при обратном направлении ряда они не совпадают с гибкими.
        Justify::WmStart => gpui::AlignContent::Start,
        Justify::WmEnd => gpui::AlignContent::End,
        Justify::Between => gpui::AlignContent::SpaceBetween,
        Justify::Around => gpui::AlignContent::SpaceAround,
        // `space-evenly` отличается от `space-around` шириной крайних
        // промежутков — сводить их в одно значение нельзя.
        Justify::Evenly => gpui::AlignContent::SpaceEvenly,
        Justify::Stretch => gpui::AlignContent::Stretch,
    }
}

fn to_items(a: Align) -> gpui::AlignItems {
    match a {
        Align::Center => gpui::AlignItems::Center,
        Align::Start => gpui::AlignItems::FlexStart,
        Align::End => gpui::AlignItems::FlexEnd,
        Align::Stretch => gpui::AlignItems::Stretch,
        Align::Baseline => gpui::AlignItems::Baseline,
    }
}

fn to_placement(p: Placement) -> gpui::GridPlacement {
    match p {
        Placement::Auto => gpui::GridPlacement::Auto,
        Placement::Line(n) => gpui::GridPlacement::Line(n),
        Placement::Span(n) => gpui::GridPlacement::Span(n),
    }
}

/// Заливка градиентом: радиальный — своим тегом (патч GPUI), линейный —
/// парой крайних стопов; промежуточные рисует сборщик дерева полосами.
pub fn fill(g: &Gradient) -> gpui::Background {
    let last = g.stops.len().saturating_sub(1);
    let (from, to) = (
        gpui::linear_color_stop(
            g.from.to_hsla(),
            g.stops.first().map(|s| s.1).unwrap_or(0.0),
        ),
        gpui::linear_color_stop(
            g.to.to_hsla(),
            g.stops.get(last).map(|s| s.1).unwrap_or(1.0),
        ),
    );
    let base = if g.radial {
        gpui::radial_gradient(from, to, g.circle)
    } else {
        gpui::linear_gradient(g.angle_deg, from, to)
    };
    // Промежуточные цвета: до четырёх стопов заливка несёт сама (патч GPUI),
    // сверх того сборщик дерева по-прежнему кладёт полосы.
    if g.stops.len() > 2 {
        let stops: Vec<gpui::LinearColorStop> = g
            .stops
            .iter()
            .take(4)
            .map(|(c, p)| gpui::linear_color_stop(c.to_hsla(), *p))
            .collect();
        return base.with_stops(&stops);
    }
    base
}

pub fn apply(d: Div, c: &Computed) -> Div {
    let mut d = d;
    d = apply_layout(d, c);
    d = apply_box(d, c);
    d = apply_paint(d, c);
    apply_text(d, c)
}

/// Стиль контейнера-сетки: дорожки, неявные дорожки, направление.
fn grid_style(mut d: Div, c: &Computed) -> Div {
    d = d.grid();
    // Оси сетки ЛОГИЧЕСКИЕ: «колонки» идут вдоль строки, «ряды» — вдоль
    // потока. При вертикальном письме строка идёт сверху вниз, а поток —
    // поперёк, поэтому колонки становятся физическими рядами и наоборот.
    // Раскладка под нами письма не знает и считает оси физическими, так что
    // переставляем здесь, на границе.
    let flip = c.vertical == Some(true);
    let along_line = |d: Div, tracks: Vec<gpui::GridTrack>| -> Div {
        if flip {
            d.grid_template_rows(tracks)
        } else {
            d.grid_template_cols(tracks)
        }
    };
    // Список дорожек точнее числа колонок: он несёт ширину по
    // содержимому и фиксированные колонки (патч GPUI, см. доку).
    // Доля дорожки в `repeat(auto-fill, 25%)` считается от размера контейнера,
    // а он известен прямо здесь: `25%` в трёхстах точках — четыре дорожки по
    // 75. Пока доля отбрасывалась, сетка не получала дорожек вовсе
    // (`column-auto-repeat-002` и родня).
    let auto_fill = c.grid_auto_fill_min.or_else(|| {
        let k = c.auto_repeat_cols?.track_pct?;
        match c.width {
            Some(Len::Px(w)) => Some(k * w),
            _ => None,
        }
    });
    // ПРОБОВАЛИ И ОТКАТИЛИ: `grid-template-columns: subgrid` разворачивать в
    // столько СВОИХ дорожек, сколько линий родителя элемент перекрывает.
    // Семейству subgrid-gap +10, но −17 по subgrid-auto-fill и базовым линиям:
    // прежде зелёные пары совпадали с эталоном ИМЕННО одноколоночным
    // поведением, а свои дорожки без настоящих ширин родителя их разломали.
    // Возвращаться только с настоящей передачей дорожек родителя вниз.
    match (&c.grid_tracks, c.grid_cols, auto_fill) {
        (Some(tracks), _, _) => d = along_line(d, tracks.iter().map(track).collect()),
        // «Сколько влезет» умеет сама раскладка — короткая форма GPUI.
        (None, _, Some(min)) if !flip => d = d.grid_cols_min(px(min)),
        (None, Some(n), _) if !flip => d = d.grid_cols(n),
        (None, Some(n), _) => {
            d = d.grid_template_rows((0..n).map(|_| gpui::GridTrack::Auto).collect())
        }
        _ => {}
    }
    if let Some(rows) = &c.grid_rows {
        let tracks: Vec<gpui::GridTrack> = rows.iter().map(track).collect();
        d = if flip {
            d.grid_template_cols(tracks)
        } else {
            d.grid_template_rows(tracks)
        };
    }
    // Неявные дорожки: элементов больше, чем описано — их размер задаёт
    // `grid-auto-*`, иначе они выходят по содержимому.
    let (auto_line, auto_flow_axis) = if flip {
        (&c.grid_auto_cols, &c.grid_auto_rows)
    } else {
        (&c.grid_auto_rows, &c.grid_auto_cols)
    };
    if let Some(t) = auto_line {
        d.style().grid_auto_rows = Some(track(t));
    }
    if let Some(t) = auto_flow_axis {
        d.style().grid_auto_cols = Some(track(t));
    }
    if let Some(f) = c.grid_auto_flow {
        // Направление наполнения тоже логическое: «по рядам» значит «вдоль
        // строки», а строка при вертикальном письме идёт сверху вниз.
        let f = if flip {
            match f {
                AutoFlow::Row => AutoFlow::Col,
                AutoFlow::Col => AutoFlow::Row,
                AutoFlow::RowDense => AutoFlow::ColDense,
                AutoFlow::ColDense => AutoFlow::RowDense,
            }
        } else {
            f
        };
        d.style().grid_auto_flow = Some(match f {
            AutoFlow::Row => gpui::GridAutoFlow::Row,
            AutoFlow::Col => gpui::GridAutoFlow::Column,
            AutoFlow::RowDense => gpui::GridAutoFlow::RowDense,
            AutoFlow::ColDense => gpui::GridAutoFlow::ColumnDense,
        });
    }
    d
}

fn apply_layout(mut d: Div, c: &Computed) -> Div {
    match c.display {
        // Блок в GPUI — дефолт; отдельного вызова не требует.
        Some(Display::Flex) | Some(Display::InlineFlex) => d = d.flex(),
        // Инлайновая коробка в строке не растягивается по ширине родителя.
        Some(Display::InlineBlock) => d = d.flex_shrink_0(),
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
        (None, true)
            if matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex)) =>
        {
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
    let dir = if c.vertical == Some(true)
        && c.sideways == Some(true)
        && c.vertical_rl != Some(true)
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
    if c.flex_wrap == Some(true) {
        d = d.flex_wrap();
        // При `vertical-rl` поперечная ось строки идёт справа налево — обратно
        // тому, как переносит колонку раскладка. Перенос разворачивается,
        // иначе вторая строка уходит не в ту сторону.
        // Поперечная ось — та, что осталась. У РЯДА (ось строки) это ось
        // потока: она разворачивается при `vertical-rl`. У КОЛОНКИ (ось
        // потока) поперечная — ось строки, и её разворачивает `direction: rtl`
        // (`flexbox-writing-mode-004/005`: контейнеры-колонки шли зеркально).
        let flip = if matches!(c.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse)) {
            c.rtl == Some(true)
        } else {
            c.vertical_rl == Some(true)
        };
        if (c.flex_wrap_reverse == Some(true)) != flip {
            d.style().flex_wrap = Some(gpui::FlexWrap::WrapReverse);
        }
    }
    if c.grid_col.is_some() || c.grid_row.is_some() {
        let span = |p: Option<(Placement, Placement)>| {
            let (a, b) = p.unwrap_or((Placement::Auto, Placement::Auto));
            to_placement(a)..to_placement(b)
        };
        d.style().grid_location = Some(gpui::GridLocation {
            row: span(c.grid_row),
            column: span(c.grid_col),
        });
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
    // В письме `vertical-rl` поперечная ось идёт СПРАВА НАЛЕВО: её начало —
    // правый край. У гибкой раскладки обратной поперечной оси нет, поэтому
    // элемент, который не растягивается, прижимается к концу — это и есть
    // правый край (замерено пробой: ряд в `vertical-rl` против колонки с
    // прижимом вправо расходился на 5.42%).
    if c.vertical_rl == Some(true)
        && c.align_items.is_none()
        && matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex))
    {
        d = d.items_end();
    }
    match c.align_items {
        Some(Align::Center) => d = d.items_center(),
        Some(Align::Start) => d = d.items_start(),
        Some(Align::End) => d = d.items_end(),
        Some(Align::Baseline) => d = d.items_baseline(),
        Some(Align::Stretch) | None => {}
    }
    // `align-self` — про САМ элемент, а не про его детей. Раньше оба свойства
    // писались в одно поле, и элемент выравнивал содержимое вместо себя.
    if let Some(a) = c.align_self {
        d.style().align_self = Some(match a {
            Align::Center => gpui::AlignItems::Center,
            Align::Start => gpui::AlignItems::FlexStart,
            Align::End => gpui::AlignItems::FlexEnd,
            Align::Baseline => gpui::AlignItems::Baseline,
            Align::Stretch => gpui::AlignItems::Stretch,
        });
    }
    // `flex-basis: auto` — это ОТСУТСТВИЕ основы, а не «во всю ширину»:
    // без отсева `flex: none` растягивал кнопку на всю строку.
    if let Some(b) = c.flex_basis.filter(|b| *b != Len::Auto) {
        d = d.flex_basis(len_to_gpui(b));
    }
    if let Some(j) = c.justify_content {
        d.style().justify_content = Some(to_content(j));
    }
    // `align-content` — распределение СТРОК, когда их несколько: без него
    // перенесённые строки прижимались к началу вместо заданного распределения.
    if let Some(a) = c.align_content {
        d.style().align_content = Some(to_content(a));
    }
    if let Some(a) = c.justify_items {
        d.style().justify_items = Some(to_items(a));
    }
    if let Some(a) = c.justify_self {
        d.style().justify_self = Some(to_items(a));
    }
    if let Some(r) = c.aspect_ratio {
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
        if let Some(r) = down {
            d = d.gap_y(len_to_gpui(r));
        }
        if let Some(cg) = across {
            d = d.gap_x(len_to_gpui(cg));
        }
    }

    // Движок раскладки всегда трактует размер как `border-box`, а CSS по
    // умолчанию — как `content-box`: заданная ширина не включает отступы и
    // рамку. Без компенсации блок с рамкой 4px выходил на 8 точек уже, чем в
    // браузере, и всё правее него уезжало (поймано сравнением с Chrome).
    let content_box = c.border_box != Some(true);
    let extra = |sides: &[Option<Len>]| -> f32 {
        if !content_box {
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

    for (val, f) in [
        (c.width, 0u8),
        (c.height, 1),
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
        // Доли считаются от родителя и компенсации не требуют.
        let l = match l {
            Len::Px(v) if f % 2 == 0 => Len::Px(v + pad_x),
            Len::Px(v) => Len::Px(v + pad_y),
            other => other,
        };
        let g = len_to_gpui(l);
        d = match f {
            0 => d.w(g),
            1 => d.h(g),
            2 => d.min_w(g),
            3 => d.min_h(g),
            4 => d.max_w(g),
            _ => d.max_h(g),
        };
    }
    d
}

fn apply_box(mut d: Div, c: &Computed) -> Div {
    // `contain: size`: коробка меряется как пустая — рост от содержимого
    // подменяется `contain-intrinsic-size` (или нулём). Подмена касается
    // размера ПО СОДЕРЖИМОМУ: высота auto считается от содержимого — её и
    // задаём; ширина блока в потоке и так не от содержимого, её не трогаем.
    if c.contain_size == Some(true) && c.height.is_none() {
        d = d.h(px(c.contain_intrinsic.1.unwrap_or(0.0)));
    }
    d = apply_sides(d, &c.padding, SideKind::Padding);
    d = apply_sides(d, &c.margin, SideKind::Margin);
    d = apply_sides(d, &c.borders(), SideKind::Border);
    d = apply_radius(d, c);
    // `clip-path: circle()` — обрезка содержимого по кругу. Прямоугольная
    // обрезка со скруглением — единственная в конвейере, но для круга и
    // эллипса она точна.
    if let Some(round) = c.clip_round {
        let base = match (c.width, c.height) {
            (Some(Len::Px(w)), Some(Len::Px(h))) => w.min(h),
            (Some(Len::Px(w)), _) => w,
            (_, Some(Len::Px(h))) => h,
            _ => 0.0,
        };
        // Доля без известного размера — это «половина стороны», то есть
        // заведомо большое значение: растеризатор обрежет его сам. Раньше
        // 0.5 понималось как полпикселя, и круг выходил квадратом.
        let radius = if round <= 1.0 {
            if base > 0.0 {
                round * base
            } else {
                9999.0 * round
            }
        } else {
            round
        };
        // Обрезка НЕ отменяет собственное скругление: берётся более сильное
        // из двух, иначе `border-radius` рядом с `clip-path` пропадал.
        let own = match c.radius.tl {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        d = d.rounded(px(radius.max(own))).overflow_hidden();
    }
    // `contain: paint` — содержимое не выходит за коробку.
    if c.contain_paint == Some(true) {
        d = d.overflow_hidden();
    }

    match c.position {
        // Слой окна для `fixed` создаёт сборщик дерева; внутри него элемент
        // размещается так же, как абсолютный.
        Some(Position::Fixed) | Some(Position::Absolute) => {
            d = d.absolute();
            // Абсолютный элемент, у которого задан только один край, не имеет
            // определённой ширины — раскладка сжимает его до самого узкого
            // содержимого, и текст встаёт столбиком по букве. В браузере такой
            // элемент занимает ширину содержимого без переносов; повторяем это.
            let horizontal = c.inset.left.is_some() && c.inset.right.is_some();
            if !horizontal && c.width.is_none() {
                d = d.flex_shrink_0().whitespace_nowrap();
            }
        }
        Some(Position::Relative) => d = d.relative(),
        // Липкий остаётся в потоке: край для него — порог прилипания, а не
        // сдвиг, поэтому вставки ниже к нему не применяются.
        Some(Position::Sticky) => return d.relative(),
        // `static` в GPUI недостижим: элемент всегда участвует в потоке
        // относительно родителя, что соответствует `relative`.
        _ => {}
    }
    // Обрезка — ДО разбора краёв: ниже стоит ранний выход для непозиционированных,
    // и всё, что после него, у обычного блока не выполнялось вовсе. Из-за этого
    // `overflow: hidden` не обрезал ничего (проверено пробой: коробка с ним и
    // без него рисовались одинаково).
    //
    // Прокрутка: в GPUI скролл требует своего состояния и обработчика, поэтому
    // на уровне стиля выражается только обрезка. Прокручиваемый контейнер
    // собирается вызывающим (см. доку, раздел «Прокрутка»).
    // Обрезка с ПОЛЕМ снимается с коробки: раскладка режет ровно по её краю,
    // а поле требует резать дальше наружу. Маску ставит свой слой
    // (`interact::ClipMargin`), его заводит сборщик дерева.
    if c.overflow_x == Some(Overflow::Hidden) || c.overflow_x == Some(Overflow::Scroll) {
        d = d.overflow_x_hidden();
    }
    if c.overflow_y == Some(Overflow::Hidden) || c.overflow_y == Some(Overflow::Scroll) {
        d = d.overflow_y_hidden();
    }
    // Поле обрезки: край отодвигается от коробки отсчёта (css-overflow-3 §5).
    // Сдвиг несёт РОДНАЯ маска (патч GPUI): рамка и фон самой коробки
    // рисуются вне маски, режется только содержимое — обёртка снаружи резала
    // и рамку.
    if let Some(m) = c.clip_margin {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let border = c.borders();
        let b = [side(border.top), side(border.right), side(border.bottom), side(border.left)];
        let pd = [
            side(c.padding.top),
            side(c.padding.right),
            side(c.padding.bottom),
            side(c.padding.left),
        ];
        let arr = match c.clip_margin_box {
            Some(2) => [b[0] + m, b[1] + m, b[2] + m, b[3] + m],
            Some(0) => [m - pd[0], m - pd[1], m - pd[2], m - pd[3]],
            _ => [m; 4],
        };
        d.style().overflow_clip_offset = Some(arr);
    }
    // Края двигают только позиционированный элемент. У обычного (`static`)
    // браузер их игнорирует, а мы сдвигали — блок с `top` в потоке уезжал.
    if !matches!(
        c.position,
        Some(Position::Relative) | Some(Position::Absolute) | Some(Position::Fixed)
    ) {
        return d;
    }
    for (val, f) in [
        (c.inset.top, 0u8),
        (c.inset.right, 1),
        (c.inset.bottom, 2),
        (c.inset.left, 3),
    ] {
        let Some(l) = val else { continue };
        if l == Len::Auto {
            continue;
        }
        let g = len_to_gpui(l);
        d = match f {
            0 => d.top(g),
            1 => d.right(g),
            2 => d.bottom(g),
            _ => d.left(g),
        };
    }

    // `translate` двигает элемент ВИЗУАЛЬНО, не трогая раскладку. Считается
    // ПОСЛЕ краёв и складывается с ними: раньше цикл краёв затирал сдвиг, и
    // у абсолютного элемента с `left` он пропадал.
    if let Some((x, y)) = c.translate {
        let shift = |base: Option<Len>, delta: Len| -> Option<Len> {
            match (base, delta) {
                (Some(Len::Px(b)), Len::Px(v)) => Some(Len::Px(b + v)),
                (None, Len::Px(v)) if v != 0.0 => Some(Len::Px(v)),
                (base, _) => base,
            }
        };
        if c.position.is_none() {
            d = d.relative();
        }
        if let Some(l) = shift(c.inset.left, x) {
            d = d.left(len_to_gpui(l));
        }
        if let Some(t) = shift(c.inset.top, y) {
            d = d.top(len_to_gpui(t));
        }
    }

    d
}

/// Наружные отступы отдельно от остального стиля.
///
/// Нужно ленте прокрутки: её видимая область — это коробка БЕЗ наружных
/// отступов, и когда отступ оставался на прокручиваемом узле, лента считала
/// его своей высотой и показывала лишнее.
pub fn margins(d: Div, s: &Sides) -> Div {
    apply_sides(d, s, SideKind::Margin)
}

enum SideKind {
    Padding,
    Margin,
    Border,
}

fn apply_sides(mut d: Div, s: &Sides, kind: SideKind) -> Div {
    for (val, side) in [(s.top, 0u8), (s.right, 1), (s.bottom, 2), (s.left, 3)] {
        let Some(l) = val else { continue };
        // `margin: auto` — это центрирование блока, а не «нет значения».
        // Отступы и рамки с `auto` смысла не имеют, их пропускаем.
        if l == Len::Auto {
            if matches!(kind, SideKind::Margin) {
                d = match side {
                    0 => d.mt(gpui::Length::Auto),
                    1 => d.mr(gpui::Length::Auto),
                    2 => d.mb(gpui::Length::Auto),
                    _ => d.ml(gpui::Length::Auto),
                };
            }
            continue;
        }
        let g = len_to_gpui(l);
        d = match (&kind, side) {
            (SideKind::Padding, 0) => d.pt(g),
            (SideKind::Padding, 1) => d.pr(g),
            (SideKind::Padding, 2) => d.pb(g),
            (SideKind::Padding, _) => d.pl(g),
            (SideKind::Margin, 0) => d.mt(g),
            (SideKind::Margin, 1) => d.mr(g),
            (SideKind::Margin, 2) => d.mb(g),
            (SideKind::Margin, _) => d.ml(g),
            // Толщина рамки в GPUI задаётся только абсолютной длиной.
            (SideKind::Border, side) => match (l, side) {
                (Len::Px(v), 0) => d.border_t_1().border_t(px(v)),
                (Len::Px(v), 1) => d.border_r_1().border_r(px(v)),
                (Len::Px(v), 2) => d.border_b_1().border_b(px(v)),
                (Len::Px(v), _) => d.border_l_1().border_l(px(v)),
                _ => d,
            },
        };
    }
    d
}

/// Скругление углов.
///
/// Доля считается от размера коробки: `border-radius: 50%` — это круглый
/// аватар, самая частая запись после пикселей. GPUI принимает только
/// абсолютную длину, поэтому долю разрешаем сами по заданному размеру, а без
/// него берём заведомо большое значение — растеризатор обрежет его половиной
/// меньшей стороны, что и даёт круг.
fn apply_radius(mut d: Div, c: &Computed) -> Div {
    let r = &c.radius;
    let base = match (c.width, c.height) {
        (Some(Len::Px(w)), Some(Len::Px(h))) => w.min(h),
        (Some(Len::Px(w)), _) => w,
        (_, Some(Len::Px(h))) => h,
        _ => f32::NAN,
    };
    let resolve = |l: Option<Len>| -> Option<f32> {
        match l? {
            Len::Px(v) => Some(v),
            Len::Pct(p) if base.is_nan() => Some(9999.0 * p.min(1.0)),
            Len::Pct(p) => Some(base * p),
            // Неразрешённый `em` — от базового кегля (см. `len_to_gpui`).
            Len::Em(k) => Some(k * 16.0),
            Len::EmPx(k, add) => Some(k * 16.0 + add),
            Len::Ch(k) => Some(k * crate::metrics::ch_ex_px("", 16.0).0),
            Len::Ic(k) => Some(k * crate::metrics::ic_px("", 16.0)),
            Len::Ex(k) => Some(k * crate::metrics::ch_ex_px("", 16.0).1),
            Len::Lh(k) => Some(k * 1.2 * 16.0),
            Len::LhPx(k, add) => Some(k * 1.2 * 16.0 + add),
            Len::Vw(_) | Len::Vh(_) => None,
            // Размер по содержимому числом не выражается.
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => None,
        }
    };
    for (val, corner) in [(r.tl, 0u8), (r.tr, 1), (r.br, 2), (r.bl, 3)] {
        let Some(v) = resolve(val) else { continue };
        d = match corner {
            0 => d.rounded_tl(px(v)),
            1 => d.rounded_tr(px(v)),
            2 => d.rounded_br(px(v)),
            _ => d.rounded_bl(px(v)),
        };
    }
    d
}

fn apply_paint(mut d: Div, c: &Computed) -> Div {
    // Смешивание больше не живёт на заливке: раньше блендер знал четыре
    // формулы и красил только фон узла, а CSS смешивает ВСЁ поддерево целиком.
    // Теперь оно считается при сборке буфера группы (см. `render::grouped`).
    // Фон, обрезанный внутренним краем (`background-clip`), красит не сама
    // коробка, а отдельный слой внутри неё (`render::clip_layer`): коробка в
    // раскладке красится целиком, вместе с рамкой и полями.
    if c.bg_clip.is_none() {
        if let Some(g) = &c.gradient {
            // Градиенту с размером/повтором/позицией нужна механика плитки —
            // его рисует слой-картинка (см. render::decorations), заливка
            // красила бы всю коробку.
            if !c.gradient_as_tile() {
                d = d.bg(fill(g));
            }
            if let Some(bg) = c.background {
                d = d.bg(gpui::Background::from(bg.to_hsla()));
            }
        } else if let Some(bg) = c.background {
            d = d.bg(gpui::Background::from(bg.to_hsla()));
        }
    }
    // Цвет рамки: единый — прямо в стиль. Разные цвета сторон рисуются
    // полосами в сборщике дерева: у GPUI цвет рамки один на элемент.
    // Рамка-картинка рисуется ВМЕСТО обычной рамки (css-backgrounds-3 §6):
    // толщина остаётся держать раскладку, а цвет не красится — иначе рамка
    // проступала из-под картинки (`border-image-00*`: «no red», а красная
    // рамка видна).
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    let uniform = sides.first().filter(|f| sides.iter().all(|s| s == *f));
    let border_image_on = c
        .border_image
        .as_ref()
        .is_some_and(|bi| !bi.src.is_empty());
    if !border_image_on
        && let Some(bc) = uniform.copied().copied().or(c.border_color)
    {
        d = d.border_color(bc.to_hsla());
    }
    if c.border_dashed == Some(true) {
        d = d.border_dashed();
    }
    if c.border_dotted == Some(true) {
        d.style().border_style = Some(gpui::BorderStyle::Dotted);
    }
    if let Some(o) = c.opacity {
        d = d.opacity(o);
    }
    // `visibility: hidden` — элемент занимает своё место, но не рисуется.
    if c.hidden == Some(true) {
        d.style().visibility = Some(gpui::Visibility::Hidden);
    }
    // Элемент, не ловящий курсор, не меняет и его форму.
    if let Some(name) = c
        .cursor
        .as_ref()
        .filter(|_| c.pointer_events_none != Some(true))
    {
        // Набор GPUI совпадает с CSS почти буква в букву; неизвестное имя
        // оставляем без изменений, а не подменяем стрелкой.
        let style = match name.as_str() {
            "pointer" => Some(gpui::CursorStyle::PointingHand),
            "text" | "vertical-text" => Some(gpui::CursorStyle::IBeam),
            "crosshair" => Some(gpui::CursorStyle::Crosshair),
            "grab" => Some(gpui::CursorStyle::OpenHand),
            "grabbing" | "move" | "all-scroll" => Some(gpui::CursorStyle::ClosedHand),
            "default" | "auto" => Some(gpui::CursorStyle::Arrow),
            "not-allowed" | "no-drop" => Some(gpui::CursorStyle::OperationNotAllowed),
            "context-menu" => Some(gpui::CursorStyle::ContextualMenu),
            "copy" => Some(gpui::CursorStyle::DragCopy),
            "alias" => Some(gpui::CursorStyle::DragLink),
            "ew-resize" | "col-resize" => Some(gpui::CursorStyle::ResizeLeftRight),
            "ns-resize" | "row-resize" => Some(gpui::CursorStyle::ResizeUpDown),
            "e-resize" => Some(gpui::CursorStyle::ResizeRight),
            "w-resize" => Some(gpui::CursorStyle::ResizeLeft),
            "n-resize" => Some(gpui::CursorStyle::ResizeUp),
            "s-resize" => Some(gpui::CursorStyle::ResizeDown),
            "nwse-resize" | "nw-resize" | "se-resize" => {
                Some(gpui::CursorStyle::ResizeUpLeftDownRight)
            }
            "nesw-resize" | "ne-resize" | "sw-resize" => {
                Some(gpui::CursorStyle::ResizeUpRightDownLeft)
            }
            _ => None,
        };
        if let Some(st) = style {
            d.style().mouse_cursor = Some(st);
        }
    }
        // Тень без своего цвета — цветом текста ЭТОГО элемента (метка:
    // отрицательная альфа; css-backgrounds-3 §7, currentColor).
    let shadow_colour = |sh: &crate::computed::Shadow| {
        if sh.color.a < 0.0 {
            c.color.unwrap_or(crate::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        } else {
            sh.color
        }
    };
if !c.inset_shadows.is_empty() {
        d.style().inset_box_shadow = Some(
            c.inset_shadows
                .iter()
                .map(|s| gpui::BoxShadow {
                    color: shadow_colour(s).to_hsla(),
                    offset: gpui::point(px(s.x), px(s.y)),
                    blur_radius: px(s.blur),
                    spread_radius: px(s.spread),
                })
                .collect(),
        );
    }
    if !c.shadows.is_empty() {
        d = d.shadow(
            c.shadows
                .iter()
                // Резкую тень (без размытия) рисует слой-квад в декорациях:
                // примитив с нулевым размытием вырождается в шейдере.
                .filter(|s| s.blur > 0.0)
                .map(|s| gpui::BoxShadow {
                    color: shadow_colour(s).to_hsla(),
                    offset: gpui::point(px(s.x), px(s.y)),
                    blur_radius: px(s.blur),
                    spread_radius: px(s.spread),
                })
                .collect::<Vec<_>>(),
        );
    }
    d
}

fn apply_text(mut d: Div, c: &Computed) -> Div {
    // Оформление текста нужно и на блоке: в ветке «строка из кусков» текст
    // рисуется обычными `div`, и подчёркивание, живущее только в прогонах,
    // там пропадало.
    if c.underline == Some(true) {
        d.style()
            .text
            .get_or_insert_with(Default::default)
            .underline = Some(gpui::UnderlineStyle {
            thickness: px(1.),
            color: c.color.map(|col| col.to_hsla()),
            wavy: false,
        });
    }
    if c.line_through == Some(true) {
        d.style()
            .text
            .get_or_insert_with(Default::default)
            .strikethrough = Some(gpui::StrikethroughStyle {
            thickness: px(1.),
            color: c.color.map(|col| col.to_hsla()),
        });
    }
    // Возможности шрифта: капитель, старостильные цифры, ширина начертания —
    // всё это таблицы OpenType, и GPUI умеет их включать.
    if let Some(family) = &c.font_family {
        // Имя из разметки — придуманное (`@font-face`): в набор обязано уйти
        // имя, под которым файл знает система шрифтов. Без подмены весь
        // текст, идущий гpui-раскладкой (не резчиком), набирался подменным
        // системным шрифтом.
        d = d.font_family(crate::fonts::alias(family).unwrap_or_else(|| family.clone()));
    }
    if let Some(pct) = c.font_stretch {
        d.style()
            .text
            .get_or_insert_with(Default::default)
            .font_stretch = Some(gpui::FontStretch::from_percent(pct));
    }
    if !c.font_features.is_empty() {
        d.style()
            .text
            .get_or_insert_with(Default::default)
            .font_features = Some(gpui::FontFeatures(std::sync::Arc::new(
            c.font_features.clone(),
        )));
    }
    if let Some(col) = c.color {
        d = d.text_color(col.to_hsla());
    }
    if let Some(Len::Px(size)) = c.font_size {
        d = d.text_size(px(size));
    }
    if let Some(w) = c.font_weight {
        d = d.font_weight(gpui::FontWeight(w as f32));
    }
    if c.italic == Some(true) {
        d = d.italic();
    }
    if let Some(lh) = c.line_height {
        d = match lh {
            Len::Px(v) => d.line_height(px(v)),
            Len::Pct(mult) => d.line_height(relative(mult)),
            Len::Em(k) => d.line_height(px(k * 16.0)),
            Len::EmPx(k, add) => d.line_height(px(k * 16.0 + add)),
            Len::Ch(k) => d.line_height(px(k * crate::metrics::ch_ex_px("", 16.0).0)),
            Len::Ic(k) => d.line_height(px(k * crate::metrics::ic_px("", 16.0))),
            Len::Ex(k) => d.line_height(px(k * crate::metrics::ch_ex_px("", 16.0).1)),
            Len::Lh(k) | Len::LhPx(k, _) => d.line_height(relative(k)),
            Len::Vw(k) | Len::Vh(k) => d.line_height(relative(k)),
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => d,
        };
    }
    if c.nowrap == Some(true) {
        d = d.whitespace_nowrap();
    }
    // Выравнивание текста разбиралось, но до элемента не доходило — поле
    // оставалось мёртвым, и `text-align: center` не делал ничего.
    match c.text_align {
        Some(TextAlign::Center) => d = d.text_center(),
        Some(TextAlign::Right) => d = d.text_right(),
        Some(TextAlign::Left) => d = d.text_left(),
        Some(TextAlign::Justify) => d = d.text_align(gpui::TextAlign::Justify),
        // Логические края разворачивает наследование; сюда они доходят только
        // у узлов вне него — там письмо слева направо, как в корне документа.
        Some(TextAlign::Start) => d = d.text_left(),
        Some(TextAlign::End) => d = d.text_right(),
        None => {}
    }
    if c.monospace == Some(true) {
        d = d.font_family(crate::metrics::mono_family());
    }
    if let Some(Len::Px(v)) = c.letter_spacing {
        d = d.letter_spacing(px(v));
    }
    // `text-overflow: ellipsis` действует на БЛОК-КОНТЕЙНЕРЕ с overflow,
    // отличным от visible (css-overflow-3 §text-overflow) — слитый стиль
    // растаскивал его на текстовые куски, и «…» дорисовывался даже там,
    // где текст помещался.
    if c.ellipsis == Some(true)
        && c.overflow_x
            .is_some_and(|o| o != crate::computed::Overflow::Visible)
    {
        d = d.text_ellipsis();
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computed::Computed;
    use crate::css::parse_decls;

    /// Стиль применяется к настоящему `Div` и читается обратно из `Style` —
    /// так проверяется именно маппинг, а не наше представление о нём.
    fn styled(css: &str) -> gpui::StyleRefinement {
        let mut c = Computed::default();
        c.apply_decls(&parse_decls(css));
        let mut d = apply(gpui::div(), &c);
        d.style().clone()
    }

    #[test]
    fn box_model_reaches_gpui() {
        let s = styled("padding: 4px 8px; margin-top: 6px; border: 2px solid #333");
        assert_eq!(s.padding.top, Some(px(4.).into()));
        assert_eq!(s.padding.right, Some(px(8.).into()));
        assert_eq!(s.margin.top, Some(px(6.).into()));
        assert_eq!(s.border_widths.top, Some(px(2.).into()));
        assert!(s.border_color.is_some());
    }

    #[test]
    fn flex_layout_reaches_gpui() {
        let s = styled("display: flex; flex-direction: column; align-items: center; gap: 6px");
        assert_eq!(s.display, Some(gpui::Display::Flex));
        assert_eq!(s.flex_direction, Some(gpui::FlexDirection::Column));
        assert_eq!(s.align_items, Some(gpui::AlignItems::Center));
        assert_eq!(s.gap.height, Some(px(6.).into()));
    }

    #[test]
    fn percentage_width_becomes_a_fraction() {
        let s = styled("width: 50%");
        assert_eq!(s.size.width, Some(relative(0.5).into()));
    }

    #[test]
    fn multiple_shadows_survive() {
        let s = styled("box-shadow: 0 1px 2px #000, 0 4px 12px rgba(0,0,0,.5)");
        assert_eq!(s.box_shadow.as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn gradient_takes_first_and_last_stop() {
        let s = styled("background: linear-gradient(90deg, #000000, #444444, #ffffff)");
        assert!(s.background.is_some(), "градиент доехал до фона");
    }

    #[test]
    fn unsupported_properties_leave_no_trace() {
        // Ни фильтров, ни трансформов, ни z-index в GPUI нет: стиль обязан
        // остаться пустым, а не получить приблизительную замену.
        let s = styled("filter: blur(4px); transform: rotate(45deg); z-index: 5; float: left");
        assert!(s.background.is_none() && s.opacity.is_none());
        assert!(s.size.width.is_none() && s.size.height.is_none());
    }
}
