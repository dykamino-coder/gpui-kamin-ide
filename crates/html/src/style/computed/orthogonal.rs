//! Orthogonal inline sizes distinguish percentage basis from shrink-to-fit space.
//! CSS Writing Modes 4 §7.3.1–2: intrinsic minimums can exceed fallback space.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct AxisSizes {
    pub size: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl AxisSizes {
    pub fn limit(self) -> Option<f32> {
        self.size.or(self.max).map(|v| self.clamp(v))
    }

    pub fn clamp(self, value: f32) -> f32 {
        value
            .min(self.max.unwrap_or(f32::INFINITY))
            .max(self.min.unwrap_or(0.0))
    }
}

// The variants are the CSS keywords `min-content`/`max-content`/`fit-content`.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum InlineKeyword {
    MinContent,
    MaxContent,
    FitContent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InlineConstraint {
    pub available: f32,
    pub fixed: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
}

impl InlineConstraint {
    pub fn used(self, min_content: f32, max_content: f32) -> f32 {
        self.used_keyword(None, min_content, max_content)
    }

    pub fn used_keyword(
        self,
        keyword: Option<InlineKeyword>,
        min_content: f32,
        max_content: f32,
    ) -> f32 {
        let intrinsic = match keyword {
            Some(InlineKeyword::MinContent) => min_content,
            Some(InlineKeyword::MaxContent) => max_content.max(min_content),
            Some(InlineKeyword::FitContent) | None => max_content
                .max(min_content)
                .min(min_content.max(self.available)),
        };
        AxisSizes {
            size: None,
            min: self.min,
            max: self.max,
        }
        .clamp(self.fixed.unwrap_or(intrinsic))
    }
}

/// A missing nearest scrollport size does not authorize searching farther up.
pub(crate) fn available(parent: AxisSizes, scrollport: Option<AxisSizes>, icb: f32) -> f32 {
    parent
        .limit()
        .unwrap_or(f32::INFINITY)
        .min(
            scrollport
                .and_then(AxisSizes::limit)
                .unwrap_or(f32::INFINITY),
        )
        .min(icb)
        .max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_short_content_does_not_claim_the_entire_fallback() {
        let c = InlineConstraint {
            available: 600.0,
            fixed: None,
            min: None,
            max: None,
        };
        assert_eq!(c.used(120.0, 120.0), 120.0);
        assert_eq!(c.used(700.0, 700.0), 700.0);
        assert_eq!(c.used(30.0, 1000.0), 600.0);
    }

    #[test]
    fn own_minimum_overrides_maximum_and_fixed_size() {
        let c = InlineConstraint {
            available: 600.0,
            fixed: Some(200.0),
            min: Some(400.0),
            max: Some(300.0),
        };
        assert_eq!(c.used(50.0, 1000.0), 400.0);
    }

    #[test]
    fn scrollport_minimum_floors_maximum_then_icb_caps_fallback() {
        let scrollport = AxisSizes {
            size: None,
            min: Some(720.0),
            max: Some(240.0),
        };
        assert_eq!(
            available(AxisSizes::default(), Some(scrollport), 600.0),
            600.0
        );
        assert_eq!(
            available(AxisSizes::default(), Some(AxisSizes::default()), 600.0),
            600.0
        );
        assert_eq!(
            available(
                AxisSizes {
                    size: Some(400.0),
                    ..AxisSizes::default()
                },
                None,
                600.0
            ),
            400.0
        );
    }
}
