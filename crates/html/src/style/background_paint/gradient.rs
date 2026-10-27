//! Выбор пути градиента по собственным операндам фона.

impl super::BackgroundValues {
    pub(crate) fn gradient_as_tile(&self) -> bool {
        // Несколько слоёв рисует стопка плиток (`bg_layers`): заливка коробки
        // верхним градиентом легла бы ПОД нижние слои.
        if self.gradient.is_some() && self.layers.is_some() {
            return true;
        }
        self.gradient_image.is_some()
            && (self.gradient.as_ref().is_some_and(|g| {
                g.radial
                    || g.stops_raw.iter().any(|(_, _, px)| px.is_some())
                    || g.stops.iter().any(|s| crate::style::values::color_space::out_of_gamut(s.0))
            })
                || self.top.size != crate::style::computed::BgSize::Auto
                || self.top.repeat.is_some()
                || self.top.position.x.is_some()
                || self.top.position.y.is_some()
                || self.top.origin.is_some()
                // Угол разрешается по конкретному размеру картинки;
                // этой геометрией владеет отрисовка плитки.
                || (self.top.clip != Some(crate::style::computed::BgClip::Text)
                    && self.gradient.as_ref().is_some_and(|g| {
                        g.shape.corner_angle((1.0, 1.0)).is_some()
                    }))
                // Пространство смешения, которого GPU-путь не выражает
                // (всё, кроме гамма-sRGB и OKLab — css-color-4 §12.2):
                // цвет обязан считаться на точку, иначе полярную дугу и
                // линейный свет пришлось бы изображать полосами, а
                // квантование полос уже замерено в минус (см. `HSL_ARC`).
                || !matches!(
                    self.gradient.as_ref().map(|g| g.space),
                    None | Some(crate::style::computed::GradSpace::Srgb)
                        | Some(crate::style::computed::GradSpace::Oklab)
                )
                // Цвет фона лежит ПОД всеми слоями (css-backgrounds-3 §3.1):
                // у заливки коробки место одно, поэтому цвет — ей, градиент —
                // слоем сверху (`bg-color-with-gradient`).
                || self.color.is_some_and(|c| c.a > 0.0))
    }
}
