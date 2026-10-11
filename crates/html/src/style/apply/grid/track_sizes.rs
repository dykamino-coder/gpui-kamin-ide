//! Перевод дорожек сетки (Track/TrackSize) в дорожки GPUI: пределы minmax, размер по содержимому.

use super::*;

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
pub(super) fn track(t: &TrackSize) -> gpui::GridTrack {
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

/// `justify-content`/`align-content` → распределение GPUI.
pub(in crate::style::apply) fn to_content(j: Justify) -> gpui::AlignContent {
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
