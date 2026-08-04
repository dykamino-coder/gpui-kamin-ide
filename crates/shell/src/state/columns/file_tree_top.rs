//! Верх файловой колонки: дерево файлов.
//!
//! Ветка `top_content` вынесена как есть (`plan/100-refactor-250.md`).

use crate::state::model::RootView;
use gpui::{AnyElement, Context, Window};
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn file_top_tree(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
        p: &'static Palette,
    ) -> AnyElement {
        // Файл не выбран → `PanelPlaceholder label="File" slot="center"`
        // (`FilePanel.tsx:120-126`). Наш прежний `.empty` из
        // `FileViewer` (codicon-file 36 + «Ctrl+P») здесь недостижим и
        // в оригинале: FilePanel до FileViewer не доходит (ревью ц.13).
        crate::ui::panel_placeholder::panel_placeholder(
            "File",
            "Click a file in any panel, or drag-and-drop one from outside",
            crate::ui::panel_placeholder::SlotIcon::Center,
            p,
        )
    }
}
