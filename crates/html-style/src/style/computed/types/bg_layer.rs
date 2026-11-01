//! Фоновые компоненты одного слоя: разбираются каскадом, копируются при отрисовке.

use super::*;
use crate::values::ValueCx;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BgLayer {
    pub image: Option<ImageSource>,
    pub size: BgSize,
    pub position: BgPos,
    pub(super) position_math: [Option<super::super::background_position::PositionExpression>; 2],
    pub repeat: Option<BgRepeat>,
    pub origin: Option<BgClip>,
    pub clip: Option<BgClip>,
    pub fixed: Option<bool>,
    pub blend: Option<BlendMode>,
}

impl BgLayer {
    pub fn from_computed(c: &Computed) -> Self {
        Self {
            image: c.reset_background.bg_image.clone(),
            size: c.reset_background.bg_size,
            position: c.reset_background.bg_pos,
            position_math: c.reset_background.bg_position_math.clone(),
            repeat: c.reset_background.bg_repeat,
            origin: c.reset_background.bg_origin,
            clip: c.reset_background.bg_clip,
            fixed: c.reset_background.bg_fixed,
            blend: c.reset_background.background_blend,
        }
    }

    /// Циклический выбор компонента слоя без промежуточного стиля элемента.
    pub fn copy_component(&mut self, prop: BgListProp, source: &Self) {
        match prop {
            BgListProp::Image => self.image = source.image.clone(),
            BgListProp::Size => self.size = source.size,
            BgListProp::PositionX => {
                self.position.x = source.position.x;
                self.position_math[0] = source.position_math[0].clone();
            }
            BgListProp::PositionY => {
                self.position.y = source.position.y;
                self.position_math[1] = source.position_math[1].clone();
            }
            BgListProp::Repeat => self.repeat = source.repeat,
            BgListProp::Origin => self.origin = source.origin,
            BgListProp::Clip => self.clip = source.clip,
            BgListProp::Attachment => self.fixed = source.fixed,
            BgListProp::BlendMode => self.blend = source.blend,
        }
    }

    pub fn position_used(&self, fallback: (f32, f32), free: (f32, f32)) -> (f32, f32) {
        super::super::background_position::used(&self.position_math, fallback, free)
    }

    pub(super) fn parse(prop: BgListProp, raw: &str, vcx: ValueCx<'_>) -> Self {
        let mut c = Computed::default();
        c.apply_one(prop.key(), raw, vcx);
        if c.reset_background.bg_image.is_none() {
            c.reset_background.bg_image =
                c.reset_background
                    .gradient
                    .take()
                    .map(|value| ImageSource::Gradient {
                        value: Box::new(value),
                        repeating: false,
                    });
        }
        Self {
            image: c.reset_background.bg_image.take(),
            size: c.reset_background.bg_size,
            position: c.reset_background.bg_pos,
            position_math: std::mem::take(&mut c.reset_background.bg_position_math),
            repeat: c.reset_background.bg_repeat,
            origin: c.reset_background.bg_origin,
            clip: c.reset_background.bg_clip,
            fixed: c.reset_background.bg_fixed,
            blend: c.reset_background.background_blend,
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

    pub fn resolve_lengths(&mut self, fix: &impl Fn(&mut Option<Len>), vcx: ValueCx<'_>) {
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
}
