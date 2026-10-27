//! Размеры синтетического ряда и продолжения предка спаннера.

use super::BoxDimensions;
use crate::dom::Element;
use crate::style::computed::Computed;

impl BoxDimensions {
    pub(crate) fn for_element(e: &Element, style: &Computed) -> Self {
        let mut sizes: BoxDimensions = Self::from_computed(style);
        if e.marks.spanner_content_box {
            // Preferred size уже несёт остаток; исходные min/max и box-sizing
            // были применены при native-измерении исходной коробки.
            sizes.minimum[1] = None;
            sizes.maximum[1] = None;
            sizes.border_box = false;
        }
        sizes
    }

    /// Щуп кромок исключает ограничения высоты до переноса aspect-ratio.
    pub(crate) fn unconstrained_height(mut self) -> Self {
        self.preferred[1] = None;
        self.minimum[1] = None;
        self.maximum[1] = None;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::block::{available_height, margin_height};
    use crate::style::computed::BoxStyle;
    use crate::style::values::value::Len;

    #[test]
    fn content_remainder_is_the_child_basis_without_original_bounds() {
        let mut e = Element::synthetic(
            "div",
            Computed {
                height: Some(Len::Px(80.)),
                min_height: Some(Len::Px(150.)),
                max_height: Some(Len::Px(180.)),
                border_box: Some(true),
                ..Default::default()
            },
            Default::default(),
            vec![],
        );
        e.style.used_mut().padding.top = Some(Len::Px(10.));
        e.style.used_mut().padding.bottom = Some(Len::Px(20.));
        e.marks.spanner_content_box = true;
        let style = BoxStyle::shared(e.style.used());
        let sizes = BoxDimensions::for_element(&e, &style);
        assert_eq!(
            available_height::inner_with_dimensions(&style, &sizes, None, Some(200.), 600.),
            Some(80.)
        );
        assert_eq!(
            available_height::inner(&style, None, Some(200.), 600.),
            Some(120.)
        );
        let doc = crate::document::state::DocState::new(Default::default());
        assert!(!margin_height::raises(&e, doc.values()));
        assert!(!margin_height::lowers(&e, doc.values()));
        assert_eq!(e.style.used().min_height, Some(Len::Px(150.)));
        assert_eq!(e.style.used().max_height, Some(Len::Px(180.)));
        assert_eq!(e.style.used().border_box, Some(true));
    }
}
