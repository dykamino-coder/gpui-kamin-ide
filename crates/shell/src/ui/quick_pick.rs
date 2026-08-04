//! QuickPickModal (B8 1:1, ядро): vscode.window.showQuickPick от exthost.
//! Top-center бокс (стиль палитры): title? + фильтр-инпут + список; single —
//! клик резолвит [index]; canPickMany — чекбоксы + OK. Esc/скрим → null.
//! Ответ хосту — отложенный respond(req_id) (kamin_ws HostReply::Later).

pub use crate::ui::qp_state::QuickPickState;

use gpui::prelude::*;
use gpui::{AnyElement, Entity, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Рендер модалки (в overlay). filter — значение фильтр-инпута.
pub fn quick_pick(
    st: &QuickPickState,
    input: Option<&Entity<InputState>>,
    // Каретка в поле? `.input:focus { border-color: accent-primary }`,
    // иначе рамка `bg-surface 70 %` (CSS:42,48) — у нас была всегда accent
    // (ревью ц.26)
    input_focused: bool,
    filter: &str,
    vw: f32,
    vh: f32,
    tx: &Sender<ShellEvent>,
    p: &'static Palette,
) -> AnyElement {
    let req_id = st.req_id;
    let mut list = div()
        .id("qp-list")
        .flex()
        .flex_col()
        // `.list { flex: 1 }` — список забирает остаток панели, ряд
        // Cancel/OK прижат к низу (ревью ц.20: под ним зияло ~290 px)
        .flex_1()
        .min_h(px(0.))
        // `max-height` у `.list` НЕТ (CSS:50-59): высоту диктует `flex: 1`
        // внутри панели. Наш лишний потолок оставлял ~60 px пустоты под
        // списком — ровно тот дефект, который ц.23 счёл закрытым (ревью ц.25)
        .overflow_y_scrollbar()
        .gap(px(1.0));
    // Один источник правды с Enter: те же индексы, что вернёт `enter_pick`
    let visible = st.filtered(filter);
    let mut shown = 0usize;
    for (pos, i) in visible.iter().copied().enumerate() {
        list = crate::ui::qp_row::qp_row(list, st, pos, i, &mut shown, req_id, tx, p);
    }

    // `filtered.length === 0` (`QuickPickModal.tsx:91`): сепараторы входят в
    // `filtered`, значит пик с сепараторами и нулём совпадений подписи НЕ
    // показывает — мы считали только не-сепараторы (ревью ц.25)
    let _ = shown;
    if visible.is_empty() {
        // `.empty`: 12/16, italic, text-muted
        list = list.child(
            div()
                .px(px(m::SPACE_4))
                .py(px(m::SPACE_3))
                .italic()
                .text_color(rgba(p.text_muted))
                .child("No matching items"),
        );
    }

    let panel_w = 640.0_f32.min(vw - 32.0);
    let mut boxx = div()
        .id("quick-pick")
        .occlude()
        .absolute()
        .top(px(84.0))
        .left(px(((vw - panel_w) / 2.0).max(16.0)))
        .w(px(panel_w))
        // `.overlay` не задаёт `align-items` ⇒ stretch: высота панели ВСЕГДА
        // `min(100vh − 84, 60vh)`, а не по содержимому (ревью ц.17)
        .h(px((0.6_f32 * vh).min(vh - 84.0)))
        .overflow_hidden()
        .flex()
        .flex_col()
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_mantle))
        .border_1()
        .border_color(tint(rgba(p.bg_surface), 0.8))
        .shadow(crate::ui::shadows::modal())
        .child(crate::overlay::hit_area());
    if let Some(t) = &st.title {
        boxx = boxx.child(
            div()
                // `.title`: 8/16 + border-bottom bg-surface 60%
                .px(px(m::SPACE_4))
                .py(px(m::SPACE_2))
                .border_b_1()
                .border_color(tint(rgba(p.bg_surface), 0.6))
                .text_size(px(m::FS_SM))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_primary))
                .child(t.clone()),
        );
    }
    if let Some(inp) = input {
        boxx = boxx.child(
            // `.input`: margin 8/12/0, padding 8/12, bg-base, рамка
            // bg-surface 70%, radius-sm, fs-md
            div()
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .mx(px(m::SPACE_3))
                .mt(px(m::SPACE_2))
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .bg(rgba(p.bg_base))
                .border_1()
                // `.input:focus { border-color: accent-primary }` — инпут пика
                // всегда сфокусирован (ревью ц.23)
                .border_color(if input_focused {
                    rgba(p.accent_primary)
                } else {
                    tint(rgba(p.bg_surface), 0.7)
                })
                .text_size(px(m::FS_MD))
                .child(
                    div().flex_1().child(
                        Input::new(inp)
                            .appearance(false)
                            // `--fs-md` 13 и НУЛЕВОЙ собственный бокс: свои
                            // `px 8 / py 2 / h 24` Input ставит до
                            // `refine_style`, отступы даёт ряд (ревью ц.20)
                            .with_size(gpui_component::Size::Size(px(m::FS_MD / 0.875)))
                            .px_0()
                            .py_0()
                            .h_full(),
                    ),
                ),
        );
    }
    if let Some(prompt) = &st.prompt {
        // `.prompt`: 4/16/0, fs-sm, text-secondary
        boxx = boxx.child(
            div()
                .px(px(m::SPACE_4))
                .pt(px(m::SPACE_1))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_secondary))
                .child(prompt.clone()),
        );
    }
    // Обёртка тоже растягивается, иначе `flex_1` списка мёртв и под
    // рядом кнопок остаётся пустой mantle (ревью ц.23)
    boxx = boxx.child(
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .p(px(m::SPACE_1))
            .child(list),
    );
    if st.can_pick_many {
        boxx = boxx.child(crate::ui::qp_actions::actions(st, req_id, tx, p));
    }
    // `.overlay`: inset 0, `--overlay-modal`, центрирование по горизонтали,
    // отступ сверху `--layout-palette-top-offset` = 84. Клик по скриму
    // закрывает пикер, если расширение не запросило `ignoreFocusOut`.
    let tx_scrim = tx.clone();
    let ignore_focus_out = st.ignore_focus_out;
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(crate::ui::scrim::modal())
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            if !ignore_focus_out {
                let _ = tx_scrim.try_send(ShellEvent::QuickPickResolve(req_id, None));
            }
        })
        .child(boxx)
        .into_any_element()
}
