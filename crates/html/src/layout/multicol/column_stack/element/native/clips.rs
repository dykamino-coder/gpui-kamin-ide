//! Допуск native-команд повторяет границы содержимого прежнего фрагмента.

use super::*;
use crate::tolerance::COORD_EPS;

impl ColumnStack {
    pub(super) fn native_clip(
        &self,
        rect: Bounds<Pixels>,
        fragment: &Frag,
        plan: &[Frag],
        viewport: f32,
    ) -> Bounds<Pixels> {
        let kid = &self.children[fragment.kid];
        let spill = if kid.nested_cols { 0. } else { viewport };
        let continued = plan
            .iter()
            .any(|g| g.kid == fragment.kid && g.copy > fragment.copy);
        let split = plan.iter().filter(|g| g.kid == fragment.kid).count() > 1
            || (fragment.from + fragment.h < kid.h - COORD_EPS
                && kid.cuts.iter().any(|&(_, from)| from >= kid.h - COORD_EPS));
        let (head, tail) = kid.clone_dec.unwrap_or_default();
        let constrained = kid.nested_cols
            || kid.repeat.is_some()
            || kid.slack.is_some()
            || crate::layout::fragment::effect_fragment::active(&self.paint);
        let line = if self.rows.is_some_and(|r| r.wrap) {
            fragment.col / self.count
        } else {
            0
        };
        let height = self.lines_plan.borrow().get(line).map_or(0., |l| l.1);
        let (up, down) = overflow(
            split,
            kid.clone_dec.is_some(),
            constrained,
            continued,
            fragment,
            kid.over,
            height,
            spill,
        );
        geometry::slice(
            self.axis,
            rect,
            -spill,
            self.col_w.get() + spill * 2.,
            fragment.from + head - up,
            (fragment.h - head - tail).max(0.) + up + down,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn overflow(
    split: bool,
    cloned: bool,
    constrained: bool,
    continued: bool,
    f: &Frag,
    over: f32,
    height: f32,
    spill: f32,
) -> (f32, f32) {
    if cloned {
        return (0., 0.);
    }
    if !split {
        return (spill, spill);
    }
    if constrained {
        return (0., 0.);
    }
    (
        if f.copy == 0 && f.from <= COORD_EPS {
            spill
        } else {
            0.
        },
        if continued || f.from + f.h < over - COORD_EPS {
            0.
        } else {
            (height - f.y - f.h).max(0.)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_last_fragment_keeps_ink_only_to_the_column_end() {
        let f = Frag {
            kid: 0,
            col: 0,
            from: 40.,
            h: 20.,
            y: 10.,
            copy: 1,
            head: 0.,
            foot: 0.,
        };
        assert_eq!(
            overflow(true, false, false, false, &f, 60., 80., 400.),
            (0., 50.)
        );
        assert_eq!(
            overflow(true, false, false, true, &f, 60., 80., 400.),
            (0., 0.)
        );
        assert_eq!(
            overflow(true, false, true, false, &f, 60., 80., 400.),
            (0., 0.)
        );
        assert_eq!(
            overflow(false, false, false, false, &f, 60., 80., 400.),
            (400., 400.)
        );
        assert_eq!(
            overflow(false, true, false, false, &f, 60., 80., 400.),
            (0., 0.)
        );
    }
}
