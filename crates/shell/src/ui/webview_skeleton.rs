//! Скелет загрузки вебвью-панели и терминальная ошибка загрузки.
//!
//! Визуал скелета = ЕДИНЫЙ брендовый лоадер (`chat_switch_skeleton::
//! brand_loader`): раньше здесь жил собственный шиммер-скелет строк — светлее
//! фоном, с прямыми углами и без отступов, «какая-то своя особенная загрузка»
//! рядом с брендовой шторкой чата. Теперь загрузка везде выглядит одинаково;
//! от старого остались только подпись ожидания (после трёх секунд — какой
//! ретрай идёт) и карточка ошибки с Retry.

use crate::host::events::TermEvent;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;

/// `EXPLAIN_AFTER_S` — до трёх секунд подпись не показывается.
const EXPLAIN_AFTER_S: u64 = 3;

/// Отступы и радиус как у ГОТОВОГО вью (webview_body: p(8) + RADIUS_MD) —
/// лоадер выглядит той же карточкой, что и загруженная панель.
fn card(p: &Palette) -> gpui::Div {
    div()
        .absolute()
        .inset(px(8.0))
        .rounded(px(m::RADIUS_MD))
        .overflow_hidden()
        .bg(rgba(p.editor_bg))
}

/// Скелет загрузки панели: брендовый лоадер + подпись ожидания.
pub fn skeleton(seconds: u64, attempts: u32, p: &Palette) -> AnyElement {
    let mut wrap = card(p).flex().flex_col().child(
        div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .child(crate::ui::chat_switch_skeleton::brand_loader(p, "Loading…")),
    );

    if seconds >= EXPLAIN_AFTER_S {
        let mut note = format!("Waiting for the extension host to open this panel · {seconds}s");
        if attempts > 1 {
            note.push_str(&format!(" · attempt {attempts}"));
        }
        wrap = wrap.child(
            div()
                .pb(px(m::SPACE_3))
                .w_full()
                .text_center()
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_disabled))
                .child(note),
        );
    }
    wrap.into_any_element()
}

/// Терминальное состояние: бюджет resolve исчерпан.
pub fn load_error(view_id: &str, tx: &Sender<ShellEvent>, p: &Palette) -> AnyElement {
    let tx = tx.clone();
    let id = view_id.to_string();
    let hover_bg = tint(rgba(p.text_primary), 0.12);
    card(p)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(m::SPACE_2))
        .p(px(24.0))
        .text_center()
        .child(
            // fa-triangle-exclamation 22px accent-yellow @ .85; бокс = advance
            // глифа, как у инлайнового `<i>` (icon::fa жёстко 16×16 — обрежет)
            div()
                .mb(px(m::SPACE_1))
                .font_family(crate::ui::icon::FA_FAMILY)
                .font_weight(gpui::FontWeight::BLACK)
                .text_size(px(22.0))
                .text_color(tint(rgba(p.accent_yellow), 0.85))
                .child("\u{f071}"),
        )
        .child(
            div()
                .text_size(px(m::FS_MD))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgba(p.text_primary))
                .child("This panel didn't load"),
        )
        .child(
            div()
                .max_w(px(280.0))
                .text_size(px(m::FS_SM))
                // `.errHint { line-height: 1.4 }`
                .line_height(px(m::FS_SM * 1.4))
                .text_color(rgba(p.text_muted))
                .child("The extension host may still be starting up."),
        )
        .child(
            div()
                .id("wv-retry")
                .flex()
                .items_center()
                .gap(px(6.0))
                .px(px(16.0))
                .py(px(6.0))
                .rounded(px(m::RADIUS_SM))
                .border_1()
                // `--divider-soft` = text-primary 6 %; 14 % — CSS-fallback
                // `.retry`, который никогда не срабатывает (ревью ц.13)
                .border_color(tint(rgba(p.text_primary), 0.06))
                .bg(tint(rgba(p.text_primary), 0.06))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ShellEvent::Term(TermEvent::RetryView(id.clone())));
                })
                .child(
                    // fa-rotate; кегль наследуется от кнопки (fs-sm)
                    div()
                        .font_family(crate::ui::icon::FA_FAMILY)
                        .font_weight(gpui::FontWeight::BLACK)
                        .child("\u{f2f1}"),
                )
                .child("Retry"),
        )
        .into_any_element()
}
