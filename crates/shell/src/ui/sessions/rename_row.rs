//! Строка сессии в режиме переименования: инпут вместо имени.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, div, px};
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputState};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Строка в режиме переименования: имя заменено фокусным инпутом.
pub(crate) fn rename_row(
    input: &Entity<InputState>,
    row: gpui::Stateful<gpui::Div>,
    light: bool,
    is_active: bool,
    has_color: bool,
    dot: AnyElement,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let tx_key = tx.clone();
    // `.editing { background: var(--bg-surface) }` объявлен ПОЗЖЕ `.row`,
    // `.tinted` и `.active`, поэтому в тёмной теме перекрывает их. Но
    // светло-тематические `:global([data-theme=light]) .active|.tinted`
    // имеют специфичность 0,2,0 против 0,1,0 у `.editing` — там градиент
    // остаётся. Ховер-правила (`.row:hover`, `.tinted:hover`,
    // `.active:hover`) тоже 0,2,0 и уже висят на строке своим `.hover()`
    let editing_bg_wins = !(light && (is_active || has_color));
    row.when(editing_bg_wins, |r| r.bg(rgba(p.bg_surface)))
        .child(dot)
        .child(
            // `.renameInput`: bg-base, рамка 1px accent-primary,
            // radius-xs, padding 1/4, кегль fs-sm
            // (`SessionItem.module.css:176-187`) — раньше поле рисовалось
            // голым `appearance(false)` без фона, рамки и паддинга
            div()
                .relative()
                .child(crate::probe::registry::probe_area("session-rename-input"))
                .flex_1()
                .min_w(px(0.))
                .px(px(4.0))
                .py(px(1.0))
                // Высота коробки НЕ фиксируется: у оригинала 18.4
                // (padding 1/1 + рамка 1/1 + строка 14.4), у нас 20 —
                // вендорный `Input` рисует строку со своим вертикальным
                // сдвигом, и жёсткие 18.4 срезали нижние выносные (ц.35)
                .rounded(px(m::RADIUS_XS))
                .bg(rgba(p.bg_base))
                .border_1()
                .border_color(rgba(p.accent_primary))
                .on_key_down(
                    move |ev: &gpui::KeyDownEvent, _, _| match ev.keystroke.key.as_str() {
                        "enter" => {
                            let _ = tx_key.try_send(ShellEvent::CommitRename);
                        }
                        "escape" => {
                            let _ = tx_key.try_send(ShellEvent::CancelRename);
                        }
                        _ => {}
                    },
                )
                .child(
                    Input::new(input)
                        .appearance(false)
                        // `font-size: var(--fs-sm)`; Input берёт кегль из
                        // своего Size (×0.875)
                        .with_size(gpui_component::Size::Size(px(m::FS_SM / 0.875)))
                        // Свои паддинги (px 8) и высоту (h 24) Input
                        // ставит ДО `refine_style`, поэтому гасим их:
                        // отступ и рамку даёт обёртка `.renameInput`
                        // (padding 1/4), а поле высотой 24 срезало
                        // горизонтальные кромки строки 24 (ревью ц.20)
                        .px_0()
                        .py_0()
                        .h(px(16.0)),
                ),
        )
        .into_any_element()
}
