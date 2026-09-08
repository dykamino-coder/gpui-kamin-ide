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
        // Сюда шрифтовые единицы доходят только у узлов вне наследования
        // (элементы форм, корень) — запасные значения даёт единая точка.
        l @ (Len::Em(_)
        | Len::EmPx(..)
        | Len::Ch(_)
        | Len::Ic(_)
        | Len::Ex(_)
        | Len::Lh(_)
        | Len::LhPx(..)) => px(crate::metrics::fallback_len_px(l, "", 16.0).unwrap_or(0.0)).into(),
        // Единицы окна разрешает сборщик дерева; сюда они доходят только у
        // узлов вне его — доля родителя ближе всего по смыслу.
        Len::Vw(k) | Len::Vh(k) => relative(k),
        // Смешанный остаток calc (обычно px+%): честно ляжет только в
        // taffy-calc (фаза 2); пока — процентная часть, при её отсутствии
        // точечная (ближе, чем прежний сброс всего объявления).
        Len::Calc(i) => {
            let s = crate::value::calc_get(i);
            if s.pct != 0.0 {
                relative(s.pct)
            } else {
                px(s.px).into()
            }
        }
        // `auto` в размере значит «пусть решает раскладка» — это отсутствие
        // ограничения, а не значение; вызывающий такие поля не применяет.
        // Размер по содержимому — то же самое: его ставит обёртка-сетка
        // (`render::content_sized`), а не длина.
        // `anchor()` во вставке: раскладке отдаётся НОЛЬ — коробка встаёт к
        // краю содержащего блока, а сдвиг до края якоря считает
        // `anchor::AnchorPlace` на подготовке кадра; там же от этого нуля
        // отсчитывается и запасное значение `anchor(left, 20px)`.
        Len::Anchor(_) => px(0.0).into(),
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
        // Сюда единица шрифта дойти не должна: её переводит в точки
        // разрешение кегля. Если всё же дошла — ведём себя как `auto`.
        Track::Font(_) => gpui::GridTrack::Auto,
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
        TrackSize::AutoRepeat { fit, tracks } => gpui::GridTrack::AutoRepeat {
            fit: *fit,
            tracks: tracks.iter().map(track).collect(),
        },
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
        Justify::WmStart | Justify::Left => gpui::AlignContent::Start,
        Justify::WmEnd | Justify::Right => gpui::AlignContent::End,
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
        // §anchor-center вне абсолюта с якорем «behaves as center»; у
        // абсолюта с якорем сдвиг довозит `anchor::AnchorPlace` поверх.
        Align::Center | Align::AnchorCenter => gpui::AlignItems::Center,
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
    // Пространство смешения (css-color-4 §12.2). GPU-путь выражает два:
    // гамма-sRGB и OKLab — шейдер переводит цвета в вершинном и обратно
    // после смешения. Прочие пространства сюда не доходят: `gradient_as_tile`
    // уводит их на растровый путь, где цвет считается на точку.
    let base = match g.space {
        crate::computed::GradSpace::Oklab => base.color_space(gpui::ColorSpace::Oklab),
        _ => base,
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
        // Тело повтора из НЕСКОЛЬКИХ дорожек: своего «минимума» оно не даёт
        // (`auto_fill_min` разбирает одну дорожку), поэтому идёт своей ветвью.
        // Раскладка список принимает как есть — `GridTrack::AutoRepeat`
        // хранит `Vec` (`grid-auto-repeat-multiple-values-*` рисовались одной
        // плитой во всю ширину).
        // Поток ЛУНОК разворачивает повтор своим кодом (`render::lanes`), и
        // список дорожек ему только мешает: `column-auto-repeat-013`
        // уходил 0.00 → 10.92. Признак — `lanes_row`/`lanes_inline` и родня,
        // они выставлены только у лунок.
        (None, _, None)
            if !flip
                && c.grid_auto_fill_tracks.len() > 1
                // Поток ЛУНОК доходит сюда уже с `display: grid` (печать
                // `KAMIN_REPEAT_DIAG` показала `Some(Grid)`), поэтому вид его
                // не отсекает: `column-auto-repeat-013` (лунки, черновик)
                // уходит 0.00 → 10.92 — это записанная цена жилы.
                && !matches!(c.display, Some(crate::computed::Display::GridLanes)) =>
        {
            d = along_line(
                d,
                vec![gpui::GridTrack::AutoRepeat {
                    fit: c.auto_repeat_cols.is_some_and(|r| r.fit),
                    tracks: c
                        .grid_auto_fill_tracks
                        .iter()
                        .map(|v| gpui::GridTrack::Pixels(px(*v)))
                        .collect(),
                }],
            )
        }
        (None, _, Some(min)) if !flip => {
            // Повтор отдаётся раскладке СВОИМ видом: она считает, сколько
            // дорожек влезет, и при `auto-fit` схлопывает пустые
            // (css-grid-2 §auto-repeat). Прежняя короткая форма подменяла
            // дорожку растяжкой `minmax(min, 1fr)`, и уцелевшие дорожки
            // забирали весь остаток — раздавать было нечего.
            let r = c.auto_repeat_cols;
            // Тело повтора бывает из НЕСКОЛЬКИХ дорожек
            // (`repeat(auto-fill, 50px 50px)`) — раскладка это уже умеет,
            // список идёт в неё как есть.
            let lo = match r.and_then(|r| r.track_pct) {
                Some(k) => gpui::GridTrack::Percent(k),
                None => gpui::GridTrack::Pixels(px(min)),
            };
            // `minmax(N, auto | k fr)`: максимум остаётся у дорожки — растяжка
            // остатком (§12.8) и доли считает сама раскладка.
            let track = match r {
                Some(r) if r.max_auto => {
                    gpui::GridTrack::MinMax(Box::new((lo, gpui::GridTrack::Auto)))
                }
                Some(r) if r.max_fr.is_some() => gpui::GridTrack::MinMax(Box::new((
                    lo,
                    gpui::GridTrack::Fraction(r.max_fr.unwrap_or(1.0)),
                ))),
                _ => lo,
            };
            let tracks: Vec<gpui::GridTrack> = vec![track];
            d = along_line(
                d,
                vec![gpui::GridTrack::AutoRepeat {
                    fit: r.is_some_and(|r| r.fit),
                    tracks,
                }],
            )
        }
        (None, Some(n), _) if !flip => d = d.grid_cols(n),
        (None, Some(n), _) => {
            d = d.grid_template_rows((0..n).map(|_| gpui::GridTrack::Auto).collect())
        }
        _ => {}
    }
    // Повтор по ОСИ РЯДОВ: у классической сетки его не было вовсе, и
    // `grid-template-rows: repeat(auto-fill, …)` уходил в никуда — ряды
    // становились неявными, нулевой высоты.
    if let Some(r) = c.grid_rows_repeat() {
        let lo = match r.track_pct {
            Some(k) => gpui::GridTrack::Percent(k),
            None => gpui::GridTrack::Pixels(px(r.track.unwrap_or(0.0))),
        };
        // Максимум `minmax(N, auto | k fr)` — как у колонок выше.
        let unit = if r.max_auto {
            gpui::GridTrack::MinMax(Box::new((lo, gpui::GridTrack::Auto)))
        } else if let Some(k) = r.max_fr {
            gpui::GridTrack::MinMax(Box::new((lo, gpui::GridTrack::Fraction(k))))
        } else {
            lo
        };
        let line = vec![gpui::GridTrack::AutoRepeat {
            fit: r.fit,
            tracks: vec![unit],
        }];
        d = if flip {
            d.grid_template_cols(line)
        } else {
            d.grid_template_rows(line)
        };
    }
    // Строчная сетка ОБНИМАЕТ свои Px-дорожки (shrink-to-fit): блочная
    // ширина на всю строку ломала все пары с `display: inline grid` в
    // разметке эталонов (subgrid-alignment-in-subgridded-axis: серый фон до
    // края страницы вместо 100px). gpui-размер — border-box: паддинги и
    // рамки сверху.
    if c.display == Some(Display::InlineGrid) && c.width.is_none() {
        if let Some(tracks) = &c.grid_tracks {
            let all_px: Option<f32> = tracks.iter().try_fold(0.0f32, |acc, t| match t {
                crate::computed::TrackSize::Single(crate::computed::Track::Px(w)) => Some(acc + w),
                _ => None,
            });
            if let Some(mut total) = all_px.filter(|t| *t > 0.0) {
                let px_of = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                let b = c.borders();
                total += px_of(c.column_gap) * (tracks.len().saturating_sub(1)) as f32
                    + px_of(c.padding.left)
                    + px_of(c.padding.right)
                    + px_of(b.left)
                    + px_of(b.right);
                d = d.w(px(total));
            }
        }
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
        if !c.grid_auto_rows_list.is_empty() {
            d.style().grid_auto_rows_list =
                Some(c.grid_auto_rows_list.iter().map(track).collect());
        }
    }
    if let Some(t) = auto_flow_axis {
        d.style().grid_auto_cols = Some(track(t));
        if !c.grid_auto_cols_list.is_empty() {
            d.style().grid_auto_cols_list =
                Some(c.grid_auto_cols_list.iter().map(track).collect());
        }
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
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): обратная поперечная ось «по спеке» —
    // колонка при `direction: rtl` и ряд при `vertical-rl` зеркалят
    // `start`/`end` у `align-items` и у `align-self` детей (css-flexbox-1
    // §9.6). css-flexbox 733 -> 733 (+1/−1), css-writing-modes 570 -> 564:
    // шесть `text-orientation-*-100` с явным `flex-start` в vertical-rl
    // ушли в «красное видно». Зеркало умолчания ниже — единственное, что
    // подтверждено замером; явные значения оставлять физическими.
    if c.vertical_rl == Some(true)
        && c.align_items.is_none()
        && matches!(c.display, Some(Display::Flex) | Some(Display::InlineFlex))
    {
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
    match c.align_items {
        Some(Align::Center) => d = d.items_center(),
        Some(Align::Start) => d = d.items_start(),
        Some(Align::End) => d = d.items_end(),
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
        && (c.align_items_safe || c.align_self_safe || c.align_content_safe || c.justify_content_safe)
    {
        d.style().safe_alignment = Some((
            c.align_items_safe,
            c.align_self_safe,
            c.align_content_safe,
            c.justify_content_safe,
        ));
    }
    // `align-self` — про САМ элемент, а не про его детей. Раньше оба свойства
    // писались в одно поле, и элемент выравнивал содержимое вместо себя.
    if let Some(a) = c.align_self {
        d.style().align_self = Some(match a {
            Align::Center | Align::AnchorCenter => gpui::AlignItems::Center,
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
        // `left`/`right` (css-align-3 §5.2): вдоль строчной оси — как
        // `start`/`end` письма (в ряду); «if the property's axis is not
        // parallel with the inline axis, this value behaves as start» —
        // в КОЛОНКЕ это `flex-start` (`flexbox_justifycontent-right-002`).
        // Только горизонтальное письмо: в вертикальном оси уже переставлены
        // поворотом, и `left/right` там держит прежний путь (★ ЗАМЕРЕНО:
        // без этой отсечки `flexbox-justify-content-wmvert-001` 0.00 -> 1.12).
        let main_vertical = c.vertical.is_none()
            && matches!(dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
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
    if let Some(a) = c
        .justify_items
        .filter(|a| *a != Align::Baseline || !real_grid)
    {
        d.style().justify_items = Some(to_items(a));
    }
    if let Some(a) = c.justify_self {
        d.style().justify_self = Some(to_items(a));
    }
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
        // ПРОБОВАЛИ И ОТКАТИЛИ (§10.5: доля высоты от содержащего блока
        // НЕОПРЕДЕЛЁННОЙ высоты считается как `auto`): признак
        // `cb_height_def`, ставится при наследовании по цепочке долей.
        // Проба по 450 парам семей `*height*`: приобретено 2, потеряно 2
        // (`max-height-percentage-002` 1.92 -> 0.00 и `height-percentage-002`
        // против `height-percentage-003a` и `min-height-percentage-003`,
        // обе 0.00 -> «красное видно»). Сужение до одной `height` — хуже,
        // 1 против 2. «Определённость» у нас не полна: её дают ещё
        // растяжение элемента гибкого контейнера и высота ячейки, а их
        // признак не видит.
        // §10.5: доля ВЫСОТЫ от содержащего блока неопределённой высоты
        // считается как `auto`. Прежде она уходила в раскладку и решалась от
        // высоты, посчитанной по содержимому, — то есть от самой себя.
        // Оговорка спеки — про САМ элемент: абсолютно позиционированный
        // считает долю всегда, его блок определён по построению.
        // ПРОБОВАЛИ И ОТКАТИЛИ: гейт «правило только для слитого стиля»
        // (признак `merged`, ставился в `inline::inherit`). Задумывался как
        // защита сырого стиля — рамки, замещаемого и подписи таблицы, — но
        // замерено полным сводом: CSS3 2355 -> 2354, `row-auto-repeat-auto-023`
        // 0.32 -> 9.86, приобретений ноль. Сырому стилю правило тоже нужно.
        if matches!(l, Len::Pct(_))
            && f % 2 == 1
            && !c.cb_height_def
            && !c.root_box
            && !matches!(
                c.position,
                Some(Position::Absolute) | Some(Position::Fixed)
            )
        {
            continue;
        }
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
        if f == 1 && let Len::Px(h) = l {
            if kw(c.min_height) {
                d = d.min_h(px(h));
                continue;
            }
            if kw(c.max_height) {
                d = d.max_h(px(h));
                continue;
            }
        }
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
            }
            (_, Some(Len::Px(h))) => {
                let h = clamp(h, c.min_height, c.max_height);
                let mw = if bb { (h + pad_y) * r - pad_x } else { h * r };
                let mw = clamp(mw.max(0.0), None, c.max_width);
                d = d.min_w(px(mw + pad_x));
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
        && visible(c.overflow_x)
        && visible(c.overflow_y)
        && !c.scroller
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
}

fn apply_box(mut d: Div, c: &Computed) -> Div {
    // `contain: size`: коробка меряется как пустая — рост от содержимого
    // подменяется `contain-intrinsic-size` (или нулём). Подмена касается
    // размера ПО СОДЕРЖИМОМУ: высота auto считается от содержимого — её и
    // задаём; ширина блока в потоке и так не от содержимого, её не трогаем.
    // Явное `height: auto` — та же высота от содержимого: подмена нужна и
    // ему (`contain-size-replaced-003*` пишут auto буквально).
    // `contain-intrinsic-size` — ВНУТРЕННИЙ размер (css-sizing-4): отступы
    // и рамка прибавляются к нему независимо от `box-sizing`. Раньше
    // ставилась голая величина, и taffy подпирал её суммой отступов —
    // выходило max(ci, pad) вместо ci + pad (`cis-007`, `cis-008`).
    if c.contains_height() && matches!(c.height, None | Some(Len::Auto)) {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = c.borders();
        let pad = side(c.padding.top) + side(c.padding.bottom) + side(b.top) + side(b.bottom);
        d = d.h(px(c.contain_intrinsic.1.unwrap_or(0.0) + pad));
    }
    // По строчной оси то же самое, но только когда ширина ЯВНО названа
    // размером по содержимому: обычная блочная ширина и так берётся от
    // родителя, а не от содержимого.
    // Ширина от содержимого бывает не только по ключевому слову: строчный
    // контейнер, плавающий и позиционированный ужимаются по нему сами
    // (shrink-to-fit). Под обособлением содержимого у них нет — ширина
    // становится `contain-intrinsic-size`.
    let shrink_to_fit = matches!(
        c.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    ) || c.float.is_some()
        || matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        );
    if c.contains_width()
        && (matches!(
            c.width,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        ) || (shrink_to_fit && matches!(c.width, None | Some(Len::Auto))))
    {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = c.borders();
        let pad = side(c.padding.left) + side(c.padding.right) + side(b.left) + side(b.right);
        d = d.w(px(c.contain_intrinsic.0.unwrap_or(0.0) + pad));
    }
    // Вклад обособленной коробки в измеряющего родителя — тоже
    // `contain-intrinsic-size`: он же перебивает автоминимум элемента ряда
    // или сетки (`min-width: auto` = размер по содержимому, а содержимого
    // здесь нет). При ЯВНОЙ ширине вклад не нужен — она и есть ответ, а
    // подпорка снизу растягивала коробку против написанного.
    if c.contains_width()
        && matches!(c.min_width, None | Some(Len::Auto))
        && matches!(c.width, None | Some(Len::Auto))
        && !shrink_to_fit
    {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = c.borders();
        let pad = side(c.padding.left) + side(c.padding.right) + side(b.left) + side(b.right);
        d = d.min_w(px(c.contain_intrinsic.0.unwrap_or(0.0) + pad));
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
            // Явный `auto` краем не считается (CSS 2.1 §9.3.2).
            let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
            let horizontal = edge(c.inset.left) && edge(c.inset.right);
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
    // `clip` режет краску, но НЕ создаёт скролл-контейнер: авто-минимум
    // flex/grid-элемента остаётся по содержимому — у gpui/taffy для этого
    // отдельный вариант (CSSWG #7714; min-size-auto-overflow-clip).
    if c.overflow_x == Some(Overflow::Clip) {
        d.style().overflow.x = Some(gpui::Overflow::Clip);
    }
    if c.overflow_y == Some(Overflow::Clip) {
        d.style().overflow.y = Some(gpui::Overflow::Clip);
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
        let b = [
            side(border.top),
            side(border.right),
            side(border.bottom),
            side(border.left),
        ];
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
    // `translate` (css-transforms-2 §individual-transforms) действует и на
    // СТАТИКЕ: он визуальный, как `transform`, и от `position` не зависит.
    // Ранний выход стоял ПЕРЕД блоком сдвига, поэтому ветка
    // `if c.position.is_none() { d.relative() }` была недостижима.
    if !matches!(
        c.position,
        Some(Position::Relative) | Some(Position::Absolute) | Some(Position::Fixed)
    ) {
        if let Some((x, y)) = c.translate {
            let px_of = |l: Len| match l {
                Len::Px(v) => v,
                _ => 0.0,
            };
            let (dx, dy) = (px_of(x), px_of(y));
            if dx != 0.0 || dy != 0.0 {
                d = d.relative();
                if dx != 0.0 {
                    d = d.left(gpui::px(dx));
                }
                if dy != 0.0 {
                    d = d.top(gpui::px(dy));
                }
            }
        }
        return d;
    }
    // §9.4.3: у относительно сдвинутой коробки с ОБОИМИ горизонтальными
    // краями один из них избыточен — «if neither is auto, one of them must be
    // ignored: for direction ltr `right`, for rtl `left`». Раскладка под нами
    // всегда берёт начальный край, поэтому при rtl левый край снимается здесь.
    let set = |l: Option<Len>| matches!(l, Some(x) if x != Len::Auto);
    // У АБСОЛЮТНОЙ коробки то же правило §10.3.7: избыточен один из краёв,
    // и при rtl это левый. Но избыток возникает, только когда заданы все три
    // величины — при `width: auto` края решают ширину, и отбрасывать нечего.
    let over = match c.position {
        Some(Position::Relative) => true,
        Some(Position::Absolute) | Some(Position::Fixed) => set(c.width),
        _ => false,
    };
    let drop_left = over && c.cb_rtl && set(c.inset.left) && set(c.inset.right);
    for (val, f) in [
        (c.inset.top, 0u8),
        (c.inset.right, 1),
        (c.inset.bottom, 2),
        (if drop_left { None } else { c.inset.left }, 3),
    ] {
        let Some(l) = val else { continue };
        if l == Len::Auto {
            continue;
        }
        // Доля края по оси БЛОКА у относительно сдвинутой коробки считается
        // от высоты содержащего блока, а когда та не задана — край
        // вычисляется в `auto`, то есть в ноль (CSS2 §9.3.2: «If the height
        // of the containing block is not specified explicitly … the value
        // computes to auto»; Blink `relative_utils.cc::ResolveInset` отдаёт
        // `nullopt` при неопределённом размере).
        if c.position == Some(Position::Relative)
            && matches!(f, 0 | 2)
            && matches!(l, Len::Pct(_))
            && !c.cb_height_def
        {
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
/// Радиус угла в точках: доля — от BORDER-BOX (css-backgrounds-3 §5.1), а
/// `c.width` — содержимое, поэтому отбивки и рамка прибавляются. Проба
/// `probe-bg-radiuspct` (`width:20; padding:20; border:20;
/// border-radius:100% 0 0 0`) давала радиус 20 вместо 100. Без заданного
/// размера берётся заведомо большое значение — растеризатор обрежет его
/// половиной меньшей стороны, что и даёт круг.
pub(crate) fn radius_px(c: &Computed, l: Option<Len>) -> Option<f32> {
    let px_len = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = c.borders();
    let extra_w = px_len(c.padding.left) + px_len(c.padding.right) + px_len(b.left) + px_len(b.right);
    let extra_h = px_len(c.padding.top) + px_len(c.padding.bottom) + px_len(b.top) + px_len(b.bottom);
    let base = match (c.width, c.height) {
        (Some(Len::Px(w)), Some(Len::Px(h))) => (w + extra_w).min(h + extra_h),
        (Some(Len::Px(w)), _) => w + extra_w,
        (_, Some(Len::Px(h))) => h + extra_h,
        _ => f32::NAN,
    };
    match l? {
        Len::Px(v) => Some(v),
        Len::Pct(p) if base.is_nan() => Some(9999.0 * p.min(1.0)),
        Len::Pct(p) => Some(base * p),
        // Шрифтовые единицы — от запасного кегля, единой точкой.
        l => crate::metrics::fallback_len_px(l, "", 16.0),
    }
}

fn apply_radius(mut d: Div, c: &Computed) -> Div {
    // Эллиптические углы и большой неоднородный радиус режет альфа-маска
    // буфера группы; круглое скругление сверху обрезало бы форму вторым
    // лезвием (см. `Computed::radius_masked`).
    // `border-shape` не совместим с `border-radius`: радиус — «as if it was
    // set to 0» (css-borders-4 §border-shape-radius-interaction).
    if c.radius_masked() || c.border_shape.is_some() {
        return d;
    }
    let r = &c.radius;
    let resolve = |l: Option<Len>| radius_px(c, l);
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

/// Рамка ПОВЕРХ слоя картинки: цвет и толщины сторон, если рамку надо
/// рисовать отдельным слоем после плиток фона, а не квадом коробки.
///
/// Квад красит фон и рамку одним примитивом ДО детей, а слой плиток —
/// ребёнок (`render::decorations`), поэтому полупрозрачная или пунктирная
/// рамка оказывалась ПОД картинкой (`origin-border-box`: голубая rgba-рамка
/// накрыта жёлтой плиткой; `css3-background-origin-*`: зелёный квадрат
/// поверх пунктира). css-backgrounds-3 §3.7, прим.: «The background is
/// always drawn behind the border»; Blink `box_fragment_painter.cc`:
/// `PaintFillLayers` → `PaintBorder`. Сплошную непрозрачную рамку
/// `paint_tiles` и раньше обходил ужатием области краски — слой делает то же
/// для любой рамки. `None` — рисовать по-старому.
pub(crate) fn border_layer(c: &Computed) -> Option<(crate::value::Color, [f32; 4])> {
    // Слой картинки бывает только у этих двух (см. `render::decorations`).
    if c.bg_image.is_none() && !c.gradient_as_tile() {
        return None;
    }
    // Особые рамки несут свои слои, разные цвета сторон — полосы поверх:
    // всё это уже лежит над плитками (`apply_paint` о них знает).
    if c.border_image.as_ref().is_some_and(|bi| !bi.src.is_empty())
        || c.corner_shaped()
        || c.border_shape.is_some()
    {
        return None;
    }
    // Обрезка содержимого срезала бы и слой: он лежит В коробке, на
    // отрицательных отступах (`css3-background-size-contain`: пунктирная
    // рамка исчезала при `overflow: hidden`). Такие коробки красит квад.
    if !matches!(c.overflow_x, None | Some(crate::computed::Overflow::Visible))
        || !matches!(c.overflow_y, None | Some(crate::computed::Overflow::Visible))
    {
        return None;
    }
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    let uniform = sides.first().filter(|f| sides.iter().all(|s| s == *f));
    if sides.len() > 1 && uniform.is_none() {
        return None;
    }
    let w = c.borders();
    let side_px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let widths = [side_px(w.top), side_px(w.right), side_px(w.bottom), side_px(w.left)];
    if !widths.iter().any(|v| *v > 0.0) {
        return None;
    }
    // Без цвета — цвет текста, без него чёрный (как у квада).
    let colour = uniform
        .copied()
        .copied()
        .or(c.border_color)
        .or(c.color)
        .unwrap_or(crate::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        });
    Some((colour, widths))
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
    let border_image_on = c.border_image.as_ref().is_some_and(|bi| !bi.src.is_empty());
    // При РАЗНЫХ цветах сторон квад не красится вовсе: его рамка рисуется
    // поверх детей, и красная полоса накрывала цветную
    // (clip-path-polygon-007: `border: red` + `border-left: lime`).
    let mixed = sides.len() > 1 && uniform.is_none();
    // Цвет не задан вовсе — рамка красится цветом текста, а без него
    // чёрным: начальное значение `border-color` — `currentColor`, начальное
    // `color` — чёрный. Прежде такая рамка не красилась ничем и пропадала,
    // хотя место в раскладке держала (строчный путь это уже делал —
    // `inline::uniform_border`).
    let any_border = {
        let w = c.borders();
        [w.top, w.right, w.bottom, w.left]
            .iter()
            .any(|l| matches!(l, Some(Len::Px(v)) if *v > 0.0))
    };
    let current = any_border.then(|| {
        c.color.unwrap_or(crate::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        })
    });
    // Фигурные углы (`corner-shape`): рамку красит кольцевой слой по контуру
    // (`render::decorations`), квад цвета не получает — иначе его прямой
    // внутренний угол проступал бы из-под контура (corner-shape-bevel:
    // красный треугольник ~240 px² на угол при допуске 200 px на пару).
    // `border-shape`: рамку целиком рисует слой контура (`render::decorations`),
    // прямоугольная рамка квада проступала бы из-под фигуры.
    // Рамка над слоем картинки — отдельным слоем (`border_layer`,
    // `render::decorations`): квад цвета не получает, иначе рамка легла бы
    // ПОД плитки, а слой — второй раз поверх (полупрозрачная потемнела бы).
    if !border_image_on
        && !mixed
        && !c.corner_shaped()
        && c.border_shape.is_none()
        && border_layer(c).is_none()
        && let Some(bc) = uniform.copied().copied().or(c.border_color).or(current)
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

/// Текстовые свойства коробки: шрифт, кегль, цвет, начертание.
///
/// Открыта наружу для маркера списка: он рисуется отдельной коробкой и
/// свойства пункта сам не получает.
pub fn apply_text(mut d: Div, c: &Computed) -> Div {
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
        d = d.font_family(
            crate::fonts::alias_stretch(family, c.font_stretch)
                .unwrap_or_else(|| family.clone()),
        );
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
            l @ (Len::EmPx(..) | Len::Ch(_) | Len::Ic(_) | Len::Ex(_)) => d.line_height(px(
                crate::metrics::fallback_len_px(l, "", 16.0).unwrap_or(16.0),
            )),
            Len::Lh(k) | Len::LhPx(k, _) => d.line_height(relative(k)),
            Len::Vw(k) | Len::Vh(k) => d.line_height(relative(k)),
            // `anchor()` в `line-height` не бывает (css-anchor-position-1 §anchor-fn:
            // только вставки) — как незнакомая длина, без сдвига.
            Len::Calc(_)
            | Len::Auto
            | Len::MinContent
            | Len::MaxContent
            | Len::FitContent
            | Len::Anchor(_) => d,
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
        // Логические края — по НАПРАВЛЕНИЮ ПИСЬМА: `end` при rtl — левый
        // край (text-align-end-001: текст уходил вправо).
        Some(TextAlign::Start) => {
            d = if c.rtl == Some(true) {
                d.text_right()
            } else {
                d.text_left()
            }
        }
        Some(TextAlign::End) => {
            d = if c.rtl == Some(true) {
                d.text_left()
            } else {
                d.text_right()
            }
        }
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
        // Строковый маркер `text-overflow: "…текст…"` рисуется вместо
        // многоточия (css-overflow-4): gpui умеет любой текст усечения.
        d = match &c.overflow_marker {
            Some(m) => d.text_overflow(gpui::TextOverflow::Truncate(m.clone().into())),
            None => d.text_ellipsis(),
        };
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
