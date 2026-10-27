//! Вычисленные фоновые операнды донора холста без изменения CSS корня.

use super::computed::{BgLayer, Computed, Gradient, ImageSource};
use super::values::value::Color;
use crate::document::state::ValueCx;

/// Собственный источник фона холста; роль не наследуется как CSS.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasBackgroundSource {
    Root,
    Donor(std::rc::Rc<CanvasBackground>),
}

/// css-backgrounds-3 §2.11.2: краска берётся у донора, геометрия — у корня.
/// Выражения цвета уже подготовлены с цветом донора; исходный CSS остаётся
/// источником явного inherit и разрешается в контексте самого потомка.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanvasBackground {
    pub color: Option<Color>,
    pub gradient: Option<Gradient>,
    pub gradient_image: Option<ImageSource>,
    pub top: BgLayer,
    pub layers: Option<Vec<BgLayer>>,
}

impl CanvasBackground {
    pub(crate) fn from_computed(donor: &Computed) -> Self {
        Self {
            color: donor.background,
            gradient: donor.gradient.clone(),
            gradient_image: donor.gradient_raw.clone(),
            top: BgLayer::from_computed(donor),
            layers: donor.background_layers(),
        }
    }

    /// Позднее разрешение единиц окна тем же обходом операндов, что у CSS.
    /// Размер области и ось берутся от корня, как до выделения этого входа.
    pub(crate) fn resolve_viewport(
        &mut self,
        root: &Computed,
        viewport: (f32, f32),
        vcx: ValueCx<'_>,
    ) {
        let vp = root.viewport_cx(viewport);
        let fix = |l: &mut Option<super::values::value::Len>| {
            *l = l.map(|l| l.fold_viewport(vp, vcx));
        };
        if let Some(gradient) = &mut self.gradient {
            gradient.resolve_lengths(fix, vcx);
        }
        for image in [&mut self.gradient_image, &mut self.top.image]
            .into_iter()
            .flatten()
        {
            image.resolve_lengths(&fix, vcx);
        }
        for layer in self.layers.iter_mut().flatten() {
            layer.resolve_lengths(&fix, vcx);
        }
    }
}
