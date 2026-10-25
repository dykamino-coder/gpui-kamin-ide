//! Нижние края и запреты разрыва флоатов в анонимной группе страниц.
use super::*;
use crate::layout::fragment::flow::{FloatGeometry, FlowGeometry};
use crate::layout::fragment::plan::{Measurement, Space, fill_resumed};

#[test]
fn anonymous_float_group_keeps_whole_floats_and_reserves_their_extent() {
    let ranges = [(18.0, 114.0), (114.0, 210.0), (210.0, 306.0)];
    let flow = FlowGeometry {
        floats: ranges
            .iter()
            .enumerate()
            .map(|(id, &(top, bottom))| FloatGeometry {
                id: id as u64,
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
            lines: vec![crate::layout::fragment::flow::ContentLine {
                paragraph: 0,
                byte: 0,
                state: crate::layout::fragment::line_shape::line_state("X", 0, 0, false).unwrap(),
                top: 0.0,
                bottom: 18.0,
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
