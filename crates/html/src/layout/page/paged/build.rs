//! Сборка детей стопки страниц: признак «строятся листы» и слой `position:
//! fixed` текущего кида — часть `RenderOpts` (`FragCx::paged`), а не
//! состояние потока.

use crate::layout::options::RenderOpts;
use gpui::AnyElement;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Строится стопка страниц. Абсолют корня БЕЗ заданных сторон тоже уходит в
/// слой ICB: на месте он остаётся внутри обёртки кида и режется маской её
/// фрагмента (`monolithic-overflow-013`: четыре пустых листа). Экрану это не
/// нужно — там статическая позиция и есть место в потоке.
#[derive(Default)]
pub(crate) struct PagedBuild {
    /// Сборка идёт. После неё (`end`) настройки, захваченные отложенными
    /// построителями (лента прокрутки строит поддерево в своей раскладке),
    /// листов уже не видят — как и прежде.
    open: Cell<bool>,
    /// Слой `position: fixed` текущей сборки кида — по копии на лист без
    /// сдвига (`PageStack::fixed`). В слой ICB ему нельзя: тот поднимается на
    /// `p` page area и показывает фиксированный только на первом листе
    /// (`fixedpos-007..009`).
    fixed: RefCell<Vec<AnyElement>>,
}

impl PagedBuild {
    pub(crate) fn begin() -> Rc<Self> {
        let build = Rc::new(PagedBuild::default());
        build.open.set(true);
        build
    }

    pub(crate) fn end(&self) {
        self.open.set(false);
    }

    pub(crate) fn push_fixed(&self, els: Vec<AnyElement>) {
        self.fixed.borrow_mut().extend(els);
    }
}

impl std::fmt::Debug for PagedBuild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PagedBuild")
            .field("open", &self.open.get())
            .finish()
    }
}

impl RenderOpts {
    /// Идёт сборка детей стопки страниц.
    pub(crate) fn paged_build(&self) -> Option<&PagedBuild> {
        self.frag.paged.as_deref().filter(|p| p.open.get())
    }

    /// Начать сборку кида стопки страниц: слой `fixed` пуст.
    pub(crate) fn clear_paged_fixed(&self) {
        if let Some(paged) = self.paged_build() {
            paged.fixed.borrow_mut().clear();
        }
    }

    /// Забрать слой `fixed` собранного кида (вне сборки листов — пусто).
    pub(crate) fn take_paged_fixed(&self) -> Vec<AnyElement> {
        self.paged_build()
            .map(|paged| std::mem::take(&mut *paged.fixed.borrow_mut()))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::PagedBuild;
    use crate::layout::options::RenderOpts;

    #[test]
    fn options_see_the_paged_build_only_while_it_runs() {
        let paged = PagedBuild::begin();
        let mut opts = RenderOpts {
            text: Default::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        };
        assert!(opts.paged_build().is_none());
        opts.frag.paged = Some(paged.clone());
        // A builder deferred past the build (scroll area content) keeps a clone.
        let deferred = opts.clone();
        assert!(opts.paged_build().is_some());
        paged.end();
        assert!(deferred.paged_build().is_none());
    }
}
