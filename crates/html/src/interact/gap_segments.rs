//! Form ordered CSS gap decoration segments before extending their endpoints.

use super::{GapAxisRule, GapRun, subtract};

// CSS Gaps 1 §3.3: overlap-join reaches the far edge of the crossing
// decoration; main-direction junctions use half the crossing gap only.
fn inset_px(
    inset: crate::computed::GapInset,
    cw: f32,
    joins: bool,
    cross_w: f32,
    main_like: bool,
) -> f32 {
    use crate::computed::GapInset;
    use crate::value::Len;
    match inset {
        GapInset::Len(Len::Px(v)) => v,
        GapInset::Len(Len::Pct(k)) => k * cw,
        GapInset::Len(_) => 0.0,
        GapInset::OverlapJoin if joins => -(cw / 2.0) - if main_like { 0.0 } else { cross_w / 2.0 },
        GapInset::OverlapJoin => 0.0,
    }
}

pub(super) fn segments(
    run: &GapRun,
    rule: &GapAxisRule,
    main_like: bool,
    flip: bool,
) -> Vec<(f32, f32)> {
    let mut parts = vec![(run.r0, run.r1)];
    for &c in &run.hidden {
        parts = subtract(parts, c);
    }
    if rule.brk != 0 {
        for &c in &run.blocked {
            parts = subtract(parts, c);
        }
    }
    if rule.brk == 2 {
        for c in run.crossings.iter().filter(|c| c.breaks) {
            parts = subtract(parts, (c.lo, c.hi));
        }
    }
    // In RTL the physical low endpoint is the logical end (§3.3.1).
    let (lo_cap, lo_join, hi_cap, hi_join) = if flip { (1, 3, 0, 2) } else { (0, 2, 1, 3) };
    let mut out = vec![];
    for (s, e) in parts {
        let (s, s_cw, s_join, s_dw) = run.edge(s, true);
        let (e, e_cw, e_join, e_dw) = run.edge(e, false);
        // CSS Gaps 1 §3.1.2 pairs a start with a subsequent end before
        // applying insets. Removing invisible tracks can leave only a
        // junction, whose endpoints resolve in reverse order. Negative
        // insets must not turn that residual into an orphan decoration.
        // Equal positions can still describe a zero-length track segment.
        // Blink gap_decorations_painter.cc:377-380 validates the pair first.
        if e < s {
            continue;
        }
        let s2 = s + inset_px(
            rule.inset[if s_join { lo_join } else { lo_cap }],
            s_cw,
            s_join,
            s_dw,
            main_like,
        );
        let e2 = e - inset_px(
            rule.inset[if e_join { hi_join } else { hi_cap }],
            e_cw,
            e_join,
            e_dw,
            main_like,
        );
        if e2 - s2 > 0.05 {
            out.push((s2, e2));
        }
    }
    out
}
