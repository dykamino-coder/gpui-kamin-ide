//! Строки настроек и карточка удаления старого Bridge.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Карточка «Legacy Electron Bridge detected» (`LegacyBridgeCard.module.css`):
/// `.card` — flex-start, gap 12, padding 12, bg-surface, border divider-soft,
/// r-md; `.icon` 32×32 r-sm accent-primary глиф 16; `.title` 13/600;
/// `.desc` 12/1.5 muted; `.remove` — 4/12, border accent-red, текст 12/600
/// accent-red, ховер = красная заливка + #fff.
/// Рендерится ТОЛЬКО когда старый Bridge реально найден на машине.
pub(crate) fn legacy_bridge_card(
    fp: &crate::legacy_bridge::BridgeFootprint,
    // `busy` — удаление уже запущено (`LegacyBridgeCard.tsx:96-99`)
    busy: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> Option<AnyElement> {
    if !fp.found {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    if fp.squirrel {
        parts.push("installed app");
    }
    if fp.context_menu {
        parts.push("folder “Open with” menu entry");
    }
    if fp.config {
        parts.push("saved config");
    }
    let desc = format!(
        "Found: {}. KaminIDE has already imported its projects and sessions — \
you can safely remove the old app.",
        parts.join(", ")
    );
    let tx = tx.clone();
    Some(
        div()
            .flex()
            .items_start()
            .gap(px(m::SPACE_3))
            .p(px(m::SPACE_3))
            .rounded(px(m::RADIUS_MD))
            .bg(rgba(p.bg_surface))
            .border_1()
            .border_color(tint(rgba(p.text_primary), 0.06))
            .child(
                div()
                    .flex_shrink_0()
                    .w(px(32.0))
                    .h(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(m::RADIUS_SM))
                    .text_color(rgba(p.accent_primary))
                    .child(crate::ui::icon::fa("\u{f187}", 16.0)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .text_size(px(m::FS_MD))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgba(p.text_primary))
                            .child("Legacy Electron Bridge detected"),
                    )
                    .child(
                        div()
                            .mt(px(m::SPACE_1))
                            .text_size(px(m::FS_SM))
                            .line_height(px(m::FS_SM * 1.5))
                            .text_color(rgba(p.text_muted))
                            .child(desc),
                    ),
            )
            // `.remove { align-self: center }`: у родителя `items_start`,
            // поэтому центрируем кнопку обёрткой на всю высоту карточки
            // (в нашей версии gpui `self_center()` нет у Stateful)
            .child(
                div().flex().items_center().h_full().flex_shrink_0().child(
                    div()
                        .id("legacy-remove")
                        .flex_shrink_0()
                        .px(px(m::SPACE_3))
                        .py(px(m::SPACE_1))
                        .rounded(px(m::RADIUS_SM))
                        .border_1()
                        .border_color(rgba(p.accent_red))
                        .text_size(px(m::FS_SM))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgba(p.accent_red))
                        .cursor_pointer()
                        .hover(move |st| st.bg(rgba(p.accent_red)).text_color(gpui::white()))
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            // Оригинал сначала спрашивает подтверждение (danger),
                            // и лишь потом реимпортит сессии и сносит старый Bridge
                            let _ = tx.try_send(ShellEvent::OpenModal(crate::ui::modal::Modal {
                                title: "Remove the old Electron Bridge?".into(),
                                // `<br><br>` оригинала = пустая строка между
                                // абзацами (LegacyBridgeCard.tsx:44-46)
                                body: "KaminIDE replaces the standalone Electron Bridge. This \
uninstalls the app, deletes its config, and removes its folder “Open with” menu entry.\n\n\
Your sessions are re-imported into KaminIDE first, so nothing is lost."
                                    .into(),
                                confirm_label: "Remove it".into(),
                                cancel_label: Some("Keep it".into()),
                                danger: true,
                                prompt: None,
                                placeholder: None,
                                validate: None,
                                action: crate::ui::modal::ModalAction::RemoveLegacyBridge,
                            }));
                        })
                        .child(if busy {
                            "Removing…"
                        } else {
                            "Remove old Bridge"
                        }),
                ),
            )
            .into_any_element(),
    )
}
// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Чекбокс-строка настроек (label + описание).
pub(crate) fn pref_row(
    id: &'static str,
    key: &'static str,
    label: &'static str,
    desc: &'static str,
    value: bool,
    loaded: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let tx = tx.clone();
    // `.row` — gap 10, padding 4px 0, БЕЗ фона, радиуса и ховера (оригинал
    // `SettingsPanel.module.css:37-45`)
    let row = div()
        .id(id)
        .flex()
        .items_start()
        .gap(px(10.0))
        .py(px(4.0))
        .cursor_pointer()
        // ховера у строки настроек в оригинале НЕТ (`.row` без :hover)
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            // до загрузки префов строка неактивна (оригинал ставит
            // `disabled` на input — клик не должен уходить)
            if loaded {
                let _ = tx.try_send(ShellEvent::Cz(CzEvent::SetPref(key, !value)));
            }
        })
        .child(
            // Нативный `<input type="checkbox">`: 12×12, r2, `margin-top: 2px`.
            // Незажатый — белая заливка с рамкой `#767676`, зажатый — системный
            // accent `#0078d4` с белой галкой (Chromium на Windows).
            // Собственные 16×16 r4 сдвигали текст строки на +2.4 (ревью ц.23).
            div()
                .flex_shrink_0()
                .mt(px(2.0))
                .w(px(12.0))
                .h(px(12.0))
                .rounded(px(2.0))
                .border_1()
                // `:disabled` Chromium красит сам: незажатый — заливка
                // #EFEFEF с рамкой #C6C6C6, зажатый — сплошной #C6C6C6 с
                // белой галкой. Гашение `opacity .5` поверх белого давало
                // светло-серый, а не эти тона (ревью ц.35)
                .border_color(if loaded {
                    gpui::rgb(0x767676)
                } else {
                    gpui::rgb(0xc6c6c6)
                })
                .bg(if loaded {
                    gpui::rgb(0xffffff)
                } else {
                    gpui::rgb(0xefefef)
                })
                .flex()
                .items_center()
                .justify_center()
                .when(value, |d| {
                    let (fill, border) = if loaded {
                        (gpui::rgb(0x0078d4), gpui::rgb(0x0078d4))
                    } else {
                        (gpui::rgb(0xc6c6c6), gpui::rgb(0xc6c6c6))
                    };
                    d.bg(fill)
                        .border_color(border)
                        // Глиф — ТЕКСТ: без явной строчной высоты его
                        // line-box (≈1.25 кегля = 12.5 при боксе 12) выше
                        // контейнера, и галка съезжала вниз-вбок.
                        // Приравниваем line-height к кеглю — тогда
                        // flex-центрирование ставит её ровно
                        // (баг найден юзером)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(10.0))
                                .h(px(10.0))
                                .line_height(px(10.0))
                                .child(codicon("\u{eab2}", 10.0).text_color(gpui::white())),
                        )
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .gap(px(2.0))
                .child(
                    // `.row { font-size: 13px }`
                    div()
                        .text_size(px(m::FS_MD))
                        .text_color(rgba(p.text_primary))
                        .child(label),
                )
                .child(
                    // `.rowDesc` — 11px, line-height 1.5, без своей max-width
                    div()
                        .text_size(px(m::FS_XS))
                        .line_height(px(m::FS_XS * 1.5))
                        .text_color(rgba(p.text_muted))
                        .child(desc),
                ),
        );
    row.into_any_element()
}
