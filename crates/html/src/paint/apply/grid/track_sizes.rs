//! Перевод дорожек сетки (Track/TrackSize) в дорожки GPUI: пределы minmax, размер по содержимому.

use super::*;

/// Дорожка сетки в терминах GPUI. Нижняя грань всегда `min-content`: без неё
/// колонка на узкой панели схлопывается в ноль и содержимое обрезается.
/// Одна грань дорожки.
fn bound(t: &Track, c: &Computed, vcx: crate::values::ValueCx<'_>) -> gpui::GridTrack {
    match t {
        Track::Px(v) => gpui::GridTrack::Pixels(px(*v)),
        Track::Auto => gpui::GridTrack::Auto,
        Track::MinContent => gpui::GridTrack::MinContent,
        Track::MaxContent => gpui::GridTrack::MaxContent,
        Track::Fr(f) => gpui::GridTrack::Fraction(*f),
        Track::Pct(p) => gpui::GridTrack::Percent(*p),
        Track::Font(l) | Track::FitFont(l) => {
            let cx = crate::style::computed::LenCx {
                font: Some(c.font_cx(vcx)),
                viewport: Some(c.viewport_cx(vcx.viewport, vcx)),
                ..Default::default()
            };
            // Percentages refer to the grid axis, rather than the viewport.
            let terms = if matches!(l, Len::Calc(_) | Len::Math(_)) {
                cx.math_terms(*l, vcx)
            } else {
                cx.px(*l, vcx).map(|v| (0.0, v))
            };
            let Some((fraction, add)) = terms else {
                return gpui::GridTrack::Auto;
            };
            if matches!(t, Track::FitFont(_)) {
                gpui::GridTrack::FitContentCalc {
                    fraction,
                    add: px(add),
                }
            } else {
                gpui::GridTrack::Calc {
                    fraction,
                    add: px(add),
                }
            }
        }
        Track::FitPx(v) => gpui::GridTrack::FitContentPx(px(*v)),
        Track::FitPct(p) => gpui::GridTrack::FitContentPercent(*p),
    }
}

/// Дорожка сетки: одиночная либо пара граней.
///
/// Одиночная переносится как есть — оборачивать её в `minmax` нельзя, иначе
/// нижняя грань разрешает колонке вырасти сверх заданного (поймано сравнением
/// с Chrome: колонка 120px выходила 200). Исключение — доля свободного места:
/// `1fr` в CSS и есть `minmax(auto, 1fr)`, иначе она схлопывается под
/// содержимым.
pub(super) fn track(
    t: &TrackSize,
    c: &Computed,
    vcx: crate::values::ValueCx<'_>,
) -> gpui::GridTrack {
    match t {
        TrackSize::MinMax(lo, hi) => {
            gpui::GridTrack::MinMax(Box::new((bound(lo, c, vcx), bound(hi, c, vcx))))
        }
        TrackSize::Single(Track::Fr(f)) => gpui::GridTrack::MinMax(Box::new((
            gpui::GridTrack::Auto,
            gpui::GridTrack::Fraction(*f),
        ))),
        TrackSize::Single(one) => bound(one, c, vcx),
        TrackSize::AutoRepeat { fit, tracks } => gpui::GridTrack::AutoRepeat {
            fit: *fit,
            tracks: tracks.iter().map(|t| track(t, c, vcx)).collect(),
        },
    }
}

/// `justify-content`/`align-content` → распределение GPUI.
pub(in crate::paint::apply) fn to_content(j: Justify) -> gpui::AlignContent {
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
