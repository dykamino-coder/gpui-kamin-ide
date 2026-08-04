//! Бутстрап страниц кадра: вью расширений и общая страница Customize.
//!
//! Сами страницы рисует CEF (`web/`), здесь только их HTML: он приходит от
//! расширения, кладётся в стор и на диск, а вью поднимает `ui/webview_body.rs`.

use crate::state::model::RootView;
use gpui::{Context, Window};

/// Где лежит HTML вью между запусками.
fn html_cache_path(id: &str) -> std::path::PathBuf {
    let (_, cache) = crate::host_link::data_dirs();
    cache.join("webview-html").join(format!("{id}.html"))
}

/// Сохранить HTML вью на диск — чтобы следующий запуск рисовал панель сразу.
fn cache_html(id: &str, html: &str) {
    let path = html_cache_path(id);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, html);
}

impl RootView {
    pub(crate) fn frame_webviews(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // Вью АКТИВНЫХ contributed-тулов (Bridge Todos/Agents и прочие вклады)
        // не входят в `KNOWN_WEBVIEWS`: без них панель вечно висела в скелете
        // «попытка N», а resolve уходил в пустоту (баг нашёл пользователь).
        // Берём только активные — держать страницу под каждый вклад дорого.
        let active_dyn_views: Vec<String> = crate::activity::PanelSlot::ALL
            .iter()
            .filter_map(|slot| self.activity.state(*slot).active.clone())
            .filter_map(|id| crate::activity::dyn_tool(&id))
            .flat_map(|d| d.views)
            .filter(|v| v.webview)
            .map(|v| v.id)
            .filter(|v: &String| !crate::host_link::KNOWN_WEBVIEWS.contains(&v.as_str()))
            .collect();
        for id in crate::host_link::KNOWN_WEBVIEWS
            .iter()
            .copied()
            .map(String::from)
            .chain(active_dyn_views)
        {
            let id: &str = &id;
            // Расширение активируется секунд восемь — до его ответа рисуем
            // вчерашнюю страницу с диска, иначе панели висят пустыми.
            // Кэши старого формата (тема-блок запечён в начало страницы)
            // пропускаем: тему теперь вставляет обработчик схемы при отдаче,
            // а запечённый блок стоял бы ПОЗЖЕ в документе и перебивал её.
            if !crate::ui::chat_webview::has_html(id)
                && !self.pending_html.contains_key(id)
                && let Ok(html) = std::fs::read_to_string(html_cache_path(id))
                && !html.is_empty()
                && !html.starts_with("<style id=\"__kaminThemeVars\">")
            {
                self.pending_html.insert(id.to_string(), html);
            }
            if let Some(html) = self.pending_html.remove(id) {
                cache_html(id, &html);
                crate::ui::chat_webview::stage_html(id, html);
            }
        }
        // Contributed-страница Customize живёт под СВОИМ id (бандл выбирает
        // раздел из pathname — общий czShared-контейнер упразднён).
        if let Some(active) = self.cz.customize_contrib.clone()
            && let Some(html) = self.pending_html.remove(&active)
        {
            cache_html(&active, &html);
            crate::ui::chat_webview::stage_html(&active, html);
            self.cz.cz_nav_done = Some(active);
        }
    }
}
