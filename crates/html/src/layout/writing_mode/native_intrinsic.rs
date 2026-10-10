//! Intrinsic block and grid boxes retain their actual node and containing block.
use crate::dom::Element;
use crate::style::computed::{Display, Position};
use crate::style::values::value::Len;

fn keyword(value: Option<Len>) -> bool {
    matches!(
        value,
        Some(Len::MinContent | Len::MaxContent | Len::FitContent)
    )
}

pub(crate) fn eligible(element: &Element) -> bool {
    let style = &element.style;
    let grid = matches!(
        style.display,
        Some(Display::Grid | Display::InlineGrid | Display::GridLanes)
    );
    (grid || matches!(style.display, None | Some(Display::Block)))
        && (grid || style.inline_display != Some(true))
        && !matches!(style.position, Some(Position::Absolute | Position::Fixed))
        && !style.float.is_some_and(|value| value != 0)
        && !style.intrinsic_wrapper_required
        && !style.contains_width() && !style.contains_height()
        && style.calc_size.iter().all(Option::is_none)
        && (keyword(style.width) || keyword(style.height))
        && ![style.min_width, style.max_width, style.min_height, style.max_height]
            .into_iter().any(keyword)
        && !crate::render::replaced_tag(element)
        // These tags use independent form/table/list formatting builders.
        && !matches!(element.tag.as_str(),
            "table" | "caption" | "colgroup" | "col" | "thead" | "tbody" | "tfoot" |
            "tr" | "td" | "th" | "input" | "select" | "textarea" | "button" |
            "fieldset" | "legend" | "meter" | "progress" | "ul" | "ol" | "li" |
            "menu" | "details" | "summary" | "hr" | "canvas")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_keyword_items_keep_margins_placement_and_numeric_limits() {
        let mut element = crate::layout::table::anon::anon_element("div", vec![]);
        element.style.width = Some(Len::MinContent);
        element.style.height = Some(Len::MaxContent);
        element.style.margin.left = Some(Len::Px(-10.0));
        element.style.margin.right = Some(Len::Auto);
        element.style.min_width = Some(Len::Px(20.0));
        element.style.max_width = Some(Len::Px(100.0));
        element.style.vertical = Some(true);
        assert!(eligible(&element));
        assert!(!crate::render::content_sized_wraps(&element));
        element.style.intrinsic_wrapper_required = true;
        assert!(!eligible(&element));
        assert!(crate::render::content_sized_wraps(&element));
    }

    #[test]
    fn intrinsic_grid_containers_keep_keywords_on_their_actual_node() {
        let mut element = crate::layout::table::anon::anon_element("div", vec![]);
        element.style.height = Some(Len::MaxContent);
        element.style.inline_display = Some(true);
        element.style.grid_row = Some((
            crate::style::computed::Placement::Line(2),
            crate::style::computed::Placement::Span(3),
        ));
        element.style.margin.top = Some(Len::Px(10.0));
        for display in [Display::Grid, Display::InlineGrid, Display::GridLanes] {
            element.style.display = Some(display);
            assert!(eligible(&element));
            assert!(!crate::render::content_sized_wraps(&element));
        }
        element.style.max_height = Some(Len::MinContent);
        assert!(!eligible(&element));
    }

    #[test]
    fn specialized_roles_and_intrinsic_constraints_keep_their_existing_contracts() {
        let mut element = crate::layout::table::anon::anon_element("div", vec![]);
        element.style.width = Some(Len::FitContent);
        element.style.display = Some(Display::Block);
        for tag in ["table", "td", "img", "input", "canvas", "li"] {
            element.tag = tag.into();
            assert!(!eligible(&element), "{tag}");
        }
        element.tag = "div".into();
        element.style.max_width = Some(Len::MaxContent);
        assert!(!eligible(&element));
        element.style.max_width = None;
        element.style.contain_inline_size = Some(true);
        assert!(!eligible(&element));
        element.style.contain_inline_size = None;
        element.style.float = Some(1);
        assert!(!eligible(&element));
    }
}
