//! Доступ к стору дерева файлов.
//!
//! Стор — отдельная сущность (`Entity<TreeState>`), чтобы панель дерева
//! перерисовывалась только на его `notify`, а не с каждым кадром окна
//! (`plan/102-components.md`). Правило: любое изменение идёт через
//! [`RootView::tree_mut`], который сам зовёт `notify` — забыть его нельзя.

use crate::state::model::RootView;
use crate::ui::file_list::TreeState;
use gpui::App;

impl RootView {
    /// Прочитать состояние дерева.
    pub(crate) fn tree<'a>(&self, cx: &'a App) -> &'a TreeState {
        self.tree_store.read(cx)
    }

    /// Изменить состояние дерева; панель узнает об этом сама.
    ///
    /// Замыкание отдаёт «изменилось ли что-то» — `notify` зовём только тогда.
    /// Иначе на холостых вызовах (тот же флаг, удаление отсутствующего пути,
    /// повторный выбор той же строки) панель перерисовывалась зря.
    pub(crate) fn tree_mut<R: Changed>(
        &self,
        cx: &mut App,
        f: impl FnOnce(&mut TreeState) -> R,
    ) -> R {
        self.tree_store.update(cx, |tree, cx| {
            let out = f(tree);
            if out.changed() {
                cx.notify();
            }
            out
        })
    }
}

/// Изменило ли замыкание состояние. `bool` понимается буквально (это результат
/// `insert`/`remove`), всё остальное считается изменением.
pub(crate) trait Changed {
    fn changed(&self) -> bool;
}

impl Changed for bool {
    fn changed(&self) -> bool {
        *self
    }
}

impl<T> Changed for Option<T> {
    fn changed(&self) -> bool {
        true
    }
}

impl Changed for () {
    fn changed(&self) -> bool {
        true
    }
}

impl Changed for Vec<String> {
    fn changed(&self) -> bool {
        true
    }
}
