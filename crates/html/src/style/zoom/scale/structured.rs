//! Zoom absolute lengths nested inside compound computed values.

use crate::style::computed::{BorderImageWidth, Computed, Track, TrackSize};
use crate::style::values::value::Len;

fn track(t: &mut Track, k: f32) {
    match t {
        Track::Px(v) | Track::FitPx(v) => *v *= k,
        _ => {}
    }
}

fn tracks(t: &mut TrackSize, k: f32) {
    match t {
        TrackSize::Single(t) => track(t, k),
        TrackSize::MinMax(a, b) => {
            track(a, k);
            track(b, k);
        }
        TrackSize::AutoRepeat { tracks: ts, .. } => {
            for t in ts {
                tracks(t, k);
            }
        }
    }
}

pub(super) fn scale(c: &mut Computed, k: f32) {
    if let Some(corners) = &mut c.radius_ell {
        for (x, y) in corners.iter_mut().flatten() {
            for len in [x, y] {
                match len {
                    Len::Px(v) | Len::EmPx(_, v) | Len::LhPx(_, v) => *v *= k,
                    _ => {}
                }
            }
        }
    }
    if let Some(image) = &mut c.border_image {
        for width in image.width.iter_mut().chain(image.outset.iter_mut()) {
            if let BorderImageWidth::Px(v) = width {
                *v *= k;
            }
        }
    }
    if let Some(filter) = &mut c.filter {
        filter.blur *= k;
    }
    for list in [
        &mut c.grid_tracks,
        &mut c.grid_rows,
        &mut c.auto_repeat_body_cols,
        &mut c.auto_repeat_body_rows,
    ]
    .into_iter()
    .flatten()
    {
        for t in list {
            tracks(t, k);
        }
    }
    for t in [&mut c.grid_auto_cols, &mut c.grid_auto_rows]
        .into_iter()
        .flatten()
    {
        tracks(t, k);
    }
    for t in c
        .grid_auto_cols_list
        .iter_mut()
        .chain(c.grid_auto_rows_list.iter_mut())
    {
        tracks(t, k);
    }
    for value in [&mut c.grid_auto_fill_min, &mut c.grid_auto_fill_row]
        .into_iter()
        .flatten()
    {
        *value *= k;
    }
    for value in &mut c.grid_auto_fill_tracks {
        *value *= k;
    }
    for repeat in [&mut c.auto_repeat_cols, &mut c.auto_repeat_rows]
        .into_iter()
        .flatten()
    {
        repeat.track = repeat.track.map(|v| v * k);
        repeat.fit_px = repeat.fit_px.map(|v| v * k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_fractions_and_scales_nested_absolute_lengths() {
        let mut c = Computed::default();
        c.apply_one("grid-template-columns", "minmax(10px, 1fr) 25%");
        c.apply_one("border-image-width", "2 10px 20% auto");
        scale(&mut c, 2.0);
        assert_eq!(
            c.grid_tracks,
            Some(vec![
                TrackSize::MinMax(Track::Px(20.0), Track::Fr(1.0)),
                TrackSize::Single(Track::Pct(0.25)),
            ])
        );
        assert_eq!(
            c.border_image.unwrap().width,
            [
                BorderImageWidth::Times(2.0),
                BorderImageWidth::Px(20.0),
                BorderImageWidth::Pct(0.2),
                BorderImageWidth::Auto,
            ]
        );
    }
}
