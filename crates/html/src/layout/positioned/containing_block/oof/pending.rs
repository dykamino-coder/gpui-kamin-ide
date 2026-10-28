//! Очередь зависимостей до конца ICB; остановка графа разрешается через CSS fallback.

use super::{Item, OofKid, OofQueue, Place};
mod context;
use context::Context;
type Output = Rc<RefCell<Vec<Painted>>>;
use gpui::{AnyElement, App, Bounds, ContentMask, Pixels, ResultExt, Window, point, px};
use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::{Rc, Weak},
};

struct Task {
    item: Item,
    bounds: Bounds<Pixels>,
    place: Place,
    context: Context,
    output: Output,
}

struct Destination {
    items: Weak<RefCell<VecDeque<Item>>>,
    bounds: Bounds<Pixels>,
    place: Place,
    context: Context,
    output: Weak<RefCell<Vec<Painted>>>,
}

#[derive(Default)]
struct Work {
    tasks: VecDeque<Task>,
    destinations: Vec<Destination>,
    layout: Option<std::rc::Rc<crate::layout::layout_state::LayoutState>>,
}

#[derive(Clone, Default)]
pub(in crate::layout::positioned::containing_block) struct Pending(Rc<RefCell<Work>>);

pub(super) struct Painted(pub(super) AnyElement, pub(super) ContentMask<Pixels>);
impl Painted {
    pub(super) fn paint(&mut self, window: &mut Window, cx: &mut App) {
        // Колонка может задать свою маску только на paint (css-break-3 §5.4).
        window.with_content_mask(Some(self.1), |window| self.0.paint(window, cx));
    }
}

impl Pending {
    pub(super) fn observe(&self, deps: &crate::layout::positioned::anchor::Dependencies) {
        if self.0.borrow().layout.is_none() {
            self.0.borrow_mut().layout = deps.layout();
        }
    }

    pub(super) fn context(&self, window: &Window) -> Context {
        Context::capture(self.0.borrow().layout.clone(), window)
    }

    pub(super) fn register(
        &self,
        queue: &OofQueue,
        bounds: Bounds<Pixels>,
        place: Place,
        context: Context,
        output: &Output,
    ) {
        self.0.borrow_mut().destinations.push(Destination {
            items: Rc::downgrade(&queue.items),
            bounds,
            place,
            context,
            output: Rc::downgrade(output),
        });
    }

    pub(super) fn push(
        &self,
        item: Item,
        bounds: Bounds<Pixels>,
        place: Place,
        context: Context,
        output: Output,
    ) {
        self.0.borrow_mut().tasks.push_back(Task {
            item,
            bounds,
            place,
            context,
            output,
        });
    }

    fn collect(&self) {
        let mut work = self.0.borrow_mut();
        let Work {
            tasks,
            destinations,
            ..
        } = &mut *work;
        for dest in destinations {
            if let (Some(items), Some(output)) = (dest.items.upgrade(), dest.output.upgrade()) {
                tasks.extend(items.take().into_iter().map(|item| Task {
                    item,
                    bounds: dest.bounds,
                    place: dest.place,
                    context: dest.context.clone(),
                    output: output.clone(),
                }));
            }
        }
    }

    pub(super) fn finish(&self, window: &mut Window, cx: &mut App) {
        loop {
            // Выбранная проба может породить OOF в уже пройденный слой.
            self.collect();
            let at = next(self.0.borrow().tasks.iter().map(|t| match &t.item.1 {
                OofKid::Waiting(deps, _) => (deps.sequence(), deps.ready()),
                _ => (0, true),
            }))
            .unwrap_or(0);
            let task = self.0.borrow_mut().tasks.remove(at);
            let Some(Task {
                item,
                bounds,
                place,
                context,
                output,
            }) = task
            else {
                break;
            };
            // Kahn: если готовых вершин больше нет, остаток содержит цикл
            // либо отсутствующий якорь. Ранняя вершина получает CSS fallback;
            // FnOnce исключает повторное построение этой коробки.
            context
                .enter(window, |window| {
                    let (mut root, paint) = super::host::build_one(item, bounds, place, window, cx);
                    root.prepaint_at(point(px(0.), px(0.)), window, cx);
                    match paint {
                        Some(paint) => paint.publish(root, context.mask),
                        None => output.borrow_mut().push(Painted(root, context.mask)),
                    }
                })
                .log_err();
        }
    }
}

// Готовые зависимости идут первыми. При остановке графа ранняя по дереву
// коробка получает fallback: она может открыть якорь следующей коробке.
fn next(states: impl Iterator<Item = (u32, bool)>) -> Option<usize> {
    states
        .enumerate()
        .min_by_key(|(i, (seq, ready))| (!ready, *seq, *i))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ready_dependencies_precede_stalled_nodes_and_cycles_use_tree_order() {
        assert_eq!(
            super::next([(2, false), (5, true), (4, true)].into_iter()),
            Some(2)
        );
        assert_eq!(super::next([(5, false), (2, false)].into_iter()), Some(1));
        assert_eq!(super::next([(2, false), (2, false)].into_iter()), Some(0));
        assert_eq!(super::next([].into_iter()), None);
    }
}
