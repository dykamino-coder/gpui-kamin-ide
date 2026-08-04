//! WelcomePlaceholder 1:1: полноэкранная брендовая заглушка main-колонки, когда
//! нет активной сессии. Лого kaminoid 112 (+ мягкий accent-glow), заголовок
//! «KaminIDE» 2.4rem, версия-пилюля, tagline, две кнопки (New session in
//! folder… / Empty session), три feature-чипа.

use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, App, Image, SharedString, Window, div, img, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::{rgba, tint};
use crate::host_link;
use crate::ui::icon::{FA_FAMILY, fa};

const FOLDER_OPEN: &str = "\u{f07c}";
const PLUS: &str = "\u{2b}";
const COMMENTS: &str = "\u{f086}";
const FOLDER_TREE: &str = "\u{f802}";
const TERMINAL: &str = "\u{f120}";

fn feature(glyph: &'static str, label: &'static str, p: &Palette) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_muted))
        .child(fa(glyph, 13.0).text_color(rgba(p.accent_primary)))
        .child(label)
        .into_any_element()
}

/// Welcome-заглушка. `on_folder` открывает нативный пикер папки (нужен App).
/// `glow` — запечённый мягкий accent-спрайт под лого (radial в gpui нет).
pub fn welcome(
    version: &str,
    glow: Arc<Image>,
    on_folder: impl Fn(&mut Window, &mut App) + 'static,
    p: &Palette,
) -> AnyElement {
    div()
        .id("welcome")
        .relative()
        .child(crate::probe::registry::probe_area("welcome"))
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(m::SPACE_4))
        .p(px(m::SPACE_6))
        .text_center()
        // `.welcome { overflow: auto }` — в низком окне содержимое скроллится,
        // а не обрезается
        .overflow_y_scroll()
        // лого + мягкий glow (спрайт)
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .w(px(112.0))
                .h(px(112.0))
                .mb(px(m::SPACE_1))
                .child(
                    // glow 220×220 (::before оригинала), центр под лого 112
                    img(glow)
                        .absolute()
                        .left(px(-54.0))
                        .top(px(-54.0))
                        .w(px(220.0))
                        .h(px(220.0)),
                )
                .child(
                    img(SharedString::from("icons/kaminoid.svg"))
                        .relative()
                        .w(px(112.0))
                        .h(px(112.0)),
                ),
        )
        // заголовок
        .child(
            div()
                .text_size(px(38.4))
                // `letter-spacing: -0.02em` (`WelcomePlaceholder.module.css:53`)
                .letter_spacing(px(38.4 * -0.02)) // 2.4rem
                // `.title { line-height: 1.05 }` — дефолт gpui 1.169 давал
                // строку 44.9 вместо 40.3 (ревью ц.11)
                .line_height(px(38.4 * 1.05))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgba(p.text_primary))
                .child("KaminIDE"),
        )
        // Пилюля версии — ТОЛЬКО когда версия известна:
        // `{appVersion.value && <span…>}` (`WelcomePlaceholder.tsx:16`).
        // Пустая строка означает «ещё не пришла» (ревью ц.26)
        .children((!version.is_empty()).then(|| {
            div()
                .px(px(10.0))
                .py(px(2.0))
                .rounded(px(999.0))
                .bg(tint(rgba(p.accent_primary), 0.14))
                .text_size(px(m::FS_XS))
                // `font-variant-numeric: tabular-nums` (CSS:65) — цифры
                // версии не пляшут по ширине (ревью ц.26: пункт молча выпал
                // из «Осталось» после ц.13)
                .font(crate::ui::typo::tnum(gpui::FontWeight::NORMAL))
                .text_color(rgba(p.text_primary))
                .child(format!("v{version}"))
        }))
        // tagline
        .child(
            div()
                .max_w(px(480.0))
                .text_size(px(m::FS_MD))
                .line_height(px(m::FS_MD * 1.3)) // lh-snug
                .text_color(rgba(p.text_muted))
                .child(
                    "An AI-native workspace — run Claude sessions over your projects, with your \
                     files, terminal and tools in one place. Start a session to begin.",
                ),
        )
        // кнопки
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(m::SPACE_3))
                .justify_center()
                .mt(px(m::SPACE_2))
                .child(
                    div()
                        .id("welcome-folder")
                        .flex()
                        .items_center()
                        .gap(px(m::SPACE_2))
                        .px(px(m::SPACE_4))
                        .py(px(m::SPACE_2))
                        .rounded(px(m::RADIUS_SM))
                        .bg(rgba(p.accent_primary))
                        .text_size(px(m::FS_SM))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        // --accent-on-primary fallback #fff (welcome primary)
                        .text_color(gpui::white())
                        .cursor_pointer()
                        .hover({
                            // hover: accent 86% + чёрный (translateY нет в gpui)
                            let mut c = rgba(p.accent_primary);
                            c.r *= 0.86;
                            c.g *= 0.86;
                            c.b *= 0.86;
                            move |s| s.bg(c)
                        })
                        .on_mouse_down(gpui::MouseButton::Left, move |_, w, cx| on_folder(w, cx))
                        .child(fa(FOLDER_OPEN, 13.0))
                        .child("New session in folder…"),
                )
                .child(
                    div()
                        .id("welcome-empty")
                        .flex()
                        .items_center()
                        .gap(px(m::SPACE_2))
                        .px(px(m::SPACE_4))
                        .py(px(m::SPACE_2))
                        .rounded(px(m::RADIUS_SM))
                        .bg(tint(rgba(p.text_primary), 0.06))
                        .border_1()
                        // --divider-soft = text-primary 6% (ревью ц.1: было 14%)
                        .border_color(tint(rgba(p.text_primary), 0.06))
                        .text_size(px(m::FS_SM))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgba(p.text_primary))
                        .cursor_pointer()
                        .hover({
                            let hb = tint(rgba(p.text_primary), 0.12);
                            move |s| s.bg(hb)
                        })
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                            std::thread::spawn(move || {
                                if let Some(c) = host_link::client() {
                                    let _ = c.request("kamin:sessions:newNoFolderSession", vec![]);
                                }
                            });
                        })
                        .child(fa(PLUS, 13.0).font_family(FA_FAMILY))
                        .child("Empty session"),
                ),
        )
        // feature-чипы
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_x(px(m::SPACE_5))
                .gap_y(px(m::SPACE_2))
                .justify_center()
                .mt(px(m::SPACE_3))
                .max_w(px(544.0))
                .child(feature(COMMENTS, "Claude chat + tools", p))
                .child(feature(FOLDER_TREE, "Your files & editor", p))
                .child(feature(TERMINAL, "Integrated terminal", p)),
        )
        .into_any_element()
}
