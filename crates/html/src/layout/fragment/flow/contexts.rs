//! Адреса IFC принадлежат контексту: одинаковый ordinal внутри двух float не совпадает.
use super::{FlowMeasure, inline::Inline};
use std::rc::Rc;

impl FlowMeasure {
    pub(crate) fn context(&self, key: usize) -> Rc<Self> {
        self.contexts
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| {
                // У независимого BFC свои строки и собственный resume token.
                // Запрет экспорта строк родителя не отключает внутреннюю меру.
                Rc::new(Self::for_lines())
            })
            .clone()
    }

    pub(crate) fn context_path(&self, path: &[usize]) -> Option<Rc<Self>> {
        let (&first, rest) = path.split_first()?;
        let mut flow = self.context(first);
        for &key in rest {
            flow = flow.context(key);
        }
        Some(flow)
    }

    pub(crate) fn resume_paragraphs(&self, paragraphs: &[Inline], from: f32) {
        self.clear_contexts();
        let mut groups = std::collections::BTreeMap::<Vec<usize>, Vec<super::ContentLine>>::new();
        for p in paragraphs {
            groups
                .entry(p.scope.clone())
                .or_default()
                .extend_from_slice(&p.lines);
        }
        for (path, lines) in groups {
            if let Some(flow) = self.context_path(&path) {
                flow.resume_lines(&lines, from);
            } else {
                self.resume_lines(&lines, from);
            }
        }
    }

    fn clear_contexts(&self) {
        self.resume_lines(&[], 0.0);
        *self.float_resume.borrow_mut() = None;
        for child in self.contexts.borrow().values() {
            child.clear_contexts();
        }
    }
}
