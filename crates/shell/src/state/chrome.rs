//! Хром окна: радиальный фон и титлбар.
//!
//! Куски цепочки `render` перенесены как есть (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::state::consts::CHIP_W_RESERVE;
use crate::state::model::RootView;
use crate::ui::session_tabs::session_tabs;
use crate::ui::titlebar::{TitlebarState, titlebar};
use gpui::{Bounds, Context, Window, px};
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn with_chrome<E>(
        &mut self,
        root: E,
        p: &'static Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
        viewport_w: f32,
    ) -> E
    where
        E: gpui::ParentElement,
    {
        root.child(self.radial_bg.layers(Bounds {
            origin: gpui::point(px(0.), px(0.)),
            size: window.viewport_size(),
        }))
        .child(titlebar(
            p,
            TitlebarState {
                sidebar_visible: self.sidebar_visible,
                sidebar_width: (self.layout.sidebar_width_px as f32).round(),
                layout_popover_open: self.layout_popover,
                customize_open: self.cz.customize_open,
                // sun f185 / moon f186 / circle-half-stroke f042.
                // Contributed ПЕРЕБИВАЕТ: sun/moon по её uiTheme; half —
                // только system БЕЗ contributed (ревью ц.1)
                theme_glyph: if self.contrib_theme_id.is_none() && self.theme_choice == "system" {
                    "\u{f042}"
                } else if self.theme == kamin_theme::ThemeKind::Light {
                    "\u{f185}"
                } else {
                    "\u{f186}"
                },
                on_toggle_sidebar: {
                    let tx = self.tx.clone();
                    Box::new(move || {
                        let _ = tx.try_send(ShellEvent::ToggleSidebar);
                    })
                },
                on_open_palette: {
                    let tx = self.tx.clone();
                    Box::new(move || {
                        let _ = tx.try_send(ShellEvent::TogglePalette);
                    })
                },
                tx: self.tx.clone(),
            },
            match &self
                .sessions
                .as_ref()
                // Ноль открытых чипов → `SessionTabs` возвращает `null`
                // целиком: ни стрипа, ни «+», ни спейсера
                .filter(|snap| {
                    !crate::ui::session_tabs::ordered_chips(&snap.sessions, &self.chip_order)
                        .is_empty()
                }) {
                Some(snap) => Some({
                    // Бюджет стрипа = ФАКТИЧЕСКАЯ ширина слота табов,
                    // снятая probe-областью на предыдущем кадре. Константа
                    // 580 занижала резерв на ~45 (реальный правый блок
                    // 624.8 = 86 левый кластер + 452.8 до края окна + 24 px
                    // слота + 38 «+» + 24 min спейсера, замер ц.9), поэтому
                    // «N ⌄» не появлялся вовсе, а последний чип клипался.
                    // Слот сам сообщает свою ширину — резерв больше не
                    // угадывается и не разъедется при смене кластеров.
                    let slot_w = crate::probe::registry::bounds_of("tabs-slot")
                        .map(|[_, _, w, _]| w - 24.0 - 38.0 - 24.0)
                        .unwrap_or(viewport_w - 624.8);
                    let tabs_w = slot_w.max(CHIP_W_RESERVE);
                    let tx = self.tx.clone();
                    session_tabs(
                        &snap.sessions,
                        snap.active_session_id.as_deref(),
                        &self.chip_order,
                        self.chip_drag
                            .as_ref()
                            .filter(|d| d.started)
                            .and_then(|d| d.over.as_deref()),
                        // .dndDragging: перетаскиваемый чип гасится до .4
                        self.chip_drag
                            .as_ref()
                            .filter(|d| d.started)
                            .map(|d| d.src.as_str()),
                        self.switching_to.as_deref(),
                        tabs_w,
                        self.tabs_overflow_open.is_some(),
                        {
                            let tx = tx.clone();
                            move |x, y| {
                                let _ = tx.try_send(ShellEvent::ToggleTabsOverflow(x, y));
                            }
                        },
                        &tx,
                        p,
                        window,
                    )
                }),
                None => None,
            },
            window,
            cx,
        ))
    }
}
