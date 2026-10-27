//! Проверка явной геометрии до записи или изменения сцены назначения.

use super::*;

pub(super) fn point_valid(point: Point<Pixels>) -> bool {
    point.x.0.is_finite() && point.y.0.is_finite()
}

pub(super) fn bounds_valid(bounds: Bounds<Pixels>) -> bool {
    point_valid(bounds.origin)
        && bounds.size.width.0.is_finite() && bounds.size.height.0.is_finite()
        && bounds.size.width.0 >= 0.0 && bounds.size.height.0 >= 0.0
        && (bounds.origin.x.0 + bounds.size.width.0).is_finite()
        && (bounds.origin.y.0 + bounds.size.height.0).is_finite()
}

impl PaintScopeDescriptor {
    pub(super) fn validate(&self, scale: f32) -> Result<Bounds<Pixels>, PaintRecordError> {
        let bounds = match self {
            Self::Clip(bounds) => *bounds,
            Self::Group(group) => {
                let corners = group.corner_radii;
                let valid = [corners.top_left, corners.top_right, corners.bottom_right,
                    corners.bottom_left].iter().all(|v| v.0.is_finite() && v.0 >= 0.0)
                    && group.blur_radius.is_finite() && group.blur_radius >= 0.0
                    && group.opacity.is_finite() && (0.0..=1.0).contains(&group.opacity)
                    && (group.blur_radius * scale).is_finite()
                    && [corners.top_left, corners.top_right, corners.bottom_right, corners.bottom_left]
                        .iter().all(|v| (v.0 * scale).is_finite())
                    && group.polygon.len() <= 8
                    && group.polygon.iter().copied().all(|p| point_valid(p)
                        && (p.x.0 * scale).is_finite() && (p.y.0 * scale).is_finite())
                    && group.mask.as_ref().is_none_or(|(_, bounds, _)| scaled_bounds_valid(*bounds, scale))
                    && group.mask_clip.is_none_or(|clip| clip.iter().all(|v| v.is_finite())
                        && clip[2] >= 0.0 && clip[3] >= 0.0
                        && (clip[0] + clip[2]).is_finite() && (clip[1] + clip[3]).is_finite())
                    && group.projection.is_none_or(|map| map.iter().all(|v| v.is_finite()))
                    && group.fragments.iter().all(|fragment| fragment.len() <= 8
                        && fragment.iter().copied().all(|p| point_valid(p)
                            && (p.x.0 * scale).is_finite() && (p.y.0 * scale).is_finite()));
                if !valid { return Err(PaintRecordError::InvalidGeometry); }
                group.bounds
            }
        };
        if !scaled_bounds_valid(bounds, scale) { return Err(PaintRecordError::InvalidGeometry); }
        Ok(bounds)
    }
}

// Conversion must stay finite in the actual device coordinate space too.
pub(super) fn scaled_bounds_valid(bounds: Bounds<Pixels>, scale: f32) -> bool {
    bounds_valid(bounds) && [bounds.left().0, bounds.top().0, bounds.right().0,
        bounds.bottom().0, bounds.size.width.0, bounds.size.height.0]
        .iter().all(|v| (v * scale).is_finite())
}
