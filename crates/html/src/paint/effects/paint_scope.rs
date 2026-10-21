//! Неизменяемые границы покраски, которые `LayoutCx` передаёт поддереву коробки: контекст
//! наложения лишь ограничивает порядок покраски (CSS 2 Приложение E), а отложенная отрисовка
//! GPUI вдобавок ограничивает перенос позиционированных потомков (CSS 2 §10.1, CSS
//! Containment 2 §3.2). Значение копируется, поэтому ленивое поддерево хранит обе границы.

#[derive(Clone, Copy, Default)]
pub(crate) struct Depth {
    paint: usize,
    deferred: usize,
}

impl Depth {
    pub(crate) fn inside(self) -> bool {
        self.paint > 0
    }

    pub(crate) fn deferred(self) -> bool {
        self.deferred > 0
    }

    pub(crate) fn enter(self, deferred: bool, stacking: bool) -> Self {
        // CSS2 Appendix E limits paint ordering at a stacking context;
        // only deferred GPUI drawing also limits geometry relocation.
        Self {
            paint: self.paint + usize::from(deferred || stacking),
            deferred: self.deferred + usize::from(deferred),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::layout::cx::LayoutCx;

    #[test]
    fn stacking_and_deferred_boundaries_are_independent() {
        let outer = LayoutCx::default();
        let mut stacking = outer.clone();
        stacking.paint_depth = stacking.paint_depth.enter(false, true);
        assert!(stacking.paint_depth.inside());
        assert!(!stacking.paint_depth.deferred());
        let mut deferred = stacking.clone();
        deferred.paint_depth = deferred.paint_depth.enter(true, false);
        assert!(deferred.paint_depth.inside());
        assert!(deferred.paint_depth.deferred());
        assert!(!outer.paint_depth.inside());
        assert!(!stacking.paint_depth.deferred());

        let captured = deferred;
        let lazy = move || captured.paint_depth;
        let mut sibling = outer;
        sibling.paint_depth = sibling.paint_depth.enter(false, false);
        assert!(!sibling.paint_depth.inside());
        assert!(lazy().deferred());
        assert!(!LayoutCx::default().paint_depth.inside());
    }
}
