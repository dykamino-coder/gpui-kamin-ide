//! Resolved CSS path commands shared by SVG serialization and tiny-skia geometry.

mod geometry;
pub(crate) use geometry::Geometry;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(crate) enum Command {
    Move([f32; 2]),
    Line([f32; 2]),
    Horizontal(f32),
    Vertical(f32),
    Cubic([f32; 6]),
    Quadratic([f32; 4]),
    SmoothCubic([f32; 4]),
    SmoothQuadratic([f32; 2]),
    Arc([f32; 7]),
    Close,
}

pub(crate) trait Sink {
    fn command(&mut self, relative: bool, command: Command);
    fn raw(&mut self, path: &str);
}

impl Sink for String {
    fn raw(&mut self, path: &str) {
        self.push_str(path);
    }
    fn command(&mut self, relative: bool, command: Command) {
        use std::fmt::Write;
        let (letter, values): (char, &[f32]) = match &command {
            Command::Move(v) => ('M', v),
            Command::Line(v) => ('L', v),
            Command::Horizontal(v) => ('H', std::slice::from_ref(v)),
            Command::Vertical(v) => ('V', std::slice::from_ref(v)),
            Command::Cubic(v) => ('C', v),
            Command::Quadratic(v) => ('Q', v),
            Command::SmoothCubic(v) => ('S', v),
            Command::SmoothQuadratic(v) => ('T', v),
            Command::Arc(v) => ('A', v),
            Command::Close => ('Z', &[]),
        };
        self.push(if relative {
            letter.to_ascii_lowercase()
        } else {
            letter
        });
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                self.push(' ');
            }
            let _ = write!(self, "{value}");
        }
        self.push(' ');
    }
}
