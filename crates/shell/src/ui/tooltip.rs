//! Тултип (Tooltip.module.css 1:1): bg-surface, text-primary, padding 4 8,
//! radius-xs 4, fs-xs 11, lh 1.3, shadow-mini (0 2 8 rgba .3), nowrap ellipsis,
//! max-width 640. gpui сам показывает по ховеру с задержкой; позиционируем
//! ЦЕНТРОМ под курсором (курсор ~центр иконки → визуально по центру элемента,
//! как document-tooltip оригинала).

use gpui::AnimationExt as _;
use gpui::prelude::*;
use gpui::{App, SharedString, Window, div, px};
use kamin_metrics as m;

use crate::colors::rgba;

/// Высота бокса: lh(FS_XS*1.3) + py 4×2.
fn box_h() -> f32 {
    m::FS_XS * 1.3 + 8.0
}

/// View тултипа с точными стилями оригинала.
pub struct KaminTooltip {
    text: SharedString,
}

impl KaminTooltip {
    fn shadow() -> Vec<gpui::BoxShadow> {
        // `--shadow-mini` из словаря: в светлой теме он ink-tinted, а не
        // чёрный .3 (ревью ц.11)
        crate::ui::shadows::mini()
    }
}

impl Drop for KaminTooltip {
    fn drop(&mut self) {
        // gpui скрыл тултип → погасить overlay-копию
        if let Some(tx) = crate::host_link::event_tx() {
            let _ = tx.try_send(crate::host_link::ShellEvent::TooltipHide);
        }
    }
}

/// Стилизованный бокс тултипа по абсолютной позиции (для overlay-слоя).
pub fn tooltip_box_at(
    text: &SharedString,
    x: f32,
    y: f32,
    window: &mut Window,
) -> gpui::AnyElement {
    let p = kamin_theme::current_palette();
    let half = half_width(text, window);
    // Над элементом (курсор ~центр) + clamp к вьюпорту, как оригинал
    let vw = f32::from(window.viewport_size().width);
    // `VIEWPORT_GUTTER_PX = 8` (`clamp-popup.ts:18`) — было 4 (ревью ц.14)
    let left = (x - half).clamp(8.0, (vw - 2.0 * half - 8.0).max(8.0));
    // Над курсором; если сверху не влезает (титлбар — самый верх окна),
    // ПЕРЕВОРАЧИВАЕМ вниз, а не прижимаем к краю: прижатый тултип ложился
    // прямо на кнопку и закрывал её (баг найден юзером). У оригинала это
    // делает `clamp-popup.ts` — flip стороны при нехватке места.
    let above = y - 14.0 - box_h();
    let top = if above < 8.0 { y + 20.0 } else { above };
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .bg(rgba(p.bg_surface))
        .text_color(rgba(p.text_primary))
        .px(px(m::SPACE_2))
        .py(px(4.0))
        .rounded(px(m::RADIUS_XS))
        .text_size(px(m::FS_XS))
        .line_height(px(m::FS_XS * 1.3))
        .flex()
        .shadow(KaminTooltip::shadow())
        // Ширина — ПО КОНТЕНТУ; `max-width` и эллипсис живут на самом тексте.
        // Когда всё это висело на абсолютной коробке, она схлопывалась и от
        // подписи оставалось одно «…» (баг, пойман юзером)
        .child(
            div()
                .max_w(px(640.0_f32.min(vw - 16.0).max(80.0)))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(text.clone()),
        )
        // Fade-in 100мс (оригинал: transition opacity .1s). Стейт анимации
        // живёт, пока элемент в кадре: скрытие тултипа сбрасывает его, и
        // следующий показ снова начинает с 0. Под RDP fade = лишние кадры в
        // сеть на каждый ховер («тултипы тормозили») — показываем мгновенно.
        .map(|d| {
            if crate::win_integration::reduce_motion() {
                d.into_any_element()
            } else {
                d.with_animation(
                    "tooltip-fade",
                    gpui::Animation::new(std::time::Duration::from_millis(100)),
                    |d, delta| d.opacity(delta),
                )
                .into_any_element()
            }
        })
}

fn half_width(text: &SharedString, window: &mut Window) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: gpui::Font {
            family: crate::root::UI_FONT.into(),
            features: gpui::FontFeatures::default(),
            fallbacks: None,
            weight: gpui::FontWeight::NORMAL,
            style: gpui::FontStyle::Normal,
            stretch: gpui::FontStretch::Normal,
        },
        color: gpui::black(),
        background_color: None,
        // Поля строчного бокса вокруг фона прогона: здесь их нет.
        font_size: None,
        background_pad: Default::default(),
        background_radius: Default::default(),
        background_border: None,
        underline: None,
        strikethrough: None,
    };
    let w = f32::from(
        window
            .text_system()
            .shape_line(text.clone(), px(m::FS_XS), &[run], None)
            .width,
    );
    (w.min(640.0) + 2.0 * m::SPACE_2) / 2.0
}

impl Render for KaminTooltip {
    fn render(&mut self, window: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        // Ф6: окно одно, и страницы CEF живут В кадре — тултип по-прежнему
        // рисует passive-слой стека (поверх всего), сюда лишь публикуем текст.
        let is_overlay = false;
        if !is_overlay {
            if let Some(tx) = crate::host_link::event_tx() {
                // Позиция мыши ЭТОГО окна в момент показа (last_mouse root
                // запаздывал при SendInput-телепорте — тултип улетал)
                let pos = window.mouse_position();
                let _ = tx.try_send(crate::host_link::ShellEvent::TooltipShow(
                    self.text.to_string(),
                    f32::from(pos.x),
                    f32::from(pos.y),
                ));
            }
            return div().into_any_element();
        }
        let p = kamin_theme::current_palette();
        // Ширина текста шейпером → бокс absolute со сдвигом -w/2 от курсора.
        // margin/padding-трюки не годятся: margin корня игнорируется
        // layout_as_root, паддинг не бывает отрицательным.
        let run = gpui::TextRun {
            len: self.text.len(),
            font: gpui::Font {
                family: crate::root::UI_FONT.into(),
                features: gpui::FontFeatures::default(),
                fallbacks: None,
                weight: gpui::FontWeight::NORMAL,
                style: gpui::FontStyle::Normal,
                // Ширина начертания у шрифтов оболочки обычная.
                stretch: gpui::FontStretch::Normal,
            },
            color: gpui::black(),
            background_color: None,
            // Поля строчного бокса вокруг фона прогона: здесь их нет.
            font_size: None,
            background_pad: Default::default(),
            background_radius: Default::default(),
            background_border: None,
            underline: None,
            strikethrough: None,
        };
        let text_w = f32::from(
            window
                .text_system()
                .shape_line(self.text.clone(), px(m::FS_XS), &[run], None)
                .width,
        );
        let half = (text_w.min(640.0) + 2.0 * m::SPACE_2) / 2.0;
        div()
            .relative()
            .child(
                div()
                    .absolute()
                    .left(px(-half))
                    .top(px(-(box_h() + 14.0)))
                    // Регион на САМ бокс (легаси region-режим): обёртка в регионе
                    // рисовалась чёрной плитой
                    .child(crate::overlay::tooltip_region())
                    .bg(rgba(p.bg_surface))
                    .text_color(rgba(p.text_primary))
                    .px(px(m::SPACE_2))
                    .py(px(4.0))
                    .rounded(px(m::RADIUS_XS))
                    .text_size(px(m::FS_XS))
                    .line_height(px(m::FS_XS * 1.3))
                    .flex()
                    .shadow(Self::shadow())
                    // Ограничение ширины и эллипсис — на ТЕКСТЕ, не на самой
                    // абсолютной коробке: на коробке они схлопывали её, и от
                    // подписи оставалось одно «…» (баг, пойман юзером; в
                    // overlay-пути починено тем же приёмом)
                    .child(
                        div()
                            .max_w(px(640.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(self.text.clone()),
                    ),
            )
            .into_any_element()
    }
}

/// Хелпер: `.child(...)`-совместимый билдер тултипа для `.tooltip()`.
pub fn tooltip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView {
    let text: SharedString = text.into();
    move |_, cx| {
        // Во время драга ручек тултипы молчат (флаг вместо блокера ввода).
        let text = if crate::state::drag_flag::active() {
            SharedString::default()
        } else {
            text.clone()
        };
        cx.new(|_| KaminTooltip { text }).into()
    }
}
