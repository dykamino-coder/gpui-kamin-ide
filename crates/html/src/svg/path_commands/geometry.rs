//! Разрешение SVG-команд с состоянием в пути tiny-skia.

use crate::style::values::basic_shape::commands::{Command, Sink};
use gpui::path_scene::tiny_skia_path::{Path, PathBuilder};
use std::sync::Arc;

/// Match the shortest decimal f32 values previously passed to SVG's f64
/// coordinate arithmetic. This stack buffer preserves arc control points
/// without constructing or parsing path syntax.
fn coordinate(value: f32) -> f64 {
    ryu::Buffer::new().format(value).parse().unwrap_or(f64::NAN)
}

#[derive(Default)]
pub(crate) struct Geometry {
    pub builder: PathBuilder,
    current: (f64, f64),
    start: (f64, f64),
    cubic: Option<(f64, f64)>,
    quadratic: Option<(f64, f64)>,
    closed: bool,
}

impl Geometry {
    pub fn finish(self) -> Option<Arc<Path>> {
        self.builder.finish().map(Arc::new)
    }

    pub fn parse(raw: &str) -> Option<Arc<Path>> {
        gpui::path_scene::parse_path(raw)
    }
}

impl Sink for Geometry {
    fn raw(&mut self, path: &str) {
        if let Some(path) = Self::parse(path) {
            self.builder.push_path(&path);
        }
    }
    fn command(&mut self, relative: bool, command: Command) {
        use Command::*;
        let (cx, cy) = self.current;
        let point = |x, y| {
            let (x, y) = (coordinate(x), coordinate(y));
            if relative { (cx + x, cy + y) } else { (x, y) }
        };
        if self.closed && !matches!(command, Move(_) | Close) {
            self.builder
                .move_to(self.start.0 as f32, self.start.1 as f32);
        }
        let old_cubic = self.cubic.take();
        let old_quadratic = self.quadratic.take();
        self.closed = false;
        let end = match command {
            Move([x, y]) => {
                let p = point(x, y);
                self.builder.move_to(p.0 as f32, p.1 as f32);
                self.start = p;
                p
            }
            Line([x, y]) => {
                let p = point(x, y);
                self.builder.line_to(p.0 as f32, p.1 as f32);
                p
            }
            Horizontal(x) => {
                let x = coordinate(x) + if relative { cx } else { 0.0 };
                self.builder.line_to(x as f32, cy as f32);
                (x, cy)
            }
            Vertical(y) => {
                let y = coordinate(y) + if relative { cy } else { 0.0 };
                self.builder.line_to(cx as f32, y as f32);
                (cx, y)
            }
            Cubic([x1, y1, x2, y2, x, y]) => {
                let (a, b, p) = (point(x1, y1), point(x2, y2), point(x, y));
                self.builder.cubic_to(
                    a.0 as f32, a.1 as f32, b.0 as f32, b.1 as f32, p.0 as f32, p.1 as f32,
                );
                self.cubic = Some(b);
                p
            }
            SmoothCubic([x2, y2, x, y]) => {
                let a = old_cubic.map_or((cx, cy), |(x, y)| (cx * 2.0 - x, cy * 2.0 - y));
                let (b, p) = (point(x2, y2), point(x, y));
                self.builder.cubic_to(
                    a.0 as f32, a.1 as f32, b.0 as f32, b.1 as f32, p.0 as f32, p.1 as f32,
                );
                self.cubic = Some(b);
                p
            }
            Quadratic([x1, y1, x, y]) => {
                let (a, p) = (point(x1, y1), point(x, y));
                self.builder
                    .quad_to(a.0 as f32, a.1 as f32, p.0 as f32, p.1 as f32);
                self.quadratic = Some(a);
                p
            }
            SmoothQuadratic([x, y]) => {
                let a = old_quadratic.map_or((cx, cy), |(x, y)| (cx * 2.0 - x, cy * 2.0 - y));
                let p = point(x, y);
                self.builder
                    .quad_to(a.0 as f32, a.1 as f32, p.0 as f32, p.1 as f32);
                self.quadratic = Some(a);
                p
            }
            Arc([rx, ry, rotation, large, sweep, x, y]) => {
                let p = point(x, y);
                let arc = kurbo::SvgArc {
                    from: (cx, cy).into(),
                    to: p.into(),
                    radii: (coordinate(rx), coordinate(ry)).into(),
                    x_rotation: coordinate(rotation).to_radians(),
                    large_arc: large != 0.0,
                    sweep: sweep != 0.0,
                };
                if let Some(arc) = kurbo::Arc::from_svg_arc(&arc) {
                    let mut end = (cx, cy);
                    arc.to_cubic_beziers(0.1, |a, b, p| {
                        self.builder.cubic_to(
                            a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32,
                        );
                        end = (p.x, p.y);
                    });
                    // svgtypes retains the calculated endpoint, including its rounding.
                    end
                } else {
                    self.builder.line_to(p.0 as f32, p.1 as f32);
                    p
                }
            }
            Close => {
                self.builder.close();
                self.closed = true;
                self.start
            }
        };
        self.current = end;
    }
}
