//! Вычисленный стиль: выбор пути краски градиента — сплошная заливка GPU или
//! растровая плитка (`gradient_as_tile`). Плитка нужна, когда требуется её
//! механика (размер, повтор, позиция, origin, несколько слоёв, цвет фона под
//! градиентом), радиальной форме, стопам в точках, цветам вне гаммы и
//! пространствам смешения, которых GPU-путь не выражает (css-color-4 §12.2).

use crate::style::computed::BgListProp;

impl super::Computed {
    /// Градиент, которому нужна МЕХАНИКА ПЛИТКИ (размер, повтор, позиция,
    /// свой край): сплошная заливка её не умеет, рисует слой-картинка.
    pub(crate) fn gradient_as_tile(&self) -> bool {
        // Несколько слоёв рисует стопка плиток (`bg_layers`): заливка коробки
        // верхним градиентом легла бы ПОД нижние слои.
        if self.gradient.is_some() && self.bg_lists.get(BgListProp::Image).is_some() {
            return true;
        }
        self.gradient_raw.is_some()
            && (self.gradient.as_ref().is_some_and(|g| {
                g.radial
                    || g.stops_raw.iter().any(|(_, _, px)| px.is_some())
                    || g.stops.iter().any(|s| crate::style::values::color_space::out_of_gamut(s.0))
            })
                || self.bg_size != crate::style::computed::BgSize::Auto
                || self.bg_repeat.is_some()
                || self.bg_pos.x.is_some()
                || self.bg_pos.y.is_some()
                || self.bg_origin.is_some()
                // The angle cannot be resolved until the image's concrete
                // size is known; the tile painter owns that geometry.
                || (self.bg_clip != Some(crate::style::computed::BgClip::Text)
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
                || self.background.is_some_and(|c| c.a > 0.0))
    }
}
