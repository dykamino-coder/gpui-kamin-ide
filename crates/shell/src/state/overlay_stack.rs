//! Оверлеи В ГЛАВНОМ ОКНЕ (`plan/101-cef.md`, Ф6).
//!
//! Раньше меню, модалки, поповеры и тултипы жили во втором прозрачном окне
//! поверх главного — иначе их накрывали child-окна WebView2. Страницы теперь
//! рисует CEF прямо в кадр, и весь стек ложится обычным слоем ПОВЕРХ дерева
//! главного окна: ни синхронизации окон, ни hit-test-сабкласса, ни второй
//! строки в диспетчере задач.
//!
//! Слои те же и в том же порядке, что были в `OverlayWindow::render`:
//! passive (тултипы) → search (палитра/QuickOpen/FiF/symbols) → меню сессий →
//! контекст-меню → пикеры → модалка. Функции слоёв не менялись — они всегда
//! принимали `&RootView` и не знали, в каком окне живут.

use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Window, div};

impl RootView {
    /// Слой оверлеев поверх всего дерева главного окна.
    pub(crate) fn overlay_stack(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Пока идёт fadeIn модалки, просим следующий кадр: кадр рисуется по
        // событию, иначе анимация застынет на первом значении.
        let modal_fading = self.modal_at.is_some_and(|t| t.elapsed().as_millis() < 140);
        if modal_fading {
            cx.notify();
        }
        let modal_age_ms = self
            .modal_at
            .map(|t| t.elapsed().as_millis())
            .unwrap_or(u128::MAX);
        let p = self.theme.palette();
        let tx = self.tx.clone();
        // viewport_size отдаёт ЛОГИЧЕСКИЕ px (gpui::Pixels) — делить на
        // масштаб нельзя: слой считал окно в 1.25 раза меньше, и тосты
        // вставали к центру вместо правого-нижнего угла (скрин юзера).
        let vp = window.viewport_size();
        let vw = f32::from(vp.width);
        let vh = f32::from(vp.height);

        // Снимки состояний для стендов (раньше публиковало overlay-окно).
        crate::overlay::diag_states::publish_states(self, window);

        // Шрифт/метрика как у корня: слой строится отдельным поддеревом.
        let mut layer = div()
            .absolute()
            .top_0()
            .left_0()
            .w(gpui::px(vw))
            .h(gpui::px(vh))
            .font(gpui::Font {
                family: crate::root::UI_FONT.into(),
                features: gpui::FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)])),
                fallbacks: None,
                weight: gpui::FontWeight::NORMAL,
                style: gpui::FontStyle::Normal,
                // Ширина начертания у шрифтов оболочки обычная.
                stretch: gpui::FontStretch::Normal,
            })
            .text_color(crate::colors::rgba(p.text_primary))
            .line_height(gpui::relative(1.169));

        layer = crate::overlay::layers::passive::add_passive(layer, self, p, &tx, vw, vh, window);
        layer = crate::overlay::layers::search::add_search(layer, self, p, &tx, vw, vh, window, cx);
        layer = crate::overlay::layers::menus_session::add_menus_session(
            layer, self, p, &tx, vw, vh, window,
        );
        layer = crate::overlay::layers::menus_context::add_menus_context(
            layer, self, p, &tx, vw, vh, window,
        );
        layer =
            crate::overlay::layers::pickers::add_pickers(layer, self, p, &tx, vw, vh, window, cx);
        let modal_data = self.modal.clone();
        layer = self.add_modal(layer, modal_data, p, &tx, modal_age_ms, window, cx);
        // Живой тултип — ПОСЛЕДНИМ: подсказки элементов внутри поповеров/меню
        // должны ложиться поверх их карт (раньше жил в passive, первым слоем,
        // и layout-поповер накрывал собственный тултип — скрин юзера).
        if let Some((text, (mx, my))) = self.tooltip_live.clone() {
            layer = layer.child(crate::ui::tooltip::tooltip_box_at(&text, mx, my, window));
        }

        // deferred: слой рисуется ПОСЛЕ остального дерева — поверх страниц CEF
        // и всего контента, как раньше рисовало отдельное окно.
        gpui::deferred(layer).priority(200).into_any_element()
    }
}
