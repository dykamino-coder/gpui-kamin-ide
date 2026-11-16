//! KaminIDE patch: the Compositing formulas the tile pass tests compare against,
//! in f64 over 8-bit premultiplied frames.

/// Premultiplied Compositing 1/2 result of mode `m` for one pixel (0..1).
fn mix(m: u32, s: [f64; 4], d: [f64; 4]) -> [f64; 4] {
    let (sa, da) = (s[3], d[3]);
    let a = if m == 16 { (sa + da).min(1.) } else { sa + da * (1. - sa) };
    let mut o = [0.; 4];
    for k in 0..3 {
        let (cs, cd) = (s[k], d[k]);
        o[k] = match m {
            0 => cs + cd * (1. - sa),
            1 => cs * (1. - da) + cd * (1. - sa) + cs * cd,
            2 => cs + cd - cs * cd,
            11 => cs + cd - 2. * cs * cd,
            16 => (cs + cd).min(1.),
            _ => {
                let (us, ud) = (if sa > 0. { cs / sa } else { 0. }, if da > 0. { cd / da } else { 0. });
                // difference (10): the copy path's shader formula.
                let bl = (ud - us).abs();
                (1. - da) * sa * us + da * sa * bl + (1. - sa) * da * ud
            }
        };
    }
    o[3] = a;
    o
}

/// Expected frame: `images[k]` (group k alone at opacity 1: its retained
/// image, premultiplied) faded by its opacity and blended in order over `dst`
/// with each group's mode, rounding to 8 bits after every step as the target does.
pub(super) fn expect(mut dst: Vec<u8>, images: &[Vec<u8>], modes: &[u32], opacity: &[f32]) -> Vec<u8> {
    for ((img, &m), &op) in images.iter().zip(modes).zip(opacity) {
        for (d, s) in dst.chunks_mut(4).zip(img.chunks(4)) {
            let f = |v: &[u8]| [v[0], v[1], v[2], v[3]].map(|c| c as f64 / 255.);
            let s = f(s).map(|c| c * op as f64);
            let o = mix(m, s, f(d));
            for k in 0..4 {
                d[k] = (o[k].clamp(0., 1.) * 255.).round() as u8;
            }
        }
    }
    dst
}

/// (channels off by more than one, channels off by exactly one).
pub(super) fn off(a: &[u8], b: &[u8]) -> (usize, usize) {
    let d = |x: &u8, y: &u8| x.abs_diff(*y);
    let big = a.iter().zip(b).filter(|(x, y)| d(x, y) > 1).count();
    let one = a.iter().zip(b).filter(|(x, y)| d(x, y) == 1).count();
    (big, one)
}
