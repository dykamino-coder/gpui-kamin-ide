//! Resolved border contours and their ordered luminance coverage paths.

use super::border_geometry::{border_commands, miter_limit};
use crate::svg::path_commands::Geometry;
use crate::svg::path_scene::*;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct Contour {
    data: Option<Arc<gpui::path_scene::tiny_skia_path::Path>>,
    rule: FillRule,
    pub miter: f32,
    x: f32,
    y: f32,
}

impl Contour {
    pub fn place(
        (raw, [t, r, b, l]): (&str, [f32; 4]),
        bw: f32,
        bh: f32,
        dx: f32,
        dy: f32,
    ) -> Option<Self> {
        let mut geometry = Geometry::default();
        let rule = border_commands(
            raw,
            (bw + l + r).max(0.0),
            (bh + t + b).max(0.0),
            &mut geometry,
        )?;
        Some(Self {
            data: geometry.finish(),
            rule: if rule == "evenodd" {
                FillRule::EvenOdd
            } else {
                FillRule::NonZero
            },
            miter: miter_limit(raw),
            x: dx - l,
            y: dy - t,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }

    pub fn path(&self, fill: Option<PathPaint>, stroke: Option<(PathPaint, f32)>) -> PathGroup {
        let children = self
            .data
            .iter()
            .map(|data| {
                shape(
                    data.clone(),
                    self.rule,
                    fill,
                    stroke.map(|(p, w)| (p, w, self.miter)),
                )
            })
            .collect();
        translated(group(children), self.x, self.y)
    }

    /// Expand a filled coverage shape, or contract it with an opposite-color
    /// stroke painted separately so antialiased overlap matches SVG masks.
    pub fn offset(&self, white: bool, distance: f32) -> PathGroup {
        if distance >= 0.0 {
            self.path(
                Some(gray(white)),
                (distance > 0.0).then(|| (gray(white), distance * 2.0)),
            )
        } else {
            group(vec![
                element(self.path(Some(gray(white)), None)),
                element(self.path(None, Some((gray(!white), -distance * 2.0)))),
            ])
        }
    }
}

#[cfg(test)]
mod tests;
