//! Карточка внешнего тоста — порт `toast-card.ts` (`buildToastCard`).
//!
//! Ряд «точка + заголовок + бейдж +N + ×», сообщение (3 строки), ряд кнопок
//! справа и полоса обратного отсчёта внизу: 2px акцентом, `shrink 8000ms
//! linear`, `#card:hover #bar { animation-play-state: paused }` — ховер
//! останавливает И полосу, И таймер закрытия.

use crate::ui::toast_metrics::{
    BAR_H, BODY_PAD, CARD_ALPHA, CARD_ALPHA_HOVER, CARD_GAP, CARD_PAD_X, CARD_PAD_Y, CARD_RADIUS,
    DOT, action_btn,
};
use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{Context, Window, div, px};
use kamin_metrics as m;

use crate::colors::{rgba, tint};
use crate::toast::{AUTO_DISMISS, ToastOpts};

pub struct ToastView {
    id: u64,
    opts: ToastOpts,
    /// «+N» очереди: показывает только ВЕРХНИЙ тост стопки.
    pub overflow: usize,
    /// Накопленное НЕ-паузное время: ховер его не наращивает.
    elapsed: Duration,
    last_tick: Option<Instant>,
    hovered: bool,
    /// HWND окна: ховер определяем ОПРОСОМ курсора, а не `on_hover`
    /// (см. `toast::cursor_over` — уход указателя события не даёт).
    hwnd: isize,
}

impl ToastView {
    pub fn new(id: u64, opts: ToastOpts, window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            id,
            opts,
            overflow: 0,
            elapsed: Duration::ZERO,
            last_tick: Some(crate::toast::now()),
            hovered: false,
            hwnd: crate::toast::raw_hwnd(window),
        }
    }

    /// Дотикать до текущего момента, если не на паузе.
    fn advance(&mut self) {
        let now = crate::toast::now();
        if let Some(last) = self.last_tick
            && !self.hovered
        {
            self.elapsed += now.duration_since(last);
        }
        self.last_tick = Some(now);
    }
}

impl Render for ToastView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Таймер и полоса живут НА КАДРАХ окна: таск-тикер на idle-приложении
        // просыпался раз в ~350 мс, и полоса дёргалась. `request_animation_frame`
        // держит ровный кадр, пока тост жив
        if !self.opts.sticky {
            self.hovered = crate::toast::cursor_over(self.hwnd);
            self.advance();
            if self.elapsed >= AUTO_DISMISS {
                let id = self.id;
                cx.defer(move |cx| crate::toast::close(id, cx));
            } else {
                window.request_animation_frame();
            }
        }
        let p = kamin_theme::current_palette();
        let accent = self.opts.kind.accent(p);
        let id = self.id;
        // `#card` / `#card:hover` — фон bg-primary 96 % → bg-surface 98 %
        let bg = if self.hovered {
            tint(rgba(p.bg_surface), CARD_ALPHA_HOVER)
        } else {
            tint(rgba(p.bg_primary), CARD_ALPHA)
        };
        // Полоса «сжимается» слева направо: остаток времени = ширина
        let left = 1.0 - crate::toast::progress(self.elapsed);

        let mut actions = div()
            .flex()
            .gap(px(CARD_GAP))
            .justify_end()
            // `dismissOnly` тоже рисует «Dismiss» — ряд не пустой никогда
            .child(
                action_btn("Dismiss".into(), false, accent, p).on_click(cx.listener(
                    move |_, _, _, cx| {
                        cx.defer(move |cx| crate::toast::close(id, cx));
                    },
                )),
            );
        let last = self.opts.actions.len().saturating_sub(1);
        for (i, label) in self.opts.actions.iter().enumerate() {
            actions = actions.child(action_btn(label.clone(), i == last, accent, p).on_click(
                cx.listener(move |_, _, _, cx| {
                    // Клик по действию отдаёт фокус главному окну
                    crate::toast::focus_main();
                    cx.defer(move |cx| crate::toast::close(id, cx));
                }),
            ));
        }

        div()
            .size_full()
            .p(px(BODY_PAD))
            .child(
                div()
                    .id("toast-card")
                    .size_full()
                    .relative()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .gap(px(CARD_GAP))
                    .px(px(CARD_PAD_X))
                    .py(px(CARD_PAD_Y))
                    .rounded(px(CARD_RADIUS))
                    .border_1()
                    .border_color(accent)
                    .bg(bg)
                    .shadow(crate::ui::shadows::card_popup())
                    .text_size(px(13.0))
                    .text_color(rgba(p.text_primary))
                    // `.row`
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(m::SPACE_2))
                            .child(
                                div()
                                    .w(px(DOT))
                                    .h(px(DOT))
                                    .rounded_full()
                                    .flex_shrink_0()
                                    .bg(accent),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(self.opts.title.clone()),
                            )
                            // `.badge { display: none }` пока очередь пуста
                            .when(self.overflow > 0, |d| {
                                d.child(
                                    div()
                                        .flex_shrink_0()
                                        .px(px(7.0))
                                        .py(px(2.0))
                                        .rounded(px(10.0))
                                        .bg(accent)
                                        .text_size(px(10.0))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgba(p.accent_action_fg))
                                        .child(format!("+{}", self.overflow)),
                                )
                            })
                            .child(
                                div()
                                    .id("toast-close")
                                    .flex_shrink_0()
                                    .px(px(m::SPACE_2))
                                    .py(px(m::SPACE_1))
                                    .rounded(px(4.0))
                                    .text_size(px(16.0))
                                    .text_color(rgba(p.text_muted))
                                    .cursor_pointer()
                                    .hover({
                                        let hb = rgba(p.bg_surface);
                                        let hf = rgba(p.text_primary);
                                        move |s| s.bg(hb).text_color(hf)
                                    })
                                    .child("×")
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.defer(move |cx| crate::toast::close(id, cx));
                                    })),
                            ),
                    )
                    // `.msg` — 12px text-secondary, три строки, lh 1.3
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_hidden()
                            .text_size(px(12.0))
                            .line_height(px(12.0 * 1.3))
                            .text_color(rgba(p.text_secondary))
                            .child(self.opts.message.clone()),
                    )
                    .child(actions)
                    // `#bar` — только у НЕ-sticky
                    .when(!self.opts.sticky, |d| {
                        d.child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .h(px(BAR_H))
                                .w(gpui::relative(left))
                                .bg(accent),
                        )
                    }),
            )
            .into_any_element()
    }
}
