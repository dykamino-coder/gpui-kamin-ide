//! Статус-бар (StatusBar 1:1): прозрачный фон, fs-xs text-muted, слева группа
//! счётчиков расширений/команд (circle-filled ok, warning, circle-slash,
//! symbol-keyword), справа — бренд-пилюля «KaminIDE {version}» accent-primary.
//! Contributed-items + encoding/EOL (editor-scoped) — на фазе редактора/VSIX.

use crate::host::events::CzEvent;
pub use crate::ui::status::contrib::ContribItem;
use crate::ui::status::item::{CIRCLE_FILLED, CIRCLE_SLASH, SYMBOL_KEYWORD, WARNING, item};

use crate::ui::status::contrib::contrib;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use crate::probe::registry::probe_area;
use smol::channel::Sender;

/// Счётчики реестра для статус-бара.
#[derive(Clone, Copy, Default)]
pub struct StatusCounts {
    pub ext_active: usize,
    pub ext_failed: usize,
    pub ext_disabled: usize,
    pub cmd_count: usize,
}

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Статус-бар: h из метрик, слева счётчики, справа [UTF-8][LF|CRLF]+бренд.
/// `eol` — Some при открытом редакторе (детект при открытии файла).
pub fn status_bar(
    counts: StatusCounts,
    update: Option<(String, String)>,
    // Идёт установка: (скачано, всего) → заливка + «Updating …»
    downloading: Option<(u64, Option<u64>)>,
    contrib_items: Vec<ContribItem>,
    // Открыт ли файл вообще: `UTF-8` рисуется по нему, а не по `eol`
    has_file: bool,
    eol: Option<&'static str>,
    version: &str,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let has_update = update.is_some();
    // align-items: stretch — пилюли на всю высоту 24 (ревью ц.1)
    let mut left = div().flex().gap(px(2.0)).child(item(
        Some(CIRCLE_FILLED),
        format!("{} active", counts.ext_active),
        rgba(p.accent_green),
        "Active extensions",
        p,
        Some(Box::new({
            let tx = tx.clone();
            move || {
                // Раздел Extensions Customize-режима
                let _ = tx.try_send(ShellEvent::Cz(CzEvent::OpenCustomizePanel("extensions")));
            }
        })),
    ));
    if counts.ext_failed > 0 {
        left = left.child(item(
            Some(WARNING),
            format!("{} failed", counts.ext_failed),
            rgba(p.accent_yellow),
            "Failed activations",
            p,
            None,
        ));
    }
    if counts.ext_disabled > 0 {
        left = left.child(item(
            Some(CIRCLE_SLASH),
            format!("{} off", counts.ext_disabled),
            rgba(p.text_muted),
            "Disabled extensions",
            p,
            None,
        ));
    }
    left = left.child(item(
        Some(SYMBOL_KEYWORD),
        format!("{} cmds", counts.cmd_count),
        rgba(p.text_muted),
        "Registered commands",
        p,
        Some(Box::new({
            let tx = tx.clone();
            move || {
                let _ = tx.try_send(ShellEvent::TogglePalette);
            }
        })),
    ));
    // Contributed items (alignment=1 Left) — по убыванию priority
    let mut items: Vec<&ContribItem> = contrib_items.iter().filter(|i| i.visible).collect();
    items.sort_by(|a, b| b.priority.total_cmp(&a.priority));
    for it in items.iter().filter(|i| i.alignment == 1) {
        left = left.child(contrib(it, p));
    }

    // Правая группа: бренд-пилюля версии (accent-primary, weight 500)
    // Оригинал: бренд, пилюля апдейта и прогресс — ТРИ ВЗАИМОИСКЛЮЧАЮЩИХ
    // состояния; idle-бренд кликабелен («Check for updates»).
    let brand_hover = {
        let mut c = rgba(p.bg_surface);
        c.a = 0.6;
        c
    };
    let tx_check = tx.clone();
    let brand = div()
        .id("status-brand")
        .relative()
        .child(probe_area("status-version"))
        .flex()
        .items_center()
        .px(px(m::SPACE_2))
        .rounded(px(m::RADIUS_XS))
        .text_size(px(m::FS_XS))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(rgba(p.accent_primary))
        .cursor_pointer()
        // `.item:hover { color: text-primary }` (0,1,1) перебивает
        // `.brand { color: accent-primary }` (0,1,0) — цвет тоже поднимается
        // (ревью ц.14)
        .hover(move |st| st.bg(brand_hover).text_color(rgba(p.text_primary)))
        .tooltip(crate::ui::tooltip::tooltip("Check for updates"))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            let _ = tx_check.try_send(ShellEvent::Cz(CzEvent::CheckForUpdates));
        })
        .child(format!("KaminIDE {version}"));

    div()
        .id("status-bar")
        .relative()
        .flex_shrink_0()
        .h(px(m::STATUS_BAR_HEIGHT))
        .w_full()
        .flex()
        .px(px(m::SPACE_2))
        .gap(px(m::SPACE_1))
        .text_size(px(m::FS_XS))
        .text_color(rgba(p.text_muted))
        .child(probe_area("status-bar"))
        .child(left)
        .child(
            div()
                .flex()
                .gap(px(2.0))
                .ml_auto()
                .children({
                    // Правая группа: priority ASC (ревью ц.1; левая — desc)
                    let mut right: Vec<&ContribItem> =
                        items.iter().filter(|i| i.alignment == 2).copied().collect();
                    right.sort_by(|a, b| a.priority.total_cmp(&b.priority));
                    right
                        .into_iter()
                        .map(|it| contrib(it, p))
                        .collect::<Vec<_>>()
                })
                // Порядок: contributed → encoding/EOL → update → brand
                // `UTF-8` рисуется при ЛЮБОМ открытом файле, а строка EOL —
                // только когда он известен (`StatusBar.tsx:55-63`); общий
                // `when_some` гасил и кодировку (ревью ц.15)
                .when(has_file, |row| {
                    row.child(
                        div()
                            .relative()
                            // Обёртка обязана ТЯНУТЬСЯ: у группы статус-бара
                            // `align-items: stretch`, и без `flex + h_full`
                            // пилюля UTF-8 схлопывалась по контенту и стала
                            // ниже соседей (баг найден юзером)
                            .flex()
                            .h_full()
                            .child(probe_area("status-encoding"))
                            .child(item(None, "UTF-8", rgba(p.text_muted), "Encoding", p, None)),
                    )
                })
                .when_some(eol, |row, eol| {
                    row.child(item(None, eol, rgba(p.text_muted), "End of line", p, None))
                })
                .when_some(update, |row, (ver, _url)| {
                    row.child(crate::ui::status_update_pill::update_pill(
                        &ver,
                        downloading,
                        version,
                        tx,
                        p,
                    ))
                })
                .when(!has_update, |row| row.child(brand)),
        )
        .into_any_element()
}
