//! Вход clamp, сохраняемый отложенными детьми и форкаемый для каждого измерения.

use super::ClampBuild;
use crate::layout::cx::LayoutCx;

pub(crate) struct DeferredClamp(ClampBuild);

impl DeferredClamp {
    pub(crate) fn capture(cx: &LayoutCx) -> Option<Self> {
        cx.clamp_build.context?;
        Some(Self(cx.clamp_build.snapshot()))
    }

    pub(crate) fn build(&self) -> ClampBuild {
        self.0.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::clamp::{clamp_context, clamp_next_seq};

    #[test]
    fn repeated_builds_reuse_numbers_without_consuming_outer_scope() {
        let mut cx = LayoutCx::default();
        cx.clamp_build = cx.clamp_build.container(900);
        assert_eq!(clamp_next_seq(900, &cx), 0);
        let deferred = DeferredClamp::capture(&cx).expect("active container");
        assert_eq!(clamp_next_seq(900, &cx), 1);
        assert_eq!(clamp_next_seq(900, &cx), 2);
        let mut outer = cx.clone();
        outer.clamp_build = outer.clamp_build.container(901);
        assert_eq!(clamp_next_seq(901, &outer), 0);
        for _ in 0..2 {
            let mut probe = outer.clone();
            probe.clamp_build = deferred.build();
            assert_eq!(clamp_context(&probe), Some((900, false)));
            assert_eq!(clamp_next_seq(900, &probe), 1);
            assert_eq!(clamp_next_seq(900, &probe), 2);
        }
        assert_eq!(clamp_context(&outer), Some((901, false)));
        assert_eq!(clamp_next_seq(901, &outer), 1);
        assert_eq!(super::super::line_state::clamp_peek_seq(900, &cx), 3);
    }

    #[test]
    fn deferred_build_retains_bfc_boundary_without_changing_parent() {
        let mut parent = LayoutCx::default();
        parent.clamp_build = parent.clamp_build.container(902);
        let mut inner = parent.clone();
        inner.clamp_build = inner.clamp_build.bfc();
        let deferred = DeferredClamp::capture(&inner).expect("active container");
        inner.clamp_build = deferred.build();
        assert_eq!(clamp_context(&inner), Some((902, true)));
        assert_eq!(clamp_context(&parent), Some((902, false)));
    }
}
