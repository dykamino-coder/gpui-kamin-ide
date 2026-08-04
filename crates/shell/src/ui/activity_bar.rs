//! Activity bar — прозрачная колонка 48px слева (ActivityBar.module.css 1:1):
//! паддинг 12 0, gap 8 (между списком и пикером), тайлы 32×32 radius 8,
//! иконка 18px; hover bg-surface 50% + text-primary; active accent-primary 16%.
//! Слот sidebar: пины из layout (default ["projects"]); клик = setActive.

pub use crate::ui::activity::glyphs::{phosphor_path, tool_glyph_group_hover, tool_glyph_split};
pub use crate::ui::activity::tile::tile;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use crate::probe::registry::probe_area;

pub struct ActivityEntry {
    pub id: &'static str,
    pub icon: &'static str,
}

/// Колонка активностей: фиксированный Customize-gear СВЕРХУ (не в pinned,
/// как оригинал), затем `entries` — пины слота sidebar.
/// `.dropPlaceholder` бара: 32×32 (форма плитки), r-sm, 1px dashed
/// accent 70%, фон accent 14% — обычный flex-item в `.list` (gap 2).
/// `.dropPlaceholder` рейла/бара — 32×32 пунктир accent 70% на фоне 14%.
pub fn drop_placeholder_el(p: &Palette) -> gpui::Div {
    drop_placeholder(p)
}

pub(crate) fn drop_placeholder(p: &Palette) -> gpui::Div {
    let mut a70 = rgba(p.accent_primary);
    a70.a = 0.7;
    let mut a14 = rgba(p.accent_primary);
    a14.a = 0.14;
    div()
        .w(px(32.0))
        .h(px(32.0))
        .flex_shrink_0()
        .rounded(px(m::RADIUS_SM))
        .border_1()
        .border_dashed()
        .border_color(a70)
        .bg(a14)
}

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
pub fn activity_bar(
    entries: &[ActivityEntry],
    // Канал для drag-жестов плиток (press + индекс вставки)
    tx: &Sender<ShellEvent>,
    active: Option<&str>,
    customize_active: bool,
    // Индекс вставки при tool-drag над баром (`.dropPlaceholder`)
    drop_index: Option<usize>,
    // `data-activity-drop="blocked"`: тул уже пришпилен к этому слоту —
    // дроп будет отвергнут, подсветка красная (`useActivityDropTarget.ts:37`)
    drop_blocked: bool,
    // id перетаскиваемой плитки: `.tileDragging > .btn { opacity: .3 }`
    dragging: Option<&str>,
    p: &Palette,
    on_activate: impl Fn(&'static str) + Clone + 'static,
    on_gear: impl Fn() + 'static,
    on_dots: impl Fn(f32, f32) + 'static,
) -> AnyElement {
    let mut bar = div()
        .id("activity-bar")
        .relative()
        .flex_shrink_0()
        // gpui/taffy — border-box: чтобы зазор `.body gap 8` не съедал ширину
        // бара, прибавляем 4 к ширине, а не отнимаем из неё (ревью ц.6:
        // плитки уезжали на 2px влево)
        .w(px(m::ACTIVITY_BAR_WIDTH + 4.0))
        .h_full()
        .pr(px(4.0))
        .flex()
        .flex_col()
        .items_center()
        // Оригинал: `.bar` gap 8 между группами (gear / list / picker),
        // `.list` внутри = 2px между плитками
        .gap(px(m::SPACE_2))
        .py(px(m::SPACE_3))
        .child(probe_area("activity-bar"))
        // `data-activity-drop` в оригинале висит и на самом `<nav>` бара —
        // колонка иконок тонируется вместе с картой (ревью ц.14). Скругления
        // у нава нет, поэтому радиус 0. Ширина тинта — ровно `<nav>` 48, а не
        // весь бокс 52 с компенсирующим `pr 4`; красное blocked-состояние
        // раньше терялось (ревью ц.15).
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .bottom_0()
                .w(px(m::ACTIVITY_BAR_WIDTH))
                .children(crate::ui::drop_hint::card_drop_r(
                    drop_index.is_some() || drop_blocked,
                    drop_blocked,
                    m::RADIUS_MD,
                    p,
                )),
        )
        .child(tile(
            "customize",
            "gear",
            customize_active,
            false,
            p,
            move |_, _| on_gear(),
            None,
        ));

    let mut list = div().flex().flex_col().items_center().w_full().gap(px(2.0));
    for (i, e) in entries.iter().enumerate() {
        if drop_index == Some(i) {
            list = list.child(drop_placeholder(p));
        }
        let cb = on_activate.clone();
        let id = e.id;
        list = list.child(tile(
            e.id,
            e.icon,
            active == Some(e.id),
            dragging == Some(e.id),
            p,
            move |_, _| cb(id),
            Some((
                crate::activity::PanelSlot::Sidebar,
                e.id.to_string(),
                i,
                tx.clone(),
            )),
        ));
    }
    // Вставка в конец списка (`overIndex === pinned.length`)
    if drop_index == Some(entries.len()) {
        list = list.child(drop_placeholder(p));
    }
    bar = bar.child(list);
    // «…» — пикер тулов сайдбара (как рейлы правых карт; оригинал ActivityBar)
    bar = bar.child({
        let hover_bg = {
            let mut c = rgba(p.bg_surface);
            c.a = 0.5;
            c
        };
        div()
            .id("activity-dots")
            // Якорь пикера — рект кнопки (ревью ц.15)
            .relative()
            .child(probe_area(crate::ui::tool_picker::picker_anchor_id(
                crate::activity::PanelSlot::Sidebar,
            )))
            .w(px(32.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.0))
            .text_color(rgba(p.text_muted))
            .cursor_pointer()
            .tooltip(crate::ui::tooltip::tooltip("Add or remove items"))
            .group("activity-dots-group")
            .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
            .on_mouse_down(
                gpui::MouseButton::Left,
                move |e: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    on_dots(f32::from(e.position.x), f32::from(e.position.y));
                },
            )
            .child(
                // `.picker:hover { color: text-primary }` красит и глиф;
                // бокс = кегль, как у остальных кодиконов (ревью ц.17)
                crate::ui::icon::codicon("\u{ea7c}", 18.0)
                    .text_color(rgba(p.text_muted))
                    .group_hover("activity-dots-group", {
                        let tp = rgba(p.text_primary);
                        move |st| st.text_color(tp)
                    }),
            )
    });
    bar.into_any_element()
}
