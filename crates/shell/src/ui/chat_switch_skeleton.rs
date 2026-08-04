//! Накрывашка переключения сессии (`ChatSwitchSkeleton.tsx` + `.module.css`,
//! элементы 72/76). Спокойное брендовое ожидание: марка KaminIDE с «дышащим»
//! свечением над тонкой бегущей полосой — вместо пустой панели.
//!
//! Ограничения gpui и обходы:
//! - нет `transform: scale` → «дыхание» свечения сделано анимацией РАЗМЕРА
//!   бокса (94 %→106 % от 150) с сохранением центра;
//! - нет `translateY` → «парение» логотипа сделано анимацией `top`;
//! - нет `filter: blur/drop-shadow` → свечение берётся из запечённого спрайта
//!   `radial_bg::glow_sprite`, тень логотипа не воспроизводится;
//! - бегунок полосы — абсолютный бокс, чей `left` едет от −100 % до +100 %,
//!   градиент собран из двух двухстоповых половин (у `linear_gradient` в gpui
//!   ровно два стопа).

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt as _, AnyElement, SharedString, div, img, linear_color_stop,
    linear_gradient, px,
};
use kamin_metrics as m;
use kamin_theme::Palette;
use std::time::Duration;

use crate::colors::{rgba, tint};

/// `kaminSwitchBreathe` / `kaminSwitchFloat` — 2.4s ease-in-out infinite.
const BREATHE_MS: u64 = 2400;
/// `kaminSwitchSweep` — 1.15s ease-in-out infinite.
const SWEEP_MS: u64 = 1150;
/// `.brand { width: 96px; height: 96px }`
const BRAND: f32 = 96.0;
/// `.glow { width: 150px; height: 150px }`
const GLOW: f32 = 150.0;
/// `.logo { width: 64px; height: 64px }`
const LOGO: f32 = 64.0;
/// `.bar { width: 180px; height: 3px }`
const BAR_W: f32 = 180.0;
pub(crate) const BAR_H: f32 = 3.0;

/// Полукруглая синусоида 0→1→0 для «дыхания» и «парения».
fn wave(delta: f32) -> f32 {
    (std::f32::consts::PI * delta).sin()
}

/// Шторка переключения чата — брендовый лоадер с подписью по умолчанию.
pub fn chat_switch_skeleton(p: &Palette) -> AnyElement {
    brand_loader(p, "Loading conversation…")
}

/// Единый брендовый лоадер (марка + «дышащее» свечение + бегущая полоса).
/// Используется шторкой чата И скелетом загрузки вебвью-панелей — загрузка
/// везде выглядит одинаково, а не зоопарком разных плейсхолдеров.
pub fn brand_loader(p: &Palette, caption: &'static str) -> AnyElement {
    let accent = rgba(p.accent_primary);
    let clear = tint(accent, 0.0);

    // `.glow`: радиальный градиент — ВЕКТОРНО, концентрическими полупрозрачными
    // кругами, БЕЗ запечённого спрайта. Спрайт живёт в общем атласе картинок, а
    // GPU-путь CEF копирует кадры вью в те же атласные текстуры и перезаписывал
    // регион спрайта чужим содержимым — вокруг логотипа рисовался квадрат с
    // куском другой панели вместо свечения.
    let glow = div()
        .absolute()
        .child({
            let mut stack = div().relative().size_full();
            // Много тонких колец с приращениями по smoothstep-профилю —
            // суммарная альфа к центру повторяет радиальный градиент без
            // видимых ступеней (3-4 круга читались как «кольца»).
            const RINGS: usize = 14;
            const PEAK: f32 = 0.26;
            let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
            let mut cum = 0.0f32;
            for i in 0..RINGS {
                let t = (i + 1) as f32 / RINGS as f32;
                let target = PEAK * smooth(t);
                // Поправка композитинга: кольцо добавляет ровно недостающую
                // до целевой суммарную альфу поверх уже наложенных.
                let a = ((target - cum) / (1.0 - cum)).max(0.0);
                cum = target;
                let k = 1.0 - 0.88 * t; // радиус от 100 % до 12 %
                let size = GLOW * k;
                let off = (GLOW - size) / 2.0;
                stack = stack.child(
                    div()
                        .absolute()
                        .left(px(off))
                        .top(px(off))
                        .w(px(size))
                        .h(px(size))
                        .rounded(px(size / 2.0))
                        .bg(tint(accent, a)),
                );
            }
            stack
        })
        .w(px(GLOW))
        .h(px(GLOW))
        .left(px((BRAND - GLOW) / 2.0))
        .top(px((BRAND - GLOW) / 2.0))
        .with_animation(
            "chat-switch-breathe",
            Animation::new(Duration::from_millis(BREATHE_MS))
                .repeat()
                .with_easing(gpui::ease_in_out),
            // «Дыхание» — только прозрачностью: кольца позиционированы
            // фиксированно и с изменением размера контейнера не масштабируются.
            |d, delta| d.opacity(0.5 + 0.5 * wave(delta)),
        );

    // `.logo` — 64×64, «парит» на 4 px вверх и обратно
    let logo = div()
        .absolute()
        .w(px(LOGO))
        .h(px(LOGO))
        .left(px((BRAND - LOGO) / 2.0))
        .child(
            img(SharedString::from("icons/kaminoid.svg"))
                .w(px(LOGO))
                .h(px(LOGO)),
        )
        .with_animation(
            "chat-switch-float",
            Animation::new(Duration::from_millis(BREATHE_MS))
                .repeat()
                .with_easing(gpui::ease_in_out),
            |d, delta| d.top(px((BRAND - LOGO) / 2.0 - 4.0 * wave(delta))),
        );

    // `.barFill`: три стопа (transparent → accent → transparent) двумя половинами
    let sweep = div()
        .absolute()
        .top_0()
        .h_full()
        .w(gpui::relative(1.0))
        .flex()
        .child(div().h_full().flex_1().bg(linear_gradient(
            90.,
            linear_color_stop(clear, 0.),
            linear_color_stop(accent, 1.),
        )))
        .child(div().h_full().flex_1().bg(linear_gradient(
            90.,
            linear_color_stop(accent, 0.),
            linear_color_stop(clear, 1.),
        )))
        .with_animation(
            "chat-switch-sweep",
            Animation::new(Duration::from_millis(SWEEP_MS))
                .repeat()
                .with_easing(gpui::ease_in_out),
            |d, delta| d.left(gpui::relative(-1.0 + 2.0 * delta)),
        );

    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(18.0))
        .p(px(24.0))
        .overflow_hidden()
        // Шторка лежит поверх области вебвью — у той скруглённые углы
        // (RADIUS_MD, см. webview_body); без своего скругления её прямые
        // углы торчали за карточку панели.
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.editor_bg))
        .child(
            // `.brand` — 96×96, свечение позади марки и вне потока
            div()
                .relative()
                .w(px(BRAND))
                .h(px(BRAND))
                .child(glow)
                .child(logo),
        )
        .child(
            // `.caption` — fs-sm, text-muted, трекинг .01em
            div()
                .text_size(px(m::FS_SM))
                .letter_spacing(px(m::FS_SM * 0.01))
                .text_color(rgba(p.text_muted))
                .child(caption),
        )
        .child(
            // `.bar` — 180×3, r 999, подложка text-primary 8 %
            div()
                .relative()
                .w(px(BAR_W))
                .h(px(BAR_H))
                .rounded(px(999.0))
                .overflow_hidden()
                .bg(tint(rgba(p.text_primary), 0.08))
                .child(sweep),
        )
        .into_any_element()
}
