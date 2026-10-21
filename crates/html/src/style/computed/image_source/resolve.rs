//! Обход вычисленных длин изображений и самодостаточные операнды растрового кэша.

use super::{ImageSource, Len};
use crate::document::state::ValueCx;
use crate::style::computed::{GradientShape, RadialSize};

impl ImageSource {
    pub(crate) fn needs_line_height(&self, vcx: ValueCx<'_>) -> bool {
        match self {
            Self::Gradient { value, .. } => value.needs_line_height(vcx),
            Self::Conic(value) => [value.position.x, value.position.y]
                .into_iter()
                .flatten()
                .any(|l| l.needs_line_height(vcx)),
            Self::CrossFade(items) => items.iter().any(|(image, _)| image.needs_line_height(vcx)),
            Self::Filter { image, program } => {
                image.needs_line_height(vcx) || program.needs_line_height(vcx)
            }
            _ => false,
        }
    }

    pub(crate) fn resolve_lengths(&mut self, fix: &impl Fn(&mut Option<Len>), vcx: ValueCx<'_>) {
        match self {
            Self::Gradient { value, .. } => value.resolve_lengths(fix, vcx),
            Self::Conic(value) => value.resolve_lengths(fix),
            Self::CrossFade(items) => {
                for (image, _) in items {
                    image.resolve_lengths(fix, vcx);
                }
            }
            Self::Filter { image, program } => {
                image.resolve_lengths(fix, vcx);
                program.resolve_lengths(fix, vcx);
            }
            _ => {}
        }
    }

    /// Arena indices cannot identify a value in a process-wide numeric cache.
    pub(crate) fn cacheable(&self) -> bool {
        let plain = |l: &Option<Len>| l.is_none_or(|l| matches!(l, Len::Px(_) | Len::Pct(_)));
        match self {
            Self::Gradient { value, .. } => {
                value.stop_lengths.is_empty()
                    && match &value.shape {
                        GradientShape::Radial { position, size, .. } => {
                            plain(&position.x)
                                && plain(&position.y)
                                && match size {
                                    RadialSize::Explicit(first, second) => {
                                        plain(&Some(*first)) && plain(second)
                                    }
                                    _ => true,
                                }
                        }
                        _ => true,
                    }
            }
            Self::Conic(value) => plain(&value.position.x) && plain(&value.position.y),
            Self::Color(_) => true,
            _ => false,
        }
    }
}
