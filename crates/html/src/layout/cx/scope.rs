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
        stacking.deferred.depth = stacking.deferred.depth.enter(false, true);
        assert!(stacking.deferred.depth.inside());
        assert!(!stacking.deferred.depth.deferred());
        let mut deferred = stacking.clone();
        deferred.deferred.depth = deferred.deferred.depth.enter(true, false);
        assert!(deferred.deferred.depth.inside());
        assert!(deferred.deferred.depth.deferred());
        assert!(!outer.deferred.depth.inside());
        assert!(!stacking.deferred.depth.deferred());

        let captured = deferred;
        let lazy = move || captured.deferred.depth;
        let mut sibling = outer;
        sibling.deferred.depth = sibling.deferred.depth.enter(false, false);
        assert!(!sibling.deferred.depth.inside());
        assert!(lazy().deferred());
        assert!(!LayoutCx::default().deferred.depth.inside());
    }
}
