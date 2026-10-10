//! Fraction tracks for subgrid_tracks; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

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
pub(crate) fn fr_tracks_to_px(
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
