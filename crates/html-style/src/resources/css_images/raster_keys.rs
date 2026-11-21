//! Канонические числовые ключи: только операнды, прочитанные растеризаторами.
//! Арены и авторские выражения сюда не попадают; длины решаются до сравнения.

use super::{gradient_geometry::Geometry, gradient_raster::resolved_stops, len_px};
use crate::cache::FloatKey;
use crate::style::computed::{GradSpace, ImageFilter, ImageSource};
use crate::style::values::{color_space::DirectGradient, value::Color};
use crate::values::ValueCx;

type Stop = ([FloatKey; 4], FloatKey);
type Palette = (Vec<[FloatKey; 3]>, bool, bool);

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Interpolation {
    space: GradSpace,
    hue: u8,
    palette: Option<Palette>,
}

impl Interpolation {
    pub(super) fn bytes(&self) -> usize {
        self.palette
            .as_ref()
            .map_or(0, |p| std::mem::size_of_val(p.0.as_slice()))
    }

    pub(super) fn new(space: GradSpace, hue: u8, palette: Option<&DirectGradient>) -> Self {
        Self {
            space,
            hue,
            palette: palette.map(|p| {
                let (colors, ok, single) = p.operands();
                (
                    colors.iter().map(|c| c.map(FloatKey::from)).collect(),
                    ok,
                    single,
                )
            }),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum GeometryKey {
    Linear([FloatKey; 3]),
    Radial([FloatKey; 4]),
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum ImageKey {
    Gradient {
        geometry: GeometryKey,
        stops: Vec<Stop>,
        interpolation: Interpolation,
        repeating: bool,
    },
    Conic {
        from: FloatKey,
        center: [FloatKey; 2],
        stops: Vec<Stop>,
        hints: Vec<(usize, FloatKey)>,
        interpolation: Interpolation,
        repeating: bool,
    },
    Color([FloatKey; 4]),
}

impl ImageKey {
    /// The first field in which two keys differ, for the DevTools `why`
    /// journal of a cache miss (`gradient_why`); `""` when equal.
    pub(super) fn differs(&self, other: &Self) -> &'static str {
        use ImageKey::{Color, Conic, Gradient};
        match (self, other) {
            (Gradient { geometry: g1, stops: s1, interpolation: i1, repeating: r1 },
             Gradient { geometry: g2, stops: s2, interpolation: i2, repeating: r2 }) => {
                [(g1 != g2, "geometry"), (s1 != s2, "stops"), (i1 != i2, "interpolation"), (r1 != r2, "repeating")]
                    .into_iter()
                    .find_map(|(d, name)| d.then_some(name))
                    .unwrap_or("")
            }
            (Conic { from: f1, center: c1, stops: s1, hints: h1, interpolation: i1, repeating: r1 },
             Conic { from: f2, center: c2, stops: s2, hints: h2, interpolation: i2, repeating: r2 }) => {
                [(f1 != f2, "from"), (c1 != c2, "center"), (s1 != s2, "stops"), (h1 != h2, "hints"),
                    (i1 != i2, "interpolation"), (r1 != r2, "repeating")]
                    .into_iter()
                    .find_map(|(d, name)| d.then_some(name))
                    .unwrap_or("")
            }
            (Color(a), Color(b)) if a == b => "",
            (Color(_), Color(_)) => "color",
            _ => "kind",
        }
    }

    /// Память вычисленного ключа, включая стопы и исходные полярные оси.
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::Gradient {
                stops,
                interpolation,
                ..
            } => std::mem::size_of_val(stops.as_slice()) + interpolation.bytes(),
            Self::Conic {
                stops,
                hints,
                interpolation,
                ..
            } => {
                std::mem::size_of_val(stops.as_slice())
                    + std::mem::size_of_val(hints.as_slice())
                    + interpolation.bytes()
            }
            Self::Color(_) => 0,
        }
    }

    pub(super) fn new(src: &ImageSource, css: (f32, f32), vcx: ValueCx<'_>) -> Option<Self> {
        match src {
            ImageSource::Gradient {
                value: g,
                repeating,
            } => {
                let angle = g.shape.corner_angle(css).unwrap_or(g.angle_deg);
                let geometry = Geometry::new(g, css, angle, vcx)?;
                let stops = stops(&resolved_stops(g, geometry.length()));
                let geometry = match geometry {
                    Geometry::Linear {
                        direction: (x, y),
                        length,
                    } => GeometryKey::Linear([x, y, length].map(FloatKey::from)),
                    Geometry::Radial {
                        center: (x, y),
                        radii: (rx, ry),
                    } => GeometryKey::Radial([x, y, rx, ry].map(FloatKey::from)),
                };
                Some(Self::Gradient {
                    geometry,
                    stops,
                    interpolation: Interpolation::new(g.space, g.hue, g.polar.as_ref()),
                    repeating: *repeating,
                })
            }
            ImageSource::Conic(g) => Self::conic(g, css, vcx),
            ImageSource::Color(c) => Some(Self::Color(color(c.value))),
            _ => None,
        }
    }

    pub(super) fn conic(
        g: &crate::style::computed::ConicGradient,
        css: (f32, f32),
        vcx: ValueCx<'_>,
    ) -> Option<Self> {
        Some(Self::Conic {
            from: g.from.into(),
            center: [
                len_px(g.position.x, css.0, vcx)?,
                len_px(g.position.y, css.1, vcx)?,
            ]
            .map(FloatKey::from),
            stops: stops(&g.stops),
            hints: g.hints.iter().map(|&(i, v)| (i, v.into())).collect(),
            interpolation: Interpolation::new(g.space, g.hue, g.polar.as_ref()),
            repeating: g.repeating,
        })
    }
}

fn color(c: Color) -> [FloatKey; 4] {
    [c.r, c.g, c.b, c.a].map(FloatKey::from)
}

pub(super) fn stops(values: &[(Color, f32)]) -> Vec<Stop> {
    values
        .iter()
        .map(|&(c, at)| (color(c), at.into()))
        .collect()
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct FilterKey {
    filter: Option<[FloatKey; 9]>,
    /// Authored color operations: the raster applies them in this order.
    operations: Vec<[FloatKey; 9]>,
    shadow: Option<([FloatKey; 3], [FloatKey; 4])>,
    calls: usize,
}

fn filter_fields(f: crate::style::computed::Filter) -> [FloatKey; 9] {
    let crate::style::computed::Filter {
        grayscale,
        brightness,
        saturate,
        invert,
        sepia,
        opacity,
        hue_rotate,
        contrast,
        blur,
    } = f;
    [
        grayscale, brightness, saturate, invert, sepia, opacity, hue_rotate, contrast, blur,
    ]
    .map(FloatKey::from)
}

impl From<&ImageFilter> for FilterKey {
    fn from(list: &ImageFilter) -> Self {
        Self {
            filter: list.filter.map(filter_fields),
            operations: list.operations.iter().copied().map(filter_fields).collect(),
            shadow: list.shadow.as_ref().map(|s| {
                (
                    [s.x, s.y, s.blur].map(FloatKey::from),
                    color(list.shadow_used),
                )
            }),
            calls: list.calls,
        }
    }
}
