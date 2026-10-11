//! Коробка: поля, отступы, рамки по сторонам, скругления (apply_box).

use crate::style::apply::*;
use crate::style::computed::{Computed, Display, Overflow, Position, Sides};
use crate::style::values::value::Len;
use gpui::{Div, Styled, px};

mod clip_position;
mod overflow_inset;
use clip_position::box_clip_position;
use overflow_inset::box_overflow;

pub(super) fn apply_box(mut d: Div, c: &Computed) -> Div {
    contained_intrinsic::apply(&mut d, c);
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
    // CSS Containment 2 §3.1: intrinsic keywords also size the box as empty.
    if c.contains_height()
        && matches!(
            c.height,
            None | Some(Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent)
        )
    {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = c.borders();
        let pad = side(c.padding.top) + side(c.padding.bottom) + side(b.top) + side(b.bottom);
        let ci = px(c
            .contain_intrinsic
            .1
            .unwrap_or_else(|| empty_contained_size(c, false))
            + pad);
        // `contain-intrinsic-size` — ВНУТРЕННИЙ размер (css-sizing-4
        // §intrinsic-size-override), а не использованный: у растянутого
        // строкой элемента ряда высоту даёт строка (css-flexbox-1 §9.4 п.11).
        // Явная высота глушила растяжку (`contain-intrinsic-size-010/016`:
        // 13 точек вместо 100); нижней гранью подмена держит строку
        // авто-высоты от схлопывания в ноль.
        d = if c.cross_stretched {
            d.min_h(ci)
        } else {
            d.h(ci)
        };
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
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
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
        d = d.w(px(c
            .contain_intrinsic
            .0
            .unwrap_or_else(|| empty_contained_size(c, true))
            + pad));
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
    box_clip_position(d, c)
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
    let extra_w =
        px_len(c.padding.left) + px_len(c.padding.right) + px_len(b.left) + px_len(b.right);
    let extra_h =
        px_len(c.padding.top) + px_len(c.padding.bottom) + px_len(b.top) + px_len(b.bottom);
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
        l => crate::text::metrics::fallback_len_px(l, "", 16.0),
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
