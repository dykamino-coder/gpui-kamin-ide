//! Размеры коробки как вход многоколоночной раскладки. Native-мера и остаток
//! ряда не меняют вычисленные CSS-значения исходного элемента.

use crate::style::computed::{Computed, Display, Position};
use crate::style::values::value::Len;

pub(crate) use crate::layout::block::sizing::BoxDimensions as MulticolDimensions;

impl MulticolDimensions {
    #[cfg(test)]
    pub(crate) fn css(style: &Computed) -> Self {
        Self::from_computed(style)
    }

    /// Доступный размер native-фазы уже относится к содержимому хоста.
    #[cfg(test)]
    pub(crate) fn measured(style: &Computed, width: f32, height: Option<f32>) -> Self {
        Self::css(style).with_measured_size(width, height)
    }

    pub(crate) fn with_measured_size(mut self, width: f32, height: Option<f32>) -> Self {
        self.preferred[0] = Some(Len::Px(width));
        if let Some(height) = height {
            self.preferred[1] = Some(Len::Px(height));
        }
        self.border_box = false;
        self
    }

    pub(crate) fn prefix(self) -> Self {
        Self {
            preferred: [self.preferred[0], None],
            ..self
        }
    }

    /// Процентный CB колонок соблюдает те же границы, что обычный блок.
    pub(crate) fn percentage_height(
        self,
        style: &Computed,
        outer: Option<f32>,
        inline: Option<f32>,
        viewport: f32,
    ) -> Option<f32> {
        match style.display {
            Some(Display::Contents) => return outer,
            None | Some(Display::Block | Display::ListItem | Display::InlineBlock) => {}
            _ => return None,
        }
        if style.wm().vertical()
            || (matches!(self.preferred[1], Some(Len::Pct(_)))
                && matches!(style.position, Some(Position::Absolute | Position::Fixed)))
        {
            return None;
        }
        self.content_height(
            style,
            if style.root_box {
                Some(viewport)
            } else {
                outer
            },
            inline,
        )
    }

    /// Определённая физическая высота содержимого для процентов и линеек.
    pub(crate) fn content_height(
        self,
        style: &Computed,
        outer: Option<f32>,
        inline: Option<f32>,
    ) -> Option<f32> {
        crate::layout::block::available_height::content_block_size_with_dimensions(
            style,
            &self,
            false,
            self.preferred[1],
            outer,
            inline,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_content_and_prefix_keep_css_constraints_without_style_writes() {
        let style = Computed {
            width: Some(Len::Pct(0.5)),
            height: Some(Len::Px(200.)),
            min_height: Some(Len::Px(60.)),
            max_height: Some(Len::Px(90.)),
            border_box: Some(true),
            ..Default::default()
        };
        let input = MulticolDimensions::measured(&style, 240., Some(80.));
        assert_eq!(input.preferred, [Some(Len::Px(240.)), Some(Len::Px(80.))]);
        assert_eq!(input.content_height(&style, None, None), Some(80.));
        assert_eq!(input.prefix().content_height(&style, None, None), None);
        assert_eq!(input.minimum, MulticolDimensions::css(&style).minimum);
        assert_eq!(style.width, Some(Len::Pct(0.5)));
        assert_eq!(style.height, Some(Len::Px(200.)));
        assert_eq!(style.border_box, Some(true));
    }

    #[test]
    fn percentage_basis_preserves_container_and_position_boundaries() {
        let mut style = Computed {
            height: Some(Len::Pct(0.5)),
            ..Default::default()
        };
        let basis = |style: &Computed| {
            MulticolDimensions::css(style).percentage_height(style, Some(100.), None, 600.)
        };
        assert_eq!(basis(&style), Some(50.));
        style.root_box = true;
        assert_eq!(basis(&style), Some(300.));
        style.root_box = false;
        style.position = Some(Position::Absolute);
        assert_eq!(basis(&style), None);
        style.position = None;
        style.display = Some(Display::Flex);
        assert_eq!(basis(&style), None);
        style.display = Some(Display::Contents);
        assert_eq!(basis(&style), Some(100.));
    }
}
