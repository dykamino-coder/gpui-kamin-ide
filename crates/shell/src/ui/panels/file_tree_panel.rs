//! Панель дерева файлов как ОТДЕЛЬНЫЙ компонент (`plan/102-components.md`).
//!
//! Зачем сущность, а не функция: gpui умеет переигрывать раскладку и рисование
//! вью, который не звал `notify` — но только вью, не функции. Пока весь
//! интерфейс был одним `RootView`, дерево пересчитывалось каждый кадр окна.
//!
//! Правила компонента:
//! * состояние дерева читается из стора (`Entity<TreeState>`), копий нет;
//! * всё остальное приходит пропсами через [`FileTreePanel::set_props`], и
//!   `notify` зовётся ТОЛЬКО при реальном отличии — это и есть проверка
//!   изменений;
//! * подписка на стор ставится один раз в конструкторе.

use crate::host::events::{EdEvent, TreeEvent};
use crate::host_link::ShellEvent;
use crate::ui::file_list::TreeState;
use gpui::{Context, Entity, IntoElement, Render, Window};
use kamin_theme::ThemeKind;

/// Входные данные панели. Дешёвые на сравнение: строка корня, флаги, тема.
#[derive(Clone, PartialEq)]
pub struct FileTreeProps {
    /// Корень воркспейса; `None` — «папка не открыта».
    pub workspace: Option<gpui::SharedString>,
    /// Ширина панели в логических px — бюджет усечения имён. ПРОПСОМ, а не из
    /// probe-реестра прошлого кадра: у переигранного кадра prepaint не
    /// исполняется, и панель навсегда осталась бы с прежней шириной.
    pub width: f32,
    /// Кнопка Collapse ↔ Expand: текущее состояние тумблера.
    pub all_collapsed: bool,
    /// В редакторе есть открытый файл → кнопка Locate активна.
    pub has_editor_file: bool,
    pub theme: ThemeKind,
    /// Поколение оформления (`kamin_theme::generation`): палитра расширения и
    /// иконочная тема живут в статиках, и `ThemeKind` при их смене не меняется
    /// — без этого числа панель осталась бы в старых цветах и иконках.
    pub look: u64,
}

pub struct FileTreePanel {
    tree: Entity<TreeState>,
    tx: smol::channel::Sender<ShellEvent>,
    /// Слот, в котором стоит тул: свой probe-регион на панель. Не пропс —
    /// сущность и так заведена под конкретный слот.
    probe_id: &'static str,
    props: FileTreeProps,
}

impl FileTreePanel {
    pub fn new(
        tree: Entity<TreeState>,
        tx: smol::channel::Sender<ShellEvent>,
        probe_id: &'static str,
        props: FileTreeProps,
        cx: &mut Context<Self>,
    ) -> Self {
        // Изменился стор — перерисовать панель. Без подписки кэш вью держал бы
        // на экране прошлый листинг.
        cx.observe(&tree, |_, _, cx| cx.notify()).detach();
        Self {
            tree,
            tx,
            probe_id,
            props,
        }
    }

    /// Обновить пропсы. `notify` — только при отличии: иначе кэш кадра теряет
    /// смысл и панель пересчитывается каждый кадр окна, как раньше.
    pub fn set_props(&mut self, next: FileTreeProps, cx: &mut Context<Self>) {
        if self.props != next {
            self.props = next;
            cx.notify();
        }
    }
}

/// Рисует ли дерево сейчас спиннер (хедер «Indexing…», шеврон/корень в загрузке).
fn has_spinner(tree: &TreeState) -> bool {
    tree.indexing || !tree.loading.is_empty()
}

impl Render for FileTreePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.props.theme.palette();
        let tree = self.tree.read(cx);
        // Спиннеры дерева (индексация, загрузка папки) шагают по общему такту,
        // а кэшированный вью корень окна не перерисовывает: просим такт сами.
        if has_spinner(tree) {
            crate::ui::glyph_spinner::request_frame(cx.entity_id());
        }
        let send = |event: ShellEvent, tx: &smol::channel::Sender<ShellEvent>| {
            let _ = tx.try_send(event);
        };
        let tx = self.tx.clone();
        crate::ui::file_list::file_tree(
            self.props.workspace.as_ref().map(|w| w.as_ref()),
            tree,
            self.props.all_collapsed,
            self.props.has_editor_file,
            p,
            self.probe_id,
            self.props.width,
            {
                let tx = tx.clone();
                move |path| send(ShellEvent::Tree(TreeEvent::ToggleDir(path)), &tx)
            },
            {
                let tx = tx.clone();
                move |path| send(ShellEvent::Ed(EdEvent::OpenFile(path)), &tx)
            },
            {
                let tx = tx.clone();
                move |path, is_dir, x, y| {
                    send(
                        ShellEvent::Ed(EdEvent::OpenFileMenu(path, is_dir, x, y)),
                        &tx,
                    )
                }
            },
            {
                // Кнопка = ПОЛНЫЙ ремаунт (в отличие от watcher-а)
                let tx = tx.clone();
                move || send(ShellEvent::Tree(TreeEvent::RefreshTreeHard), &tx)
            },
            {
                let tx = tx.clone();
                move || send(ShellEvent::Tree(TreeEvent::CollapseTree), &tx)
            },
            {
                let tx = tx.clone();
                move || send(ShellEvent::Ed(EdEvent::LocateSelectedFile), &tx)
            },
            {
                let tx = tx.clone();
                move |dir| send(ShellEvent::Tree(TreeEvent::ShowMoreDir(dir)), &tx)
            },
            {
                let tx = tx.clone();
                move |path, ctrl, shift| {
                    send(
                        ShellEvent::Tree(TreeEvent::SelectTreeNode(path, ctrl, shift)),
                        &tx,
                    )
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_only_while_indexing_or_loading() {
        let mut tree = TreeState::default();
        assert!(!has_spinner(&tree));
        tree.loading.insert("C:\\proj".into());
        assert!(has_spinner(&tree));
        tree.loading.clear();
        tree.indexing = true;
        assert!(has_spinner(&tree));
    }
}
