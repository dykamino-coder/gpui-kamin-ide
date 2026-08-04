//! Область Customize: панель на всю рабочую зону (колонки скрыты).
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::state::model::RootView;
use crate::ui::webview_body::gap_wrap;
use gpui::prelude::*;
use gpui::{IntoElement, div, px};
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn with_customize_area<E>(
        &mut self,
        body: E,
        p: &'static Palette,
        log_filter_focused: bool,
        design_input_focused: bool,
    ) -> E
    where
        E: gpui::ParentElement + gpui::Styled + gpui::prelude::Element,
    {
        body.when(self.cz.customize_open, |body| {
            // Customize: панель на всю область (колонки скрыты,
            // раскладка не трогается — восстановится по выходу)
            body.child(
                div().flex_1().min_w(px(0.)).h_full().child(gap_wrap(
                    crate::ui::glint::glint_surface_wv_holed(
                        p,
                        if self.cz.customize_panel == "contrib"
                            && !self.cz.customize_pages.iter().any(|c| {
                                c.views.iter().any(|v| {
                                    Some(v.id.as_str()) == self.cz.customize_contrib.as_deref()
                                })
                            })
                        {
                            // `ContributedViewBody`: страницы с таким
                            // id в реестре нет (расширение выгрузили)
                            // → «No view», а не пустая карточка
                            crate::ui::panel_placeholder::activity_placeholder(
                                "circle-large",
                                "No view",
                                p,
                            )
                        } else if self.cz.customize_panel == "contrib" {
                            // Имя вью для `h1` (`CustomizePanel.tsx:34`)
                            let contrib_name = self
                                .cz
                                .customize_pages
                                .iter()
                                .flat_map(|c| &c.views)
                                .find(|v| {
                                    Some(v.id.as_str()) == self.cz.customize_contrib.as_deref()
                                })
                                .map(|v| v.name.clone())
                                .unwrap_or_default();
                            // Contributed-страница живёт в общем вью
                            // `czShared`; готовность определяем по HTML.
                            let contrib_ready = self
                                .cz
                                .customize_contrib
                                .as_deref()
                                .is_some_and(crate::ui::chat_webview::has_html);
                            let contrib_body = if contrib_ready {
                                // Вью страницы живёт под СВОИМ id: бандл
                                // Customize выбирает раздел из location.pathname,
                                // а общий /czShared раздела не содержит — вью
                                // не поднимался и страница была пустой. Ensure
                                // идемпотентен (как в webview_body_dyn).
                                let contrib_id = crate::ui::webview_body::static_view_id(
                                    self.cz.customize_contrib.as_deref().unwrap_or_default(),
                                );
                                #[cfg(windows)]
                                if crate::web::enabled() {
                                    crate::web::ensure_html_view(contrib_id);
                                }
                                // Страницу рисует CEF: обычный элемент кадра,
                                // без дыры в фоне. Край в край, без инсетов и
                                // радиуса — страница красит себя сама.
                                div()
                                    .id("cz-contrib")
                                    .relative()
                                    .size_full()
                                    .child(crate::probe::registry::probe_area("cz-contrib"))
                                    // Радиус как у остальных вью (нижние углы
                                    // страницы срезались прямыми).
                                    .child(crate::web::web_view(
                                        contrib_id,
                                        kamin_metrics::RADIUS_MD,
                                    ))
                                    .into_any_element()
                            } else {
                                // Пока вью не отдал HTML — тот же скелет
                                // загрузки, что и в панелях: реальные секунды
                                // и попытки, иначе подписи не появятся вовсе.
                                let id = self.cz.customize_contrib.clone().unwrap_or_default();
                                let secs = self
                                    .view_resolve_start
                                    .get(&id)
                                    .map(|t| t.elapsed().as_secs())
                                    .unwrap_or(0);
                                let tries = self.view_resolve_tries.get(&id).copied().unwrap_or(0);
                                div()
                                    .relative()
                                    .size_full()
                                    .child(crate::ui::webview_skeleton::skeleton(secs, tries, p))
                                    .into_any_element()
                            };
                            // Хедер `h1` + «Contributed by an
                            // extension.» и `.bodyFlush`
                            // (`CustomizePanel.tsx:31-40`)
                            crate::ui::customize::contrib_page(&contrib_name, contrib_body, p)
                        } else {
                            crate::ui::customize::customize_panel(
                                self.cz.customize_panel,
                                self.cz.app_prefs,
                                self.cz.extensions.as_ref(),
                                &self.cz.ext_icons,
                                &self.output,
                                &self.cz.system_log,
                                self.cz
                                    .log_filter_input
                                    .as_ref()
                                    .map(|inp| (inp, self.cz.log_filter_value.clone())),
                                log_filter_focused,
                                &self.cz.logs_scroll,
                                self.cz.syslog_level,
                                &self.cz.ext_status,
                                self.cz.legacy_bridge.as_ref(),
                                self.cz.legacy_removing,
                                &self.design,
                                self.cz.design_input.as_ref(),
                                design_input_focused,
                                self.theme == kamin_theme::ThemeKind::Light,
                                &self.tx,
                                p,
                            )
                        },
                    )
                    .into_any_element(),
                )),
            )
        })
    }
}
