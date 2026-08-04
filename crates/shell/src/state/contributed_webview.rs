//! Секция contributed-вебвью (`WebviewView` расширения) внутри панели:
//! создание вью, крышка-скелет, ошибка загрузки и повтор.
//!
//! Вынесено из `root.rs` без изменения поведения.

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_theme::Palette;

use crate::colors::rgba;
use crate::host_link;
use crate::root::{RootView, webview_body_dyn};

impl RootView {
    /// Секция contributed вебвью-вью: хедер + якорь вебвью (или скелет,
    /// пока провайдер не отдал html).
    pub(crate) fn contributed_webview_section(
        &mut self,
        d: &crate::activity::DynView,
        p: &'static Palette,
    ) -> AnyElement {
        // Оригинал снимает скелет, как только пришёл html
        // (`ContributedContainerBody.tsx:87`, `hasHtml`). Ждать сообщения ОТ
        // страницы нельзя: обычное contributed-вью нашу шину не дёргает и
        // висело в скелете до «This panel didn't load».
        let alive =
            self.webviews_alive.contains(d.id.as_str()) || crate::ui::chat_webview::has_html(&d.id);
        // Оригинал (`ContributedContainerBody.tsx:25-28`): backoff
        // 350ms × 1.5 до 3s, до 45 попыток (~2 мин). Плоские 5s давали
        // «Connecting to session…» на полминуты — шесть пустых кругов.
        if !alive {
            self.view_resolve_start
                .entry(d.id.clone())
                .or_insert_with(std::time::Instant::now);
            let tries = self.view_resolve_tries.get(&d.id).copied().unwrap_or(0);
            let delay_ms = (350.0_f32 * 1.5_f32.powi(tries as i32)).min(3000.0) as u128;
            let due = self
                .view_resolve_at
                .get(&d.id)
                .is_none_or(|t| t.elapsed().as_millis() >= delay_ms);
            if tries < 45 && due {
                self.view_resolve_at
                    .insert(d.id.clone(), std::time::Instant::now());
                self.view_resolve_tries.insert(d.id.clone(), tries + 1);
                host_link::resolve_webview(d.id.clone());
            }
        } else {
            self.view_resolve_tries.remove(&d.id);
            self.view_resolve_start.remove(&d.id);
        }
        // Состояние ожидания для скелета/ошибки: (секунды, попытка,
        // бюджет исчерпан). Пока вью жив — None, тело рисует вебвью.
        let load = if alive {
            None
        } else {
            let secs = self
                .view_resolve_start
                .get(&d.id)
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);
            let tries = self.view_resolve_tries.get(&d.id).copied().unwrap_or(0);
            Some((secs, tries, tries >= 45))
        };
        // Шторка переключения чата: грейс → 1.0 → фейд 140 мс → снята.
        // Форс-снятие по `COVER_TIMEOUT_MS = 2500`, как в оригинале.
        // Грейс: ТЁПЛАЯ сессия подтверждается (bridgeShowing false→true)
        // за доли секунды — шторка не показывается вовсе, переключение
        // выглядит мгновенным. Холодная не успевает — шторка как раньше.
        const COVER_TIMEOUT_MS: u128 = 2500;
        const COVER_FADE_MS: f32 = 140.0;
        const COVER_GRACE_MS: u128 = 250;
        let chat_cover_opacity = self.chat_cover.and_then(|(started, cleared)| {
            let cleared = cleared.or_else(|| {
                (started.elapsed().as_millis() >= COVER_TIMEOUT_MS).then(std::time::Instant::now)
            });
            match cleared {
                None if started.elapsed().as_millis() < COVER_GRACE_MS => None,
                None => Some(1.0_f32),
                Some(t) => {
                    // Подтвердилось внутри грейса — шторки не было, гаснуть нечему.
                    if t.duration_since(started).as_millis() < COVER_GRACE_MS {
                        None
                    } else {
                        let k = t.elapsed().as_millis() as f32 / COVER_FADE_MS;
                        (k < 1.0).then_some(1.0 - k)
                    }
                }
            }
        });
        // Крышка: непрозрачна до готовности, потом фейд 180 мс
        // (`.loader` → `.loaderHidden`)
        const READY_FALLBACK_MS: u128 = 1200;
        const FADE_MS: f32 = 180.0;
        // `LOAD_WATCHDOG_MS`: html есть, а вью так и не ожил — вместо крышки
        // карточка Retry (`WebviewPanelView.tsx:26-31`)
        const LOAD_WATCHDOG_MS: u128 = 20_000;
        let mut stalled = false;
        let cover = if crate::ui::chat_webview::has_html(&d.id) {
            let pinged = self.webviews_alive.contains(d.id.as_str());
            let e = self
                .webview_cover
                .entry(d.id.clone())
                .or_insert_with(|| (std::time::Instant::now(), None));
            if e.1.is_none() && (pinged || e.0.elapsed().as_millis() >= READY_FALLBACK_MS) {
                e.1 = Some(std::time::Instant::now());
            }
            stalled = e.1.is_none() && e.0.elapsed().as_millis() >= LOAD_WATCHDOG_MS;
            match e.1 {
                None => Some(1.0_f32),
                Some(t) => {
                    let k = t.elapsed().as_millis() as f32 / FADE_MS;
                    if k < 1.0 { Some(1.0 - k) } else { None }
                }
            }
        } else {
            None
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h(px(0.))
            .child(self.contrib_view_header(d, p))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(webview_body_dyn(
                        d.id.clone(),
                        alive,
                        ("Tool", crate::ui::panel_placeholder::SlotIcon::RightTop),
                        load,
                        &self.tx,
                        p,
                    ))
                    // `.loader`: крышка поверх вебвью, пока он не
                    // отрисовался (спиннер 22 на bg-surface)
                    .when(stalled, |c| {
                        c.child(
                            div()
                                .absolute()
                                .inset_0()
                                .bg(rgba(p.bg_surface))
                                .child(crate::ui::webview_skeleton::load_error(&d.id, &self.tx, p)),
                        )
                    })
                    .when_some(cover.filter(|_| !stalled), |c, opacity| {
                        c.child(
                            div()
                                .absolute()
                                // Те же отступы и радиус, что у готового вью
                                // (webview_body: p(8) + RADIUS_MD) — крышка
                                // выглядит как карточка, а не заливка в края.
                                .inset(px(8.0))
                                .rounded(px(kamin_metrics::RADIUS_MD))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgba(p.bg_surface))
                                .opacity(opacity)
                                .child(crate::ui::icon::spinner_ring(
                                    gpui::SharedString::from(format!("wvspin:{}", d.id)),
                                    22.0,
                                    {
                                        let mut c = rgba(p.text_primary);
                                        c.a = 0.16;
                                        c
                                    },
                                    rgba(p.accent_action),
                                )),
                        )
                    })
                    // Накрывашка переключения сессии поверх ЧАТА
                    // (`chat-switch-cover` оригинала): пока сессия
                    // переключается, вместо чужой переписки видно
                    // брендовое ожидание (элементы 72/76)
                    .when_some(
                        if d.id == crate::ui::chat_webview::CHAT_VIEW_ID {
                            chat_cover_opacity
                        } else {
                            None
                        },
                        |c, opacity| {
                            c.child(
                                // Отступы как у готового вью (webview_body p(8)):
                                // шторка — карточка, а не заливка до краёв панели.
                                div().absolute().inset(px(8.0)).opacity(opacity).child(
                                    crate::ui::chat_switch_skeleton::chat_switch_skeleton(p),
                                ),
                            )
                        },
                    ),
            )
            .into_any_element()
    }
}
