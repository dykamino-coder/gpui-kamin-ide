//! Краска и опорная коробка изображения до применения background-clip.

use crate::document::state::ValueCx;
use crate::style::computed::{BgClip, Computed, Gradient};
use gpui::{Background, Bounds, Pixels};

pub(super) enum Fill<'a> {
    Solid(Background),
    Linear(&'a Gradient, Bounds<Pixels>),
}

impl<'a> Fill<'a> {
    pub(super) fn new(
        c: &Computed,
        bg: &'a crate::style::background_paint::BackgroundValues,
        area: super::ColorBox,
        vcx: ValueCx<'_>,
    ) -> Option<Self> {
        if matches!(area, super::ColorBox::Padding { .. })
            && !bg.gradient_as_tile()
            && let Some(gradient) = &bg.gradient
        {
            // Неначальный origin обслуживает painter плиток. У GPU-заливки остаётся
            // начальный padding-box (css-backgrounds-3 §3.8), независимо от clip.
            return Some(Self::Linear(
                gradient,
                super::geometry::area(c, area, BgClip::PaddingBox, vcx).0,
            ));
        }
        bg.color.map(|v| Self::Solid(Background::from(v.to_hsla())))
    }

    pub(super) fn at(&self, bounds: Bounds<Pixels>, matrix: [f32; 6]) -> Background {
        match self {
            Self::Solid(background) => *background,
            Self::Linear(gradient, origin) => crate::paint::apply::fill(&super::gradient::rebase(
                gradient, *origin, bounds, matrix,
            )),
        }
    }
}
