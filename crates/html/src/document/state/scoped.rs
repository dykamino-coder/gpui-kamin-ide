//! Обёртка поддерева документа: держит состояние документа живым и отмечает
//! его покрашенным — граница области кадра арен (`DocState::begin_frame`).
//!
//! Сборка кадра (`render`) возвращает элементы, а раскладка, подготовка и
//! краска идут позже; записи арен кадра читаются до краски, и только после
//! неё следующая сборка может их сбросить. Сама обёртка — `gpui::ContextScope`:
//! узел раскладки — ребёнок, а порядок краски (`PaintLast`, шаг 8
//! приложения E) она не трогает.

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
            Box::new(())
        });
        ContextScope::new(enter, child).into_any_element()
    }
}
