//! Фоновые компоненты одного слоя: разбираются каскадом, копируются при отрисовке.

use super::*;
use crate::document::state::ValueCx;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BgLayer {
    pub(super) pending: Option<String>,
    image: Option<String>,
    size: BgSize,
    position: BgPos,
    pub(super) position_math: [Option<super::super::background_position::PositionExpression>; 2],
    repeat: Option<BgRepeat>,
    origin: Option<BgClip>,
    clip: Option<BgClip>,
    fixed: Option<bool>,
    blend: Option<BlendMode>,
}

impl BgLayer {
    pub(super) fn parse(
        prop: BgListProp,
        raw: &str,
        metrics: Option<(f32, f32, f32)>,
        vcx: ValueCx<'_>,
    ) -> Self {
        let relative = has_font_units(raw);
        let value = if relative {
            let (font, ex, ch) = metrics.unwrap_or((16.0, 8.0, 8.0));
            font_lengths_to_px(raw, font, 16.0, ex, ch)
        } else {
            raw.to_string()
        };
        let mut c = Computed::default();
        c.apply_one(prop.key(), &value, vcx);
        // A multi-layer gradient always takes the image-tile path. Extract
        // its function here, rather than scanning the shorthand during paint.
        if c.bg_image.is_none()
            && c.gradient.is_some()
            && let Some(at) = value.find("gradient(")
        {
            let start = value[..at]
                .rfind(|ch: char| ch.is_whitespace() || ch == ',')
                .map_or(0, |p| p + 1);
            let mut depth = 0;
            for (i, ch) in value[at..].char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            c.bg_image = Some(value[start..at + i + 1].to_string());
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        Self {
            pending: (relative && metrics.is_none()).then(|| raw.to_string()),
            image: c.bg_image,
            size: c.bg_size,
            position: c.bg_pos,
            position_math: c.bg_position_math,
            repeat: c.bg_repeat,
            origin: c.bg_origin,
            clip: c.bg_clip,
            fixed: c.bg_fixed,
            blend: c.background_blend,
        }
    }

    pub(crate) fn apply(&self, prop: BgListProp, c: &mut Computed) {
        match prop {
            BgListProp::Shorthand => {
                c.bg_image = self.image.clone();
                c.bg_size = self.size;
                c.bg_pos = self.position;
                c.bg_position_math = self.position_math.clone();
                c.bg_repeat = self.repeat;
                c.bg_origin = self.origin;
                if self.clip.is_some() {
                    c.bg_clip = self.clip;
                }
                if self.fixed.is_some() {
                    c.bg_fixed = self.fixed;
                }
            }
            BgListProp::Image => c.bg_image = self.image.clone(),
            BgListProp::Size => c.bg_size = self.size,
            BgListProp::Position => {
                c.bg_pos = self.position;
                c.bg_position_math = self.position_math.clone();
            }
            BgListProp::Repeat => c.bg_repeat = self.repeat,
            BgListProp::Origin => c.bg_origin = self.origin,
            BgListProp::Clip => c.bg_clip = self.clip,
            BgListProp::Attachment => c.bg_fixed = self.fixed,
            BgListProp::BlendMode => c.background_blend = self.blend,
        }
    }
}
