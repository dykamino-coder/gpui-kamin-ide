//! Native fills avoid discarded bitmaps; raster backgrounds still acquire one.

use crate::work::{Counter, Phase, Session, enter};

fn paint(background: &str) -> u64 {
    let session = Session::begin();
    let phase = enter(Phase::Paint);
    let html = format!(
        "<style>html,body{{margin:0}}div{{width:87px;height:53px;background:{background}}}</style><div></div>"
    );
    super::rendered::quads(&html, 100., 80.);
    drop(phase);
    session.finish().0[Phase::Paint as usize][Counter::GradientRasters as usize]
}

#[test]
fn native_gradient_does_not_allocate_a_discarded_bitmap() {
    assert_eq!(paint("linear-gradient(23deg, #172b4d, #c15a2e)"), 0);
}

#[test]
fn radial_and_conic_backgrounds_still_acquire_a_bitmap() {
    assert!(paint("radial-gradient(ellipse, #2367a9, #d94f32)") > 0);
    assert!(paint("conic-gradient(from 17deg, #486bad, #c73a51, #486bad)") > 0);
}
