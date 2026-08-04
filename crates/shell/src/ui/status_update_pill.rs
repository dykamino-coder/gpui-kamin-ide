//! Пилюля апдейта в статус-баре: «Update X» и прогресс установки.
//!
//! Вынесено из `status_bar.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
use gpui::prelude::*;
use gpui::{div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;

/// `ver` — версия из манифеста, `version` — текущая; `downloading` —
/// (скачано, всего) во время установки.
pub(crate) fn update_pill(
    ver: &str,
    downloading: Option<(u64, Option<u64>)>,
    version: &str,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id("update-pill")
        .flex()
        .items_center()
        // `.item { gap: 4; padding: 0 8 }` — было 3 и py 1
        .gap(px(m::SPACE_1))
        .px(px(m::SPACE_2))
        .rounded(px(m::RADIUS_XS))
        .bg(tint(rgba(p.accent_primary), 0.22))
        .text_color(rgba(p.accent_primary))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        // Во время закачки оригинал рисует `div` без `.clickable` —
        // курсор обычный (ревью ц.23)
        .when(downloading.is_none(), |d| d.cursor_pointer())
        // .update:hover = accent 34% (ревью ц.1)
        .hover({
            let hb = tint(rgba(p.accent_primary), 0.34);
            move |s| s.bg(hb)
        })
        .tooltip(crate::ui::tooltip::tooltip(match downloading {
            Some(_) => "Downloading the KaminIDE update…".to_string(),
            None => format!("Update to KaminIDE {ver} — you have {version}"),
        }))
        // `installUpdate()` — качаем и ставим САМИ, как оригинал;
        // повторный клик во время установки игнорируется
        // (ревью ц.20, просьба юзера)
        .on_mouse_down(gpui::MouseButton::Left, {
            let tx = tx.clone();
            let busy = downloading.is_some();
            move |_, _, _| {
                if !busy {
                    let _ = tx.try_send(ShellEvent::Cz(CzEvent::StartUpdateInstall));
                }
            }
        })
        // codicon-cloud-download = eac2; ea9a — это arrow-down, другой
        // глиф (ревью ц.15). `.downloading { overflow: hidden }` + заливка
        .when(downloading.is_some(), |d| d.relative().overflow_hidden())
        .when_some(downloading, |d, (done, total)| {
            let frac = match total {
                Some(t) if t > 0 => (done as f32 / t as f32).clamp(0.0, 1.0),
                // Размер неизвестен — полная заливка .5, как
                // indeterminate у оригинала
                _ => 1.0,
            };
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(gpui::relative(frac))
                    .bg(tint(rgba(p.accent_primary), 0.32))
                    .when(total.is_none(), |f| f.opacity(0.5)),
            )
        })
        .child(
            // `.progressLabel { position: relative; gap: 6 }` существует
            // ТОЛЬКО при закачке; в состоянии «доступен апдейт» глиф и
            // текст лежат прямо в `.item { gap: 4 }` (ревью ц.23)
            div()
                .relative()
                .flex()
                .items_center()
                .gap(px(if downloading.is_some() {
                    6.0
                } else {
                    m::SPACE_1
                }))
                .child(codicon("\u{eac2}", 12.0))
                .child(match downloading {
                    Some((done, Some(total))) if total > 0 => {
                        format!("Updating {}%", (done * 100 / total).min(100))
                    }
                    Some((done, _)) => {
                        format!("Updating {:.1} MB", done as f64 / 1024.0 / 1024.0)
                    }
                    None => format!("Update {ver}"),
                }),
        )
}
