//! Область сжатия циклических процентов в минимальном вкладе grid-элемента.
use super::*;
use crate::style_helpers::percent;
use crate::{Style, TaffyTree};

fn item(replaced: bool, compressible: bool) -> GridItem {
    let style: Style = Style {
        size: Size {
            width: Dimension::length(40.0),
            height: Dimension::length(40.0),
        },
        max_size: Size {
            width: percent(0.5),
            height: percent(0.5),
        },
        item_is_replaced: replaced,
        item_is_compressible_replaced: compressible,
        ..Style::DEFAULT
    };
    let span = Line {
        start: OriginZeroLine(0),
        end: OriginZeroLine(1),
    };
    GridItem::new_with_placement_style_and_order(
        NodeId::new(0),
        span,
        span,
        &style,
        AlignItems::STRETCH,
        AlignItems::STRETCH,
        0,
    )
}

#[test]
fn compression_uses_css_sizing_scope_in_both_axes() {
    for axis in [AbstractAxis::Inline, AbstractAxis::Block] {
        // CSS Sizing 3 включает сжимаемые controls независимо от классификации replaced.
        for (label, replaced, compressible) in [
            ("image", true, true),
            ("input-text", false, true),
            ("select", false, true),
            ("textarea", false, true),
            ("progress", false, true),
            ("meter", false, true),
            ("marquee", false, true),
            ("button", false, false),
            ("ordinary", false, false),
            ("image-implied-compression", true, false),
        ] {
            for basis in [None, Some(60.0), Some(80.0)] {
                let mut tree: TaffyTree<()> = TaffyTree::new();
                let mut child = item(replaced, compressible);
                let area = Size {
                    width: basis,
                    height: basis,
                };
                let actual =
                    child.minimum_contribution(&mut tree.as_layout_tree(), axis, &[], area, area);
                let expected = match basis {
                    None if replaced || compressible => 0.0,
                    Some(60.0) => 30.0,
                    _ => 40.0,
                };
                assert_eq!(
                    actual, expected,
                    "{label} {axis:?} replaced={replaced} compressible={compressible} basis={basis:?}"
                );
            }
        }
    }
}
