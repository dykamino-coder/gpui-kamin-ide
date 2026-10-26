//! Нижние края и запреты разрыва флоатов в анонимной группе страниц.
use super::*;
use crate::layout::fragment::flow::{FloatGeometry, FlowGeometry};
use crate::layout::fragment::plan::{Measurement, Space, fill_resumed};

#[gpui::test]
fn page_margin_regression_keeps_wrapper_margin_out_of_overflow(cx: &mut gpui::TestAppContext) {
    use crate::layout::fragment::{
        flow::blocks::BlockBox,
        paginate::watch::{self, Watched},
    };
    use crate::layout::options::RenderOpts;
    use gpui::{IntoElement, ParentElement, Styled, div};
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(600.), px(300.)),
        |window, app| {
            for top in [0., 200.] {
                for clips_block in [false, true] {
                    let watched = Watched::for_blocks_axis(
                        BlockBox {
                            id: 7,
                            clips_block,
                            ..Default::default()
                        },
                        StackAxis::Horizontal,
                        true,
                    );
                    let mut opts = RenderOpts {
                        text: Default::default(),
                        viewport: (600., 300.),
                        normal_line_height: 1.2,
                        cx: Default::default(),
                        frag: Default::default(),
                    };
                    opts.frag.watch = Some(watched.clone());
                    let el =
                        watch::wrap(7, div().mt(px(top)).h(px(100.)).into_any_element(), &opts);
                    let mut kid = PageKid {
                        #[cfg(feature = "wpt-harness")]
                        probe: Default::default(),
                        el: div().flex().flex_col().child(el).into_any_element(),
                        regrow: None,
                        percentages: None,
                        leaf: None,
                        watched: Some(watched),
                        float_id: None,
                        float_extent: false,
                        repeat: None,
                        frags: Vec::new(),
                        monolith: false,
                        force_before: false,
                        renamed: false,
                        force_after: false,
                        shape: None,
                        page: String::new(),
                        mt: top,
                        mb: 0.,
                        inner_top: top,
                        reach: None,
                        end: Default::default(),
                    };
                    let (measured, _) = measure(
                        &mut kid,
                        StackAxis::Horizontal,
                        (600., 300.),
                        0,
                        window,
                        app,
                    );
                    assert_eq!(measured.h, 100.);
                    assert_eq!(measured.over, 100.);
                    assert_eq!(measured.mt, top);
                }
            }
            div()
        },
    );
}

#[test]
fn anonymous_float_group_keeps_whole_floats_and_reserves_their_extent() {
    let ranges = [(18.0, 114.0), (114.0, 210.0), (210.0, 306.0)];
    let flow = FlowGeometry {
        floats: ranges
            .iter()
            .enumerate()
            .map(|(id, &(top, bottom))| FloatGeometry {
                id: id as u64,
                anchor: top,
                top,
                bottom,
                margins: [0.0; 2],
                first_break: None,
            })
            .collect(),
        cuts: ranges.iter().map(|&(_, bottom)| (bottom, bottom)).collect(),
        solid: ranges.to_vec(),
        ..Default::default()
    };
    let h = float_extent(&flow, 0.0, 18.0);
    assert_eq!(h, 306.0);
    let (mut cuts, mut solid) = (Vec::new(), Vec::new());
    let mut inline = crate::layout::fragment::flow::inline::Geometry::default();
    inline
        .paragraphs
        .push(crate::layout::fragment::flow::inline::Inline {
            scope: Vec::new(),
            lines: vec![crate::layout::fragment::flow::ContentLine {
                paragraph: 0,
                byte: 0,
                state: crate::layout::fragment::line_shape::line_state("X", 0, 0, false).unwrap(),
                top: 0.0,
                bottom: 18.0,
                prefix: 0.0,
                consumed_before: 0.0,
            }],
            orphans: 2,
            widows: 2,
        });
    anonymous_breaks(flow, &mut cuts, &mut solid, &mut inline);
    assert_eq!(inline.barriers, ranges);
    let (_, _, plan) = fill_resumed(
        &[Kid {
            h,
            cuts,
            solid,
            ..Default::default()
        }],
        &|_| crate::layout::fragment::token::Space {
            block_size: Some(192.0),
            ..crate::layout::fragment::token::Space::pages()
        },
        Space::pages(8),
        &|_, _| None,
        &mut |_| Measurement {
            inline: inline.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        plan.iter()
            .map(|f| (f.col, f.from, f.h))
            .collect::<Vec<_>>(),
        [(0, 0.0, 114.0), (1, 114.0, 192.0)]
    );
}
