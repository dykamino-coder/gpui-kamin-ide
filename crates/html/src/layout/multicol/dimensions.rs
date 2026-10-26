//! Размеры коробки как вход многоколоночной раскладки. Native-мера и остаток
//! ряда не меняют вычисленные CSS-значения исходного элемента.

use crate::style::computed::{Computed, Display, Position};
use crate::style::values::value::Len;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MulticolDimensions {
    pub(crate) preferred: [Option<Len>; 2],
    pub(crate) minimum: [Option<Len>; 2],
    pub(crate) maximum: [Option<Len>; 2],
    pub(crate) border_box: bool,
}

impl MulticolDimensions {
    pub(crate) fn css(style: &Computed) -> Self {
        Self {
            preferred: [style.width, style.height],
            minimum: [style.min_width, style.min_height],
            maximum: [style.max_width, style.max_height],
            border_box: style.border_box == Some(true),
        }
    }

    /// Доступный размер native-фазы уже относится к содержимому хоста.
    pub(crate) fn measured(style: &Computed, width: f32, height: Option<f32>) -> Self {
        let mut input: MulticolDimensions = Self::css(style);
        input.preferred[0] = Some(Len::Px(width));
        if let Some(height) = height {
            input.preferred[1] = Some(Len::Px(height));
        }
        input.border_box = false;
        input
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
        let length = |value| match value {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => outer.map(|v| k * v),
            _ => None,
        };
        let side = |value| match value {
            None | Some(Len::Auto) => Some(0.),
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => inline.map(|v| k * v),
            _ => None,
        };
        let edges = if self.border_box {
            let borders = style.borders();
            side(style.padding.top)?
                + side(style.padding.bottom)?
                + side(borders.top)?
                + side(borders.bottom)?
        } else {
            0.
        };
        let height = (length(self.preferred[1])? - edges).max(0.);
        let min = length(self.minimum[1])
            .map(|v| (v - edges).max(0.))
            .unwrap_or(0.);
        let max = length(self.maximum[1])
            .map(|v| (v - edges).max(0.))
            .unwrap_or(f32::INFINITY);
        Some(height.min(max).max(min))
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
