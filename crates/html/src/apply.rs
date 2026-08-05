//! Вычисленный стиль → элемент GPUI. Здесь и проходит граница охвата.
//!
//! Правило одно: если свойство выразимо примитивами GPUI — применяем; если нет
//! — не применяем НИЧЕГО вместо него. Приблизительная замена (нарисовать
//! `filter: blur` полупрозрачностью, `inset`-тень внешней) выглядит как рабочая
//! поддержка и стоит дороже честного пропуска: расхождение всплывает у
//! пользователя, а не в тесте.

use crate::computed::{
    Align, Computed, Corners, Display, FlexDir, Justify, Overflow, Position, Sides, TextAlign,
    Track,
};
use crate::value::Len;
use gpui::{Div, InteractiveElement, Styled, px, relative};

/// Ширина/высота/отступ: доля родителя или пиксели.
fn len_to_gpui(l: Len) -> gpui::DefiniteLength {
    match l {
        Len::Px(v) => px(v).into(),
        Len::Pct(v) => relative(v),
        // `auto` в размере значит «пусть решает раскладка» — это отсутствие
        // ограничения, а не значение; вызывающий такие поля не применяет.
        Len::Auto => relative(1.0),
    }
}

/// Дорожка сетки в терминах GPUI. Нижняя грань всегда `min-content`: без неё
/// колонка на узкой панели схлопывается в ноль и содержимое обрезается.
fn track(t: &Track) -> gpui::GridTrack {
    let upper = match t {
        Track::Px(v) => gpui::GridTrack::Pixels(px(*v)),
        Track::Fr(f) => gpui::GridTrack::Fraction(*f),
        Track::Auto => gpui::GridTrack::Auto,
        Track::MinContent => gpui::GridTrack::MinContent,
        Track::MaxContent => gpui::GridTrack::MaxContent,
    };
    gpui::GridTrack::MinMax(Box::new((gpui::GridTrack::MinContent, upper)))
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
        if let Some(g) = h.gradient {
            s.background = Some(gpui::Fill::Color(gpui::linear_gradient(
                g.angle_deg,
                gpui::linear_color_stop(g.from.to_hsla(), 0.0),
                gpui::linear_color_stop(g.to.to_hsla(), 1.0),
            )));
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

pub fn apply(d: Div, c: &Computed) -> Div {
    let mut d = d;
    d = apply_layout(d, c);
    d = apply_box(d, c);
    d = apply_paint(d, c);
    apply_text(d, c)
}

fn apply_layout(mut d: Div, c: &Computed) -> Div {
    match c.display {
        // Блок в GPUI — дефолт; отдельного вызова не требует.
        Some(Display::Flex) | Some(Display::InlineFlex) => d = d.flex(),
        // Инлайновая коробка в строке не растягивается по ширине родителя.
        Some(Display::InlineBlock) => d = d.flex_shrink_0(),
        Some(Display::Grid) => {
            d = d.grid();
            // Список дорожек точнее числа колонок: он несёт ширину по
            // содержимому и фиксированные колонки (патч GPUI, см. доку).
            match (&c.grid_tracks, c.grid_cols) {
                (Some(tracks), _) => d = d.grid_template_cols(tracks.iter().map(track).collect()),
                (None, Some(n)) => d = d.grid_cols(n),
                _ => {}
            }
        }
        // `display: none` отсеивается ещё при разборе дерева: узел не строится.
        _ => {}
    }
    match c.flex_dir {
        Some(FlexDir::Row) => d = d.flex_row(),
        Some(FlexDir::RowReverse) => d = d.flex_row_reverse(),
        Some(FlexDir::Col) => d = d.flex_col(),
        Some(FlexDir::ColReverse) => d = d.flex_col_reverse(),
        None => {}
    }
    if c.flex_wrap == Some(true) {
        d = d.flex_wrap();
    }
    if let Some(g) = c.flex_grow {
        // `flex_grow()` в GPUI ставит жёсткую единицу, а `flex: 2` встречается —
        // пишем значение в стиль напрямую.
        d.style().flex_grow = Some(g);
    }
    if let Some(s) = c.flex_shrink {
        d = if s == 0.0 {
            d.flex_shrink_0()
        } else {
            d.flex_shrink()
        };
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
    if let Some(b) = c.flex_basis {
        d = d.flex_basis(len_to_gpui(b));
    }
    match c.justify_content {
        Some(Justify::Center) => d = d.justify_center(),
        Some(Justify::Start) => d = d.justify_start(),
        Some(Justify::End) => d = d.justify_end(),
        Some(Justify::Between) => d = d.justify_between(),
        Some(Justify::Around) => d = d.justify_around(),
        None => {}
    }
    if let Some((row, col)) = c.gap {
        if let Some(r) = row {
            d = d.gap_y(len_to_gpui(r));
        }
        if let Some(cg) = col {
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
    let pad_x = extra(&[
        c.padding.left,
        c.padding.right,
        c.border_width.left,
        c.border_width.right,
    ]);
    let pad_y = extra(&[
        c.padding.top,
        c.padding.bottom,
        c.border_width.top,
        c.border_width.bottom,
    ]);

    for (val, f) in [
        (c.width, 0u8),
        (c.height, 1),
        (c.min_width, 2),
        (c.min_height, 3),
        (c.max_width, 4),
        (c.max_height, 5),
    ] {
        let Some(l) = val else { continue };
        if l == Len::Auto {
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
    d = apply_sides(d, &c.padding, SideKind::Padding);
    d = apply_sides(d, &c.margin, SideKind::Margin);
    d = apply_sides(d, &c.border_width, SideKind::Border);
    d = apply_radius(d, &c.radius);

    match c.position {
        Some(Position::Absolute) => {
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
        // `static` в GPUI недостижим: элемент всегда участвует в потоке
        // относительно родителя, что соответствует `relative`.
        _ => {}
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

    // Прокрутка: в GPUI скролл требует своего состояния и обработчика, поэтому
    // на уровне стиля выражается только обрезка. Прокручиваемый контейнер
    // собирается вызывающим (см. доку, раздел «Прокрутка»).
    if c.overflow_x == Some(Overflow::Hidden) || c.overflow_x == Some(Overflow::Scroll) {
        d = d.overflow_x_hidden();
    }
    if c.overflow_y == Some(Overflow::Hidden) || c.overflow_y == Some(Overflow::Scroll) {
        d = d.overflow_y_hidden();
    }
    d
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
                d.style().margin.top = None;
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

fn apply_radius(mut d: Div, r: &Corners) -> Div {
    for (val, corner) in [(r.tl, 0u8), (r.tr, 1), (r.br, 2), (r.bl, 3)] {
        let Some(Len::Px(v)) = val else { continue };
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
    if let Some(g) = c.gradient {
        // GPUI берёт ровно два стопа: первый и последний. Промежуточные
        // отброшены на разборе — см. доку, раздел «Градиенты».
        d = d.bg(gpui::linear_gradient(
            g.angle_deg,
            gpui::linear_color_stop(g.from.to_hsla(), 0.0),
            gpui::linear_color_stop(g.to.to_hsla(), 1.0),
        ));
    } else if let Some(bg) = c.background {
        d = d.bg(bg.to_hsla());
    }
    if let Some(bc) = c.border_color {
        d = d.border_color(bc.to_hsla());
    }
    if let Some(o) = c.opacity {
        d = d.opacity(o);
    }
    // `visibility: hidden` — элемент занимает своё место, но не рисуется.
    if c.hidden == Some(true) {
        d.style().visibility = Some(gpui::Visibility::Hidden);
    }
    if let Some(name) = &c.cursor {
        // Набор GPUI совпадает с CSS почти буква в букву; неизвестное имя
        // оставляем без изменений, а не подменяем стрелкой.
        let style = match name.as_str() {
            "pointer" => Some(gpui::CursorStyle::PointingHand),
            "text" => Some(gpui::CursorStyle::IBeam),
            "crosshair" => Some(gpui::CursorStyle::Crosshair),
            "grab" => Some(gpui::CursorStyle::OpenHand),
            "grabbing" => Some(gpui::CursorStyle::ClosedHand),
            "default" => Some(gpui::CursorStyle::Arrow),
            _ => None,
        };
        if let Some(st) = style {
            d.style().mouse_cursor = Some(st);
        }
    }
    if !c.shadows.is_empty() {
        d = d.shadow(
            c.shadows
                .iter()
                .map(|s| gpui::BoxShadow {
                    color: s.color.to_hsla(),
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
            Len::Auto => d,
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
        None => {}
    }
    if c.monospace == Some(true) {
        d = d.font_family("JetBrains Mono");
    }
    if let Some(Len::Px(v)) = c.letter_spacing {
        d = d.letter_spacing(px(v));
    }
    if c.ellipsis == Some(true) {
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
