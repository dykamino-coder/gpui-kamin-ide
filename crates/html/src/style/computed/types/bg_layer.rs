//! Фоновые компоненты одного слоя: разбираются каскадом, копируются при отрисовке.

use super::*;
use crate::document::state::ValueCx;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BgLayer {
    pub(super) image: Option<ImageSource>,
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
    pub(super) fn parse(prop: BgListProp, raw: &str, vcx: ValueCx<'_>) -> Self {
        let mut c = Computed::default();
        c.apply_one(prop.key(), raw, vcx);
        if c.bg_image.is_none() {
            c.bg_image = c.gradient.map(|value| ImageSource::Gradient {
                value: Box::new(value),
                repeating: false,
            });
        }
        Self {
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

    pub(super) fn needs_line_height(&self, vcx: ValueCx<'_>) -> bool {
        let size = match self.size {
            BgSize::Fixed(w, h) => [w, h],
            _ => [None, None],
        };
        self.image
            .as_ref()
            .is_some_and(|i| i.needs_line_height(vcx))
            || [self.position.x, self.position.y]
                .into_iter()
                .chain(size)
                .flatten()
                .any(|l| l.needs_line_height(vcx))
    }

    pub(super) fn resolve_lengths(&mut self, fix: &impl Fn(&mut Option<Len>), vcx: ValueCx<'_>) {
        if let Some(image) = &mut self.image {
            image.resolve_lengths(fix, vcx);
        }
        fix(&mut self.position.x);
        fix(&mut self.position.y);
        if let BgSize::Fixed(w, h) = &mut self.size {
            fix(w);
            fix(h);
        }
    }

    pub(crate) fn apply(&self, prop: BgListProp, c: &mut Computed) {
        match prop {
            BgListProp::Image => c.bg_image = self.image.clone(),
            BgListProp::Size => c.bg_size = self.size,
            BgListProp::PositionX => {
                c.bg_pos.x = self.position.x;
                c.bg_position_math[0] = self.position_math[0].clone();
            }
            BgListProp::PositionY => {
                c.bg_pos.y = self.position.y;
                c.bg_position_math[1] = self.position_math[1].clone();
            }
            BgListProp::Repeat => c.bg_repeat = self.repeat,
            BgListProp::Origin => c.bg_origin = self.origin,
            BgListProp::Clip => c.bg_clip = self.clip,
            BgListProp::Attachment => c.bg_fixed = self.fixed,
            BgListProp::BlendMode => c.background_blend = self.blend,
        }
    }
}
