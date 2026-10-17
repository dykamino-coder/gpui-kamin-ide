//! Обёртка поддерева документа: держит состояние документа живым и делает
//! его текущим на фазах GPUI этого поддерева.
//!
//! Сборка кадра (`render`) возвращает элементы, а раскладка, подготовка и
//! краска идут позже, уже вне области сборки: размещение по якорю
//! (`AnchorPlace`) и замеры на подготовке читают правила документа именно
//! там. Сама обёртка — `gpui::ContextScope`: узел раскладки — ребёнок, а
//! порядок краски (`PaintLast`, шаг 8 приложения E) она не трогает. Краска
//! отмечает документ покрашенным — граница области кадра арен.

use super::DocState;
use gpui::{AnyElement, ContextScope, IntoElement, ScopePhase};
use std::rc::Rc;

pub struct Scoped;

impl Scoped {
    /// Обернуть элемент документа `state`.
    pub fn wrap(state: &Rc<DocState>, child: AnyElement) -> AnyElement {
        let state = state.clone();
        let enter = Rc::new(move |phase: ScopePhase| -> Box<dyn std::any::Any> {
            if phase == ScopePhase::Paint {
                state.painted.set(true);
            }
            Box::new(state.enter_frame())
        });
        ContextScope::new(enter, child).into_any_element()
    }
}
