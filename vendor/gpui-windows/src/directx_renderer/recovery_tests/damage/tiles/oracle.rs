//! KaminIDE patch: the Compositing formulas the tile pass tests compare against,
//! in f64 over 8-bit premultiplied frames.

/// Separable blend function B(Cb, Cs) of mode `m` (Compositing 1 §9.1).
fn channel(m: u32, cb: f64, cs: f64) -> f64 {
    match m {
        1 => cb * cs,
        2 => cb + cs - cb * cs,
        3 => cb.min(cs),
        4 => cb.max(cs),
        5 => channel(8, cs, cb),
        6 if cb <= 0. => 0.,
        6 if cs >= 1. => 1.,
        6 => (cb / (1. - cs)).min(1.),
        7 if cb >= 1. => 1.,
        7 if cs <= 0. => 0.,
        7 => 1. - ((1. - cb) / cs).min(1.),
        8 if cs <= 0.5 => 2. * cs * cb,
        8 => 1. - 2. * (1. - cs) * (1. - cb),
        9 => {
            let d = if cb <= 0.25 { ((16. * cb - 12.) * cb + 4.) * cb } else { cb.sqrt() };
            if cs <= 0.5 { cb - (1. - 2. * cs) * cb * (1. - cb) } else { cb + (2. * cs - 1.) * (d - cb) }
        }
        10 => (cb - cs).abs(),
        11 => cb + cs - 2. * cb * cs,
        _ => cs,
    }
}

type C3 = [f64; 3];

/// Lum of Compositing 1 §9.2 (0.3 R + 0.59 G + 0.11 B). The frames are BGRA,
/// so `c` is (B, G, R).
fn lum(c: C3) -> f64 {
    0.11 * c[0] + 0.59 * c[1] + 0.3 * c[2]
}

fn set_lum(c: C3, l: f64) -> C3 {
    let d = l - lum(c);
    let c = c.map(|v| v + d);
    let (lo, hi, l) = (c[0].min(c[1]).min(c[2]), c[0].max(c[1]).max(c[2]), lum(c));
    let c = if lo < 0. { c.map(|v| l + (v - l) * l / (l - lo).max(1e-5)) } else { c };
    if hi > 1. { c.map(|v| l + (v - l) * (1. - l) / (hi - l).max(1e-5)) } else { c }
}

fn sat(c: C3) -> f64 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: C3, s: f64) -> C3 {
    let (lo, hi) = (c[0].min(c[1]).min(c[2]), c[0].max(c[1]).max(c[2]));
    if hi <= lo { [0.; 3] } else { c.map(|v| (v - lo) * s / (hi - lo)) }
}

/// Non-separable modes 12..=15 (Compositing 1 §9.2).
fn nonseparable(m: u32, cb: C3, cs: C3) -> C3 {
    match m {
        12 => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        13 => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        14 => set_lum(cs, lum(cb)),
        _ => set_lum(cb, lum(cs)),
    }
}

/// Premultiplied Compositing 1/2 result of mode `m` for one pixel (0..1).
fn mix(m: u32, s: [f64; 4], d: [f64; 4]) -> [f64; 4] {
    let (sa, da) = (s[3], d[3]);
    match m {
        0 => return [0, 1, 2, 3].map(|k| s[k] + d[k] * (1. - sa)),
        16 => return [0, 1, 2, 3].map(|k| (s[k] + d[k]).min(1.)),
        _ => {}
    }
    let un = |c: [f64; 4], a: f64| -> C3 { [0, 1, 2].map(|k| if a > 0. { c[k] / a } else { 0. }) };
    let (cs, cb) = (un(s, sa), un(d, da));
    let mixed = if m >= 12 { nonseparable(m, cb, cs) } else { [0, 1, 2].map(|k| channel(m, cb[k], cs[k])) };
    let mut o = [0.; 4];
    for k in 0..3 {
        o[k] = (1. - da) * sa * cs[k] + da * sa * mixed[k] + (1. - sa) * da * cb[k];
    }
    o[3] = sa + da * (1. - sa);
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

/// The tile pass's frame against the levels' frame and both against the
/// formula: the pass equals the formula (exactly, or within one code) and
/// differs from the levels by at most one code.
pub(super) fn compare(name: &str, got: &[u8], levels: &[u8], want: &[u8], exact: bool) {
    let (big, one) = off(got, levels);
    let (gb, go) = off(got, want);
    let (lb, lo) = off(levels, want);
    eprintln!(
        "{name}: tiles vs levels {one} channels off by one; vs formula: tiles {gb}/{go}, levels {lb}/{lo} (>1 / =1)"
    );
    assert_eq!(big, 0, "{name}: tiles off the levels by more than one");
    let off_by_one = if exact { go } else { 0 };
    assert_eq!((gb, off_by_one), (0, 0), "{name}: tiles off the formula");
}
