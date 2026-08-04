//! Перетаскивание файла: полезная нагрузка и призрак под курсором.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use gpui::prelude::*;
use gpui::{div, px};
use kamin_metrics as m;

/// Внутренний drag файла из дерева (drop: редактор=открыть, терминал=путь).
pub struct DraggedFile {
    /// `dragPaths()` (`file-selection.ts:67`): тащат выделенную строку из
    /// набора >1 — едет ВЕСЬ набор, иначе только сама строка.
    pub paths: Vec<String>,
}
/// Ghost перетаскиваемого файла: пилюля с именем у курсора.
pub struct FileDragGhost {
    pub(crate) name: String,
}

impl gpui::Render for FileDragGhost {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let p = kamin_theme::current_palette();
        div()
            .px(px(m::SPACE_2))
            .py(px(2.0))
            .rounded(px(m::RADIUS_SM))
            .bg(rgba(p.bg_surface))
            .border_1()
            .border_color({
                let mut c = rgba(p.text_primary);
                c.a = 0.15;
                c
            })
            .text_size(px(m::FS_XS))
            .text_color(rgba(p.text_primary))
            .child(self.name.clone())
    }
}
