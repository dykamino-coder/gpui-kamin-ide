//! Метрики кадра: размеры вьюпорта и производные ширины колонок.
//!
//! Блок вынесен из `render` как есть (`plan/100-refactor-250.md`): те же
//! вычисления в том же порядке, включая побочные эффекты (масштабирование
//! колонок при смене ширины окна и починку инварианта слотов).

use crate::state::model::RootView;
use gpui::Window;
use kamin_metrics as m;

/// Размеры кадра, от которых зависит раскладка.
pub(crate) struct Metrics {
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub sidebar_w: f32,
    pub file_w: f32,
    pub right_w: f32,
    pub main_split: f32,
    pub file_bottom_px: f32,
}

impl RootView {
    pub(crate) fn frame_metrics(&mut self, window: &mut Window) -> Metrics {
        let viewport_w = f32::from(window.viewport_size().width);
        let viewport_h = f32::from(window.viewport_size().height);
        self.main_viewport = (viewport_w, viewport_h);
        // Инвариант «active ∈ pinned» по всем слотам: без него застрявший в
        // persist чужой id рисовал заглушку несуществующего тула
        for slot in crate::activity::PanelSlot::ALL {
            self.activity.repair(slot);
        }
        // Активный тул сайдбара живёт в МОДЕЛИ: перенос drag'ом и пин из
        // пикера меняют её напрямую, а подсветка плитки и резолвер тела
        // читали отдельное поле — после дропа тело переключалось на
        // принесённый тул, а подсвеченной оставалась прежняя плитка.
        if let Some(active) = self
            .activity
            .state(crate::activity::PanelSlot::Sidebar)
            .active
            .clone()
        {
            let id = crate::activity::static_id(&active);
            if id != self.sidebar_activity {
                self.sidebar_activity = id;
            }
        }
        // Контентные ширины из persist-снапшота (ratio-математика 1:1).
        // .round(): дробные px из ratio дают субпиксельное «дёрганье» соседних
        // грипов при драге.
        let sidebar_w = (self.layout.sidebar_width_px as f32).round();
        // Ресайз ОКНА → viewport adapter (`layout-autosave.ts:176-206`).
        // Пол вьюпорта: свёрнутое окно рапортует крошечные размеры (в
        // WebView2 ~146px), и обработка такого кадра схлопывает размеры из
        // ratio и ОТРАВЛЯЕТ базу фактора — на восстановлении колонки
        // взрываются. Ниже пола адаптер не работает вовсе
        let settled = match self.viewport_settle {
            // Ширина ещё меняется — перезапускаем отсчёт
            Some((w, _)) if (w - viewport_w).abs() > 0.5 => {
                self.viewport_settle = Some((viewport_w, std::time::Instant::now()));
                window.request_animation_frame();
                false
            }
            Some((_, at)) => {
                let done = at.elapsed() >= m::RESIZE_SETTLE;
                if !done {
                    // Кадров после остановки ресайза может не быть вовсе —
                    // сами просим следующий, иначе «устоявшийся» размер
                    // никогда не наступит
                    window.request_animation_frame();
                }
                done
            }
            None => {
                self.viewport_settle = Some((viewport_w, std::time::Instant::now()));
                window.request_animation_frame();
                false
            }
        };
        if settled
            && viewport_w >= m::VIEWPORT_MIN_FLOOR_W
            && viewport_h >= m::VIEWPORT_MIN_FLOOR_H
            && (viewport_w - self.last_viewport_w).abs() > 0.5
        {
            // Сайдбар И правая колонка масштабируются фактором окна
            // (`max(min, round(px · factor))`); клампа «0.6 вьюпорта» у
            // оригинала нет ни у одной из них (ревью ц.35)
            if self.last_viewport_w > 1.0 {
                let factor = viewport_w / self.last_viewport_w;
                let sb = (self.layout.sidebar_width_px as f32 * factor)
                    .round()
                    .max(m::SIDEBAR_MIN_WIDTH);
                self.layout.sidebar_width_px = f64::from(sb);
                let rp = (self.layout.right_panel_width_px as f32 * factor)
                    .round()
                    .max(m::PANEL_MIN_SIZE);
                self.layout.right_panel_width_px = f64::from(rp);
                // Масштаб — ТОЛЬКО в памяти. Персист масштабированных ширин
                // (бывшая «находка A») хратил их с полами/округлением и на
                // каждом цикле бут→максимизация дожимал пропорции («лейаут
                // сбросился/плющит» — жалобы юзера). Диск хранит канонические
                // значения (end_drag), а бут теперь открывается в прежних
                // габаритах окна (windowBounds ниже) — рассинхрона нет.
            }
            self.last_viewport_w = viewport_w;
            self.file_w_live = None;
            // Габариты окна тоже персистим: бут открывался ДЕФОЛТНЫМ
            // размером, фактор-масштаб выше пересчитывал и ЗАПИСЫВАЛ
            // урезанные ширины, а краш до максимизации оставлял их на
            // диске навсегда (корень «лейаут сбросился на дерьмо»).
            // Рестор в main.rs: бут в прежнем размере → factor 1.
            let (b, maxed) = match window.window_bounds() {
                gpui::WindowBounds::Maximized(b) => (b, true),
                gpui::WindowBounds::Fullscreen(b) => (b, true),
                gpui::WindowBounds::Windowed(b) => (b, false),
            };
            crate::layout_store::save_patch(serde_json::json!({
                "windowBounds": {
                    "x": f32::from(b.origin.x),
                    "y": f32::from(b.origin.y),
                    "w": f32::from(b.size.width),
                    "h": f32::from(b.size.height),
                    "maximized": maxed,
                }
            }));
        }
        let file_w = self
            .file_w_live
            .unwrap_or_else(|| {
                m::layout_math::width_from_ratio(
                    self.layout.file_panel_width_ratio as f32,
                    m::PANEL_MIN_SIZE,
                    viewport_w,
                )
            })
            .round();
        let right_w = (self.layout.right_panel_width_px as f32).round();
        let main_split =
            (self.layout.main_split as f32).clamp(m::MAIN_SPLIT_MIN, m::MAIN_SPLIT_MAX);
        let bottom_ratio = (self.layout.file_panel_bottom_height_ratio as f32)
            .clamp(m::BOTTOM_RATIO_MIN, m::BOTTOM_RATIO_MAX);
        // Ведущий размер — ВЫСОТА НИЖНЕЙ КАРТЫ в пикселях, как у оригинала
        // (`FilePanel.tsx:145`: `max(100, round(ratio · innerHeight))`),
        // а верх получает остаток. Доля от body_h давала недобор ~37 px
        // (замер ц.19) и минимум 83 вместо 100.
        let file_bottom_px = (bottom_ratio * viewport_h)
            .round()
            .max(m::BOTTOM_PANE_MIN_HEIGHT)
            .min(
                (viewport_h
                    - m::TITLEBAR_HEIGHT
                    - m::STATUS_BAR_HEIGHT
                    - m::BOTTOM_PANE_MIN_HEIGHT)
                    .max(m::BOTTOM_PANE_MIN_HEIGHT),
            );

        // Нет активной сессии → welcome ВНУТРИ main-карты (чат без сессии),
        Metrics {
            viewport_w,
            viewport_h,
            sidebar_w,
            file_w,
            right_w,
            main_split,
            file_bottom_px,
        }
    }
}
