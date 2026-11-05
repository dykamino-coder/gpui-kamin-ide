//! Проверки фрагментов предков спаннера.

use super::*;
use crate::layout::multicol::spanner::SpannerMetrics;

#[test]
fn owned_parent_remainder_is_not_clamped_again_by_original_constraints() {
    fn host(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => host(&e.children),
            _ => None,
        })
    }
    for clone in [false, true] {
        let tree = crate::document::parsed::parse(
            &format!(
                "<div style='height:100px; min-height:150px; max-height:180px; box-sizing:border-box; box-decoration-break:{}'><p>before</p><h2 style='column-span:all'>span</h2><p>after</p></div>",
                if clone { "clone" } else { "slice" }
            ),
            "",
        );
        let e = host(&tree).expect("spanner parent");
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800., 600.),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        let parts = spanner_parts(
            &[Node::Element(e.clone())],
            &BoxStyle::default(),
            &opts,
            200.,
            &mut |c, _, _| SpannerMetrics {
                block_size: if has_deep_spanner(c) { 115. } else { 30. },
                content_width: 200.,
                content_size: Some(100.),
                start_edge: 10.,
                end_edge: 5.,
            },
        );
        let fragments: Vec<&Element> = parts
            .iter()
            .filter_map(|part| match part {
                SpanPart::Body(nodes) => nodes.iter().find_map(|node| match node {
                    Node::Element(e) if e.tag == "div" => Some(e),
                    _ => None,
                }),
                SpanPart::Span(_) => None,
            })
            .collect();
        assert_eq!(fragments.len(), 2);
        let dimensions = |f: &Element| {
            crate::layout::block::sizing::BoxDimensions::for_element(f, &f.style.used())
        };
        assert_eq!(dimensions(fragments[0]).preferred[1], None);
        assert_eq!(
            dimensions(fragments[1]).preferred[1],
            Some(Len::Px(if clone { 85. } else { 80. }))
        );
        for f in fragments {
            assert_eq!(dimensions(f).minimum[1], None);
            assert_eq!(dimensions(f).maximum[1], None);
            assert!(!dimensions(f).border_box);
            assert_eq!(
                dimensions(f).preferred[1],
                f.style.used().reset_box_layout.height
            );
            assert_eq!(
                f.style.used().reset_box_layout.min_height,
                e.style.used().reset_box_layout.min_height
            );
            assert_eq!(
                f.style.used().reset_box_layout.max_height,
                e.style.used().reset_box_layout.max_height
            );
            assert_eq!(
                f.style.used().reset_border.border_box,
                e.style.used().reset_border.border_box
            );
        }
        assert_eq!(e.style.used().reset_box_layout.height, Some(Len::Px(100.)));
        assert_eq!(
            e.style.used().reset_box_layout.min_height,
            Some(Len::Px(150.))
        );
        assert_eq!(
            e.style.used().reset_box_layout.max_height,
            Some(Len::Px(180.))
        );
        assert_eq!(e.style.used().reset_border.border_box, Some(true));
    }
}
