//! Тело contributed-вью с `TreeDataProvider` (TreeViewBody 1:1).
//! Дети тянутся лениво с хоста (`kamin:tree:getChildren`): корень при первом
//! показе, уровень узла — при первом раскрытии. Стили строк общие с
//! файловым деревом (`FileTreeView.module.css`).
//!
//! Drag-and-drop (`TreeDragAndDropController`) — на gpui-драге: строки
//! перетаскиваются, только когда вью зарегистрировала контроллер
//! (`kamin:tree:dnd`), хост получает `handleDrag`/`handleDrop`.

use crate::ui::ctree::level::level;
pub use crate::ui::ctree::model::TreeDragGhost;
pub use crate::ui::ctree::types::{TreeMeta, TreeNodeDto, TreeViewState};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::rgba;
use crate::host_link::ShellEvent;

pub(crate) const CHEVRON_DOWN: &str = "\u{eab4}";
pub(crate) const CHEVRON_RIGHT: &str = "\u{eab6}";
pub(crate) const CHECK: &str = "\u{eab2}";
pub(crate) const CIRCLE_OUTLINE: &str = "\u{eabc}";
pub(crate) const FOLDER: &str = "\u{ea83}";

/// Кап детей на уровень (TREE_CHILD_CAP оригинала).
pub(crate) const TREE_CHILD_CAP: usize = 100;
/// `indentPx(depth) = depth*12 + 8`.
pub(crate) const INDENT_PX: f32 = 12.0;
pub(crate) const BASE_INDENT_PX: f32 = 8.0;

impl gpui::Render for TreeDragGhost {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let p = kamin_theme::current_palette();
        div()
            .px(px(m::SPACE_2))
            .py(px(2.0))
            .rounded(px(m::RADIUS_XS))
            .bg(rgba(p.bg_surface))
            .text_size(px(m::FS_SM))
            .text_color(rgba(p.text_primary))
            .child(SharedString::from(self.label.clone()))
    }
}

/// Тело tree-вью: message-баннер + скролл-тело с уровнями.
pub fn tree_view_body(
    view: &str,
    st: Option<&TreeViewState>,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let empty = TreeViewState::default();
    let st = st.unwrap_or(&empty);
    let mut rows: Vec<AnyElement> = Vec::new();
    // `scrollIntoView({block:"nearest"})` одноразового reveal: индекс строки
    // считаем при сборке, скроллим после — если узел уже отрисован
    let reveal = st.reveal.borrow().clone();
    let mut reveal_ix = None;
    level(
        &mut rows,
        view,
        st,
        "",
        0.0,
        tx,
        p,
        reveal.as_deref(),
        &mut reveal_ix,
    );
    if let Some(ix) = reveal_ix
        && let Some(item) = st.rows.bounds_for_item(ix)
    {
        // `scrollIntoView({ block: "nearest" })`: двигаем минимально — только
        // если строка вышла за край. Bounds прошлого кадра: пока их нет,
        // reveal остаётся взведённым и применится следующим кадром.
        let view = st.scroll.bounds();
        let (top, bottom) = (item.origin.y, item.origin.y + item.size.height);
        let (vtop, vbottom) = (view.origin.y, view.origin.y + view.size.height);
        let mut off = st.scroll.offset();
        if bottom > vbottom {
            off.y -= bottom - vbottom;
        } else if top < vtop {
            off.y += vtop - top;
        }
        st.scroll.set_offset(off);
        st.reveal.replace(None);
    }
    let mut root = div().flex_1().flex().flex_col().min_h(px(0.));
    if let Some(msg) = &st.meta.message {
        // инлайн-стиль оригинала: padding 4/8, fs-sm, opacity .75
        root = root.child(
            div()
                .px(px(m::SPACE_2))
                .py(px(m::SPACE_1))
                .text_size(px(m::FS_SM))
                .opacity(0.75)
                .child(SharedString::from(msg.clone())),
        );
    }
    root.child(
        // `.body`: flex 1, overflow auto, padding 4 6 8, fs-sm
        div()
            .id(SharedString::from(format!("tvbody:{view}")))
            .flex_1()
            .min_h(px(0.))
            // Видимый ползунок, как у файлового дерева: в оригинале оба тела
            // получают один и тот же вебкитовский thumb (ревью ц.18)
            .overflow_y_scrollbar_with(&st.scroll)
            .pt(px(m::SPACE_1))
            .px(px(6.0))
            .pb(px(m::SPACE_2))
            .text_size(px(m::FS_SM))
            .child(
                // Список строк отдельным элементом: с него снимаем bounds
                // строк для reveal (см. `TreeViewState::rows`)
                div()
                    .id(SharedString::from(format!("tvrows:{view}")))
                    .flex()
                    .flex_col()
                    .w_full()
                    .track_scroll(&st.rows)
                    .children(rows),
            ),
    )
    .into_any_element()
}
