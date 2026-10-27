//! Неразрывность начальной декорации и примыкающего монолитного содержимого.

use super::{FlowGeometry, at, tree};

#[test]
fn adjoining_monolith_carries_the_block_start_decoration() {
    for gap in [0., 10.] {
        let (b, ids) = tree(
            "<div style='border:3px solid'><div style='height:350px;contain:size'></div></div>",
        );
        let laid = at(&[(b.id, 0., 356. + gap), (ids[0], 3. + gap, 353. + gap)]);
        let m = b
            .breaks(356. + gap, &laid)
            .finish(FlowGeometry::default(), 356. + gap);
        assert_eq!(m.barriers.contains(&(0., 353.)), gap == 0.);
        assert_eq!(m.solid.contains(&(0., 353.)), gap == 0.);
        if gap == 0. {
            assert!(!m.solid.iter().any(|p| p.0 == 3.));
            let k = crate::layout::fragment::types::Kid {
                h: 356.,
                over: 356.,
                cuts: m.cuts,
                solid: m.solid,
                ..Default::default()
            };
            let (_, _, plan) = crate::layout::fragment::plan::fill_at(
                &[k],
                &|_| 100.,
                crate::layout::fragment::plan::Space::pages(8),
            );
            assert_eq!(
                plan.iter()
                    .map(|f| (f.col, f.from, f.h))
                    .collect::<Vec<_>>(),
                [
                    (0, 0., 100.),
                    (1, 100., 100.),
                    (2, 200., 100.),
                    (3, 300., 56.)
                ]
            );
        }
    }
}

#[test]
fn nested_adjoining_decorations_follow_the_first_monolith() {
    let (b, ids) = tree(
        "<div style='border:3px solid'><div style='border:2px solid'><div style='height:350px;contain:size'></div></div></div>",
    );
    let laid = at(&[(b.id, 0., 360.), (ids[0], 3., 357.), (ids[1], 5., 355.)]);
    let m = b.breaks(360., &laid).finish(FlowGeometry::default(), 360.);
    assert!(m.barriers.contains(&(0., 355.)));
    assert!(m.solid.contains(&(0., 355.)));
}
