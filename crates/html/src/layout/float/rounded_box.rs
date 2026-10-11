//! Continuous rounded-box exclusions for CSS Shapes line bands.

#[derive(Clone, Debug, PartialEq)]
pub struct RoundedBox {
    rect: [f32; 4],
    radii: [(f32, f32); 4],
    canvas: (f32, f32),
    side: i32,
}

impl RoundedBox {
    pub(crate) fn new(
        rect: (f32, f32, f32, f32),
        mut radii: [(f32, f32); 4],
        canvas: (f32, f32),
        side: i32,
    ) -> Self {
        for r in &mut radii {
            if r.0 <= 0.0 || r.1 <= 0.0 {
                *r = (0.0, 0.0);
            }
        }
        // CSS Backgrounds 3 §Overlapping Curves reduces all radii by the same factor.
        let mut scale = 1.0f32;
        for (sum, edge) in [
            (radii[0].0 + radii[1].0, rect.2),
            (radii[3].0 + radii[2].0, rect.2),
            (radii[0].1 + radii[3].1, rect.3),
            (radii[1].1 + radii[2].1, rect.3),
        ] {
            if sum > edge && sum > 0.0 {
                scale = scale.min(edge.max(0.0) / sum);
            }
        }
        for r in &mut radii {
            *r = (r.0 * scale, r.1 * scale);
        }
        Self {
            rect: [rect.0, rect.1, rect.2, rect.3],
            radii,
            canvas,
            side,
        }
    }

    pub(crate) fn bottom(&self) -> f32 {
        self.canvas.1
    }

    pub(crate) fn cut(&self, y0: f32, y1: f32) -> f32 {
        let [x, y, w, h] = self.rect;
        if w <= 0.0 || h <= 0.0 || y1 <= y.max(0.0) || y0 >= (y + h).min(self.canvas.1) {
            return 0.0;
        }
        let (lo, hi) = (y0.max(y).max(0.0), y1.min(y + h).min(self.canvas.1));
        let (top, bottom, full) = if self.side < 0 {
            (self.radii[1], self.radii[2], x + w)
        } else {
            (self.radii[0], self.radii[3], self.canvas.0 - x)
        };
        // CSS Shapes 1 §Shapes from Box Values uses the continuous box boundary. Blink's
        // BoxShape::GetExcludedInterval (box_shape.cc:53-106) examines both
        // band endpoints and the straight edge between corner arcs.
        if lo <= y + h - bottom.1 && hi >= y + top.1 {
            return full.clamp(0.0, self.canvas.0);
        }
        let at = |py: f32| {
            let (radius, dy) = if py < y + top.1 {
                (top, y + top.1 - py)
            } else if py > y + h - bottom.1 {
                (bottom, py - (y + h - bottom.1))
            } else {
                return full.clamp(0.0, self.canvas.0);
            };
            let t = (dy / radius.1).clamp(0.0, 1.0);
            full - radius.0 * (1.0 - (1.0 - t * t).max(0.0).sqrt())
        };
        at(lo).max(at(hi)).clamp(0.0, self.canvas.0)
    }

    pub(crate) fn hash_bits(&self) -> u64 {
        self.rect
            .iter()
            .copied()
            .chain(self.radii.iter().flat_map(|r| [r.0, r.1]))
            .chain([self.canvas.0, self.canvas.1])
            .fold(self.side as u64, |hash, v| {
                hash.rotate_left(7) ^ v.to_bits() as u64
            })
    }
}
