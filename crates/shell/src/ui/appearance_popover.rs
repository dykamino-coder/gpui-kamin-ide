//! Поповер оформления: темы и наборы иконок.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::{Palette, ThemeKind};
use smol::channel::Sender;

/// Поповер Appearance: Dark/Light + Icons (клики не закрывают).
#[allow(clippy::too_many_arguments)]
pub fn appearance_popover(
    theme: ThemeKind,
    theme_choice: &'static str,
    contrib_themes: &[(String, String, String, bool)],
    contrib_active: Option<&str>,
    icon_themes: &[(String, String, String)],
    icon_active: Option<&str>,
    vw: f32,
    window: &mut gpui::Window,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    // Ширина колонки по САМОЙ ДЛИННОЙ подписи: px 12×2 + иконка 16 + gap 8 +
    // текст + gap 8 + галка 12; нижняя граница 140 (`minmax(140px, 1fr)`).
    let col_w = |labels: &[String], window: &mut gpui::Window| -> f32 {
        let text = labels
            .iter()
            .map(|l| crate::ui::text_fit::measure(l, m::FS_SM, window))
            .fold(0.0_f32, f32::max);
        (m::SPACE_3 * 2.0 + 16.0 + m::SPACE_2 + text + m::SPACE_2 + 12.0).max(140.0)
    };
    // ThemeQuickToggle 1:1: header «Appearance» + System-тумблер, 3 колонки
    // Dark / Light / Icons; пики НЕ закрывают поповер.
    let builtin_active = contrib_active.is_none();
    let item = |id: SharedString,
                glyph: &'static str,
                label: SharedString,
                on: bool,
                ev: Option<ShellEvent>,
                tx: Sender<ShellEvent>| {
        let hover_bg = tint(rgba(p.text_primary), 0.10);
        let mut row = div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .text_size(px(m::FS_SM))
            .text_color(rgba(p.text_primary))
            .child(
                div()
                    .w(px(16.0))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .child(
                        crate::ui::icon::fa(glyph, 12.0)
                            .text_color(rgba(p.text_primary))
                            .into_any_element(),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(label),
            );
        // .picked: постоянный accent-фон 16% (hover его не перебивает)
        if on {
            row = row.bg(tint(rgba(p.accent_primary), 0.16));
        }
        if let Some(ev) = ev {
            row = row
                .cursor_pointer()
                .when(!on, |r| r.hover(move |s| s.bg(hover_bg)))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ev.clone());
                });
        }
        // Галка всегда в вёрстке (visibility) — ширина колонок стабильна
        row = row.child(
            div()
                .w(px(12.0))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .child(
                    // .itemTick — fa-check (не codicon), ревью ц.1
                    crate::ui::icon::fa("\u{f00c}", 10.0)
                        .text_color(rgba(p.accent_primary))
                        .when(!on, |c| c.invisible()),
                ),
        );
        row.into_any_element()
    };
    let column = |title: &'static str, rows: Vec<AnyElement>, w: f32| {
        div()
            .flex()
            .flex_col()
            .gap(px(m::SPACE_1))
            // Ширина ИЗМЕРЕНА по самой длинной подписи (не меньше 140):
            // `minmax(140px, 1fr)` при `width: max-content` = интринсик,
            // а фикс-454 резал длинные имена тем (пойман юзером дважды)
            .w(px(w))
            .flex_shrink_0()
            .child(
                div()
                    .px(px(m::SPACE_2))
                    .py(px(m::SPACE_1))
                    .text_size(px(m::FS_XS))
                    // `letter-spacing: 0.04em` (`ThemeQuickToggle.module.css:102`)
                    .letter_spacing(px(m::FS_XS * 0.04))
                    .text_color(rgba(p.text_muted))
                    .child(title.to_uppercase()),
            )
            .child(
                // .colList: max-height 320 + скролл (длинные списки тем)
                div()
                    .id(SharedString::from(format!("ap-col-{title}")))
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .max_h(px(320.0))
                    .overflow_y_scrollbar()
                    .children(rows),
            )
            .into_any_element()
    };

    let contrib_col = |dark: bool| -> Vec<AnyElement> {
        contrib_themes
            .iter()
            .filter(|(_, _, _, dark_ui)| *dark_ui == dark)
            .map(|(id, label, path, dark_ui)| {
                item(
                    SharedString::from(format!("ap-ct-{id}")),
                    if *dark_ui { "\u{f186}" } else { "\u{f185}" },
                    SharedString::from(label.clone()),
                    contrib_active == Some(id.as_str()),
                    Some(ShellEvent::Cz(CzEvent::SetContributedTheme(
                        id.clone(),
                        path.clone(),
                        *dark_ui,
                    ))),
                    tx.clone(),
                )
            })
            .collect()
    };

    let mut dark_rows = vec![item(
        "ap-dark".into(),
        "\u{f186}",
        "Kamin Dark".into(),
        builtin_active && theme == ThemeKind::Dark && theme_choice != "system",
        Some(ShellEvent::SetThemeChoice("dark")),
        tx.clone(),
    )];
    dark_rows.extend(contrib_col(true));
    let mut light_rows = vec![item(
        "ap-light".into(),
        "\u{f185}",
        "Kamin Light".into(),
        builtin_active && theme == ThemeKind::Light && theme_choice != "system",
        Some(ShellEvent::SetThemeChoice("light")),
        tx.clone(),
    )];
    light_rows.extend(contrib_col(false));
    let mut icon_rows = vec![item(
        "ap-cat".into(),
        "\u{f86d}",
        "Catppuccin".into(),
        icon_active.is_none(),
        Some(ShellEvent::Cz(CzEvent::SetIconTheme(None))),
        tx.clone(),
    )];
    icon_rows.extend(icon_themes.iter().map(|(id, label, path)| {
        item(
            SharedString::from(format!("ap-it-{id}")),
            "\u{f86d}",
            SharedString::from(label.clone()),
            icon_active == Some(id.as_str()),
            Some(ShellEvent::Cz(CzEvent::SetIconTheme(Some((
                id.clone(),
                path.clone(),
            ))))),
            tx.clone(),
        )
    }));

    // Ширины колонок по фактическим подписям (см. `col_w`)
    let dark_labels: Vec<String> = std::iter::once("Kamin Dark".to_string())
        .chain(
            contrib_themes
                .iter()
                .filter(|(_, _, _, dark)| *dark)
                .map(|(_, label, _, _)| label.clone()),
        )
        .collect();
    let light_labels: Vec<String> = std::iter::once("Kamin Light".to_string())
        .chain(
            contrib_themes
                .iter()
                .filter(|(_, _, _, dark)| !*dark)
                .map(|(_, label, _, _)| label.clone()),
        )
        .collect();
    let icon_labels: Vec<String> = std::iter::once("Catppuccin".to_string())
        .chain(icon_themes.iter().map(|(_, label, _)| label.clone()))
        .collect();
    // `grid-template-columns: repeat(3, minmax(140px, 1fr))` — равные треки:
    // ширину задаёт самая длинная подпись ЛЮБОЙ из колонок, а не своя
    // (ревью ц.15: колонки расходились на десятки px)
    let col = col_w(&dark_labels, window)
        .max(col_w(&light_labels, window))
        .max(col_w(&icon_labels, window));
    let (w_dark, w_light, w_icons) = (col, col, col);
    // `width: max-content` — сумма колонок + два gap + p 8×2 + рамка
    let pop_w = w_dark + w_light + w_icons + m::SPACE_2 * 2.0 + m::SPACE_2 * 2.0 + 2.0;

    // .sysToggle: off=transparent+muted, hover text-primary 10%;
    // .sysOn (и его hover) accent 16% + text-primary
    crate::ui::appearance_frame::frame(
        dark_rows,
        light_rows,
        icon_rows,
        w_dark,
        w_light,
        w_icons,
        pop_w,
        builtin_active,
        theme_choice,
        vw,
        column,
        tx,
        p,
    )
}
