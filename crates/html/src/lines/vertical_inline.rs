//! Resolve vertical text's inline contribution during the parent's native measurement.
//! Intrinsic probes and final constraints share the actual paragraph shaping path.

use super::*;
use crate::computed::orthogonal::{InlineConstraint, InlineKeyword};

impl Paragraph {
    pub(crate) fn vertical_inline_constraint(
        mut self,
        constraint: Option<InlineConstraint>,
        keyword: Option<InlineKeyword>,
    ) -> Self {
        self.vertical_inline = constraint.map(|constraint| (constraint, keyword));
        self
    }

    pub(super) fn measured_inline_limit(
        &mut self,
        known: Option<Pixels>,
        available: gpui::AvailableSpace,
        fallback: Option<Pixels>,
        window: &mut Window,
    ) -> Option<Pixels> {
        if let Some(known) = known {
            return Some(known);
        }
        if self.vertical
            && let Some((constraint, keyword)) = self.vertical_inline
        {
            let minimum = f32::from(self.min_content(window));
            let maximum = self
                .split(None, window)
                .iter()
                .map(|line| f32::from(line.width + line.indent.max(px(0.0))))
                .fold(minimum, f32::max);
            return Some(px(resolve_inline(
                constraint, keyword, available, minimum, maximum,
            )));
        }
        match available {
            gpui::AvailableSpace::Definite(value) => Some(value),
            gpui::AvailableSpace::MaxContent if self.vertical => fallback,
            gpui::AvailableSpace::MaxContent => None,
            gpui::AvailableSpace::MinContent => Some(self.min_content(window)),
        }
    }
}

fn resolve_inline(
    mut constraint: InlineConstraint,
    keyword: Option<InlineKeyword>,
    available: gpui::AvailableSpace,
    minimum: f32,
    maximum: f32,
) -> f32 {
    // A layout-time track size can tighten the inherited fallback, but cannot
    // turn an unbreakable minimum into a smaller wrapped contribution.
    if let gpui::AvailableSpace::Definite(value) = available {
        constraint.available = constraint.available.min(f32::from(value));
    }
    let keyword = keyword.or_else(|| match available {
        gpui::AvailableSpace::MinContent => Some(InlineKeyword::MinContent),
        _ => None,
    });
    constraint.used_keyword(keyword, minimum, maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constraint() -> InlineConstraint {
        InlineConstraint {
            available: 600.0,
            fixed: None,
            min: None,
            max: None,
        }
    }

    #[test]
    fn final_track_constrains_fit_content_without_using_the_longest_wrapped_line() {
        let available = gpui::AvailableSpace::Definite(px(100.0));
        assert_eq!(
            resolve_inline(constraint(), None, available, 30.0, 140.0),
            100.0
        );
        assert_eq!(
            resolve_inline(constraint(), None, available, 120.0, 140.0),
            120.0
        );
        assert_eq!(
            resolve_inline(constraint(), None, available, 30.0, 80.0),
            80.0
        );
    }

    #[test]
    fn intrinsic_probe_and_authored_keyword_remain_distinct() {
        assert_eq!(
            resolve_inline(
                constraint(),
                None,
                gpui::AvailableSpace::MinContent,
                30.0,
                900.0
            ),
            30.0
        );
        assert_eq!(
            resolve_inline(
                constraint(),
                None,
                gpui::AvailableSpace::MaxContent,
                30.0,
                900.0
            ),
            600.0
        );
        assert_eq!(
            resolve_inline(
                constraint(),
                Some(InlineKeyword::MaxContent),
                gpui::AvailableSpace::Definite(px(100.0)),
                30.0,
                900.0
            ),
            900.0
        );
    }

    #[test]
    fn fixed_inline_size_and_own_minimum_survive_tighter_parent_space() {
        let mut constraint = constraint();
        constraint.fixed = Some(200.0);
        constraint.min = Some(300.0);
        constraint.max = Some(250.0);
        assert_eq!(
            resolve_inline(
                constraint,
                None,
                gpui::AvailableSpace::Definite(px(100.0)),
                30.0,
                900.0
            ),
            300.0
        );
    }
}
