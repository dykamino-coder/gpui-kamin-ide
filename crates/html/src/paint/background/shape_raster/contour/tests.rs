//! Геометрия контура против чисел эталонов WPT (corner-shape-*-ref).

use super::*;

fn boxed(k: f32, r: f32) -> Shaped {
    Shaped {
        rect: (0.0, 0.0, 100.0, 100.0),
        radii: [(r, r); 4],
        k: [k; 4],
    }
}

fn near(a: P, b: P) -> bool {
    (a.0 - b.0).abs() < 2e-3 && (a.1 - b.1).abs() < 2e-3
}

#[test]
fn squircle_outer_matches_blink_bezier() {
    let p = outer(&boxed(2.0, 25.0));
    assert!(near(p.start, (75.0, 0.0)));
    let Seg::Cubic(a, b, c) = p.segs[1] else {
        panic!("{:?}", p.segs[1])
    };
    assert!(near(a, (86.001, 0.0)), "{a:?}");
    assert!(near(b, (92.009, -0.036)), "{b:?}");
    assert!(near(c, (96.022, 3.978)), "{c:?}");
}

#[test]
fn bevel_outer_and_inner_are_octagons() {
    let s = boxed(0.0, 25.0);
    let o = outer(&s).polygon().unwrap();
    assert_eq!(o.len(), 8);
    assert!(near(o[0], (75.0, 0.0)) && near(o[1], (100.0, 25.0)));
    let Inner::Polygon(i) = inner(&s, [5.0; 4]) else {
        panic!("curved")
    };
    assert_eq!(i.len(), 8, "{i:?}");
    assert!(i.iter().any(|p| near(*p, (72.929, 5.0))), "{i:?}");
    assert!(i.iter().any(|p| near(*p, (95.0, 27.071))), "{i:?}");
}

#[test]
fn notch_inner_keeps_the_offset_concave_edges() {
    let s = boxed(f32::NEG_INFINITY, 25.0);
    let Inner::Polygon(i) = inner(&s, [5.0; 4]) else {
        panic!("curved")
    };
    // The 5px border extends the 25px notch to 30px along the inner edge.
    assert!(i.iter().any(|p| near(*p, (70.0, 5.0))), "{i:?}");
    assert!(i.iter().any(|p| near(*p, (70.0, 30.0))), "{i:?}");
}

#[test]
fn curved_inner_is_a_clipped_rectangle() {
    let s = boxed(2.0, 25.0);
    match inner(&s, [5.0; 4]) {
        Inner::Clipped { rect, clips } => {
            assert_eq!(rect, (5.0, 5.0, 90.0, 90.0));
            assert_eq!(clips.len(), 4);
            // Squircle start normal is perpendicular to the edge.
            assert!(near(clips[0].start, (75.0, 5.0)), "{:?}", clips[0].start);
        }
        Inner::Polygon(p) => panic!("{p:?}"),
    }
}

#[test]
fn round_corners_without_radius_stay_square() {
    let s = Shaped {
        rect: (0.0, 0.0, 50.0, 40.0),
        radii: [(0.0, 0.0); 4],
        k: [0.0; 4],
    };
    let o = outer(&s).polygon().unwrap();
    assert_eq!(o, vec![(50.0, 0.0), (50.0, 40.0), (0.0, 40.0), (0.0, 0.0)]);
}

#[test]
fn mixed_corners_draw_only_the_scoop_as_curves() {
    let s = Shaped {
        rect: (0.0, 0.0, 200.0, 200.0),
        radii: [(100.0, 100.0); 4],
        k: [0.0, -1.0, f32::NEG_INFINITY, f32::INFINITY],
    };
    let cubics = |segs: &[Seg]| segs.iter().filter(|s| matches!(s, Seg::Cubic(..))).count();
    // Scoop: the round (conic) quarter; bevel, notch and square are lines.
    assert_eq!(cubics(&outer(&s).segs), 1);
    let Inner::Clipped { clips, .. } = inner(&s, [20.0; 4]) else {
        panic!("polygon")
    };
    // The square corner is not clipped; the notch clip stays polygonal.
    assert_eq!(clips.len(), 3);
    assert_eq!(cubics(&clips[1].segs), 0);
}
