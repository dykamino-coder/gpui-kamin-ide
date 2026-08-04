//! Строка QuickPick: сепаратор, иконка, метка, описание, выделение.
//!
//! Тело цикла вынесено как есть (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use crate::ui::qp_state::QuickPickState;
use gpui::prelude::*;
use gpui::{SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

#[allow(clippy::too_many_arguments)]
pub(crate) fn qp_row<E: gpui::ParentElement>(
    list: E,
    st: &QuickPickState,
    pos: usize,
    i: usize,
    shown: &mut usize,
    req_id: u64,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> E {
    let mut list = list;
    let it = &st.items[i];
    if it.separator {
        // `.separator`: 4/12, mt 4, fs-xs uppercase muted, border-top
        // bg-surface 60% (у первого — без линии и без mt)
        // `.separator:first-child` — первый ребёнок СПИСКА, то есть
        // первый ОТФИЛЬТРОВАННЫЙ (CSS:123), а не первый в исходном
        // массиве: отфильтровали хвост — у нас появлялась лишняя
        // линия и `margin-top` (ревью ц.25)
        let is_first = pos == 0;
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_1))
                .when(!is_first, |d| {
                    d.mt(px(m::SPACE_1))
                        .border_t_1()
                        .border_color(tint(rgba(p.bg_surface), 0.6))
                })
                .text_size(px(m::FS_XS))
                .letter_spacing(px(m::FS_XS * 0.04))
                .text_color(rgba(p.text_muted))
                .child(SharedString::from(it.label.to_uppercase())),
        );
        return list;
    }
    *shown += 1;
    // `.item:hover { background: accent 18% }` (было text-primary 8%)
    let hover_bg = tint(rgba(p.accent_primary), 0.18);
    let tx_row = tx.clone();
    let multi = st.can_pick_many;
    let on = st.checked.contains(&i);
    let mut row = div()
        .id(SharedString::from(format!("qp-{i}")))
        .flex()
        // `.item`: baseline, 8/12, fs-md, text-primary
        .items_baseline()
        .gap(px(m::SPACE_2))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .rounded(px(m::RADIUS_SM))
        .text_size(px(m::FS_MD))
        .text_color(rgba(p.text_primary))
        .cursor_pointer()
        .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            if multi {
                let _ = tx_row.try_send(ShellEvent::QuickPickToggle(i));
            } else {
                let _ = tx_row.try_send(ShellEvent::QuickPickResolve(req_id, Some(vec![i])));
            }
        });
    if multi {
        row = row.child(
            // `.check{font-size:13px}` стоит на самом `.codicon` → проигрывает
            // вендорной базе: эффективный кегль 16. Цвет `accent-primary`
            // у ОБОИХ состояний (ревью ц.14)
            codicon(if on { "\u{eab2}" } else { "\u{ebb5}" }, 16.0)
                // `.check { color: var(--accent-primary) }` — ОБА состояния
                // (ревью ц.14: невыбранный красился text-muted)
                .text_color(rgba(p.accent_primary))
                // `.check { align-self: center }`: у `Div` в gpui нет
                // `self_center()`, поэтому центрируем боксом строки —
                // глиф иначе садится на базовую линию (`items_baseline`)
                .flex()
                .items_center()
                .flex_shrink_0(),
        );
    }
    // `renderCodiconText` (`QuickPickModal.tsx:107-108,113`): `$(icon)`
    // в подписи, описании и детали — это ГЛИФ, а не текст (ревью ц.35)
    row = row
        .child(
            div()
                .flex_shrink_0()
                .child(crate::ui::codicon_text::render(&it.label, m::FS_MD)),
        )
        .when(!it.description.is_empty(), |r| {
            // `.description { color: text-muted; font-size: fs-sm }`
            r.child(
                div()
                    .text_size(px(m::FS_SM))
                    .text_color(rgba(p.text_muted))
                    .child(crate::ui::codicon_text::render(&it.description, m::FS_SM)),
            )
        })
        .when(!it.detail.is_empty(), |r| {
            // `.detail`: ml auto, моно fs-xs, эллипсис
            r.child(
                div()
                    .ml_auto()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font_family(crate::ui::design_panel::MONO)
                    .text_size(px(m::FS_XS))
                    .text_color(rgba(p.text_muted))
                    .child(crate::ui::codicon_text::render(&it.detail, m::FS_XS)),
            )
        });
    list = list.child(crate::ui::focus_ring::focusable(
        row,
        &format!("qp-{i}"),
        m::RADIUS_SM,
        rgba(p.accent_primary),
    ));
    list
}
