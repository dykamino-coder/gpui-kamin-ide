//! Customize: страницы настроек — темы, иконочные темы, горячие клавиши,
//! элементы статус-бара и страницы от расширений.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::CzEvent;
use crate::host::events::TreeEvent;
use crate::host_link::{self, ShellEvent};
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Customize: страницы настроек — темы, иконочные темы, горячие клавиши.
    pub(crate) fn apply_customize_pages(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Cz(CzEvent::ExtensionIcon(id, url)) => {
                self.cz.ext_icons.insert(id, url);
            }
            ShellEvent::Cz(CzEvent::StatusBarSnapshot(items)) => {
                self.status_items.clear();
                if let Some(arr) = items.as_array() {
                    for v in arr {
                        if let Some(it) = crate::ui::status_bar::ContribItem::from_value(v) {
                            self.status_items.insert(it.id.clone(), it);
                        }
                    }
                }
            }
            ShellEvent::Cz(CzEvent::StatusBarUpsert(v)) => {
                if let Some(it) = crate::ui::status_bar::ContribItem::from_value(&v) {
                    if it.visible {
                        self.status_items.insert(it.id.clone(), it);
                    } else {
                        self.status_items.remove(&it.id);
                    }
                }
            }
            ShellEvent::Cz(CzEvent::StatusBarRemove(id)) => {
                self.status_items.remove(&id);
            }
            ShellEvent::Cz(CzEvent::ThemesList(themes)) => {
                // Restore персиста contributed-темы при ПЕРВОМ списке (бут без
                // кэша: contrib_theme_id ещё None). С кэшем тема уже применена
                // на старте — реестр её только подтверждает.
                if self.contrib_theme_id.is_none()
                    && let Some(saved) = crate::layout_store::load_raw_key("contributedThemeId")
                    && let Some(saved_id) = saved.as_str()
                    && let Some((id, _, path, dark_ui)) =
                        themes.iter().find(|(id, _, _, _)| id == saved_id).cloned()
                {
                    let _ = self
                        .tx
                        .try_send(ShellEvent::Cz(CzEvent::SetContributedTheme(
                            id, path, dark_ui,
                        )));
                }
                // ФОЛБЭК: расширения загрузились (список НЕ пуст), а активной
                // темы среди них нет — поставщик удалён/выключен. Откат на
                // builtin + чистка персиста и бут-кэша (иначе кэш поднимал бы
                // мёртвую тему вечно).
                if !themes.is_empty()
                    && let Some(active) = self.contrib_theme_id.clone()
                    && !themes.iter().any(|(id, _, _, _)| *id == active)
                {
                    let _ = std::fs::remove_file(crate::theme_sync::theme_cache_path());
                    let choice = self.theme_choice;
                    let _ = self.tx.try_send(ShellEvent::SetThemeChoice(choice));
                }
                self.contrib_themes = themes;
            }
            ShellEvent::Cz(CzEvent::IconThemesList(list)) => {
                // Реестр contributed приехал → чистим слоты от id, которых нет
                // ни среди builtin, ни в расширениях (в persist оставался
                // мусор из отладочных прогонов: панель показывала заглушку
                // несуществующего тула, плитки для него в баре нет)
                if self.activity.prune_unknown() {
                    self.save_activity();
                }
                // Restore персиста icon-темы при первом списке
                if self.icon_theme_id.is_none()
                    && let Some(saved) = crate::layout_store::load_raw_key("iconThemeId")
                    && let Some(saved_id) = saved.as_str()
                    && let Some((id, _, path)) =
                        list.iter().find(|(id, _, _)| id == saved_id).cloned()
                {
                    let _ = self
                        .tx
                        .try_send(ShellEvent::Cz(CzEvent::SetIconTheme(Some((id, path)))));
                }
                self.icon_themes = list;
            }
            ShellEvent::Cz(CzEvent::SetIconTheme(choice)) => match choice {
                None => {
                    crate::icon_theme::set_active(None);
                    self.icon_theme_id = None;
                    crate::layout_store::save_patch(
                        serde_json::json!({ "iconThemeId": serde_json::Value::Null }),
                    );
                }
                Some((id, path)) => {
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        let Some(client) = host_link::client() else {
                            return;
                        };
                        match client.request("kamin:iconTheme:load", vec![serde_json::json!(path)])
                        {
                            Ok(doc) => {
                                let theme = crate::icon_theme::IconTheme::parse(&doc);
                                let _ = tx.try_send(ShellEvent::Cz(CzEvent::IconThemeLoaded(
                                    id,
                                    Box::new(theme),
                                )));
                            }
                            Err(e) => eprintln!("iconTheme load failed: {e}"),
                        }
                    });
                }
            },
            ShellEvent::Cz(CzEvent::IconThemeLoaded(id, theme)) => {
                crate::icon_theme::set_active(Some(*theme));
                crate::layout_store::save_patch(serde_json::json!({ "iconThemeId": &id }));
                self.icon_theme_id = Some(id);
            }
            ShellEvent::Cz(CzEvent::UpdateAvailable(version, url)) => {
                self.update_available = Some((version, url));
            }
            ShellEvent::Cz(CzEvent::KeybindingsList(items)) => {
                self.contrib_keys = crate::contrib_keys::build_map(&items);
            }
            ShellEvent::Cz(CzEvent::CustomizePages(pages)) => {
                // Contributed-страницы обязаны попасть в реестр известных
                // вью: и HTML (`kamin:webviewView:html`), и сообщения
                // (`kamin:webview:post`) фильтруются по `webview_known`, а
                // `KNOWN_WEBVIEWS` знает только chat/plan/console. Без этой
                // регистрации страница получала resolve, но её HTML молча
                // выбрасывался — «Loading…» навсегда.
                for page in pages.iter().flat_map(|c| &c.views) {
                    host_link::register_dynamic_webview(&page.id);
                    // Ресолв — ТОЛЬКО по открытию страницы. Ранний ресолв всех
                    // десяти давал шторм: каждая отдаёт ~1.6 МБ HTML и свои
                    // стартовые invoke, то есть ~16 МБ и под сотню запросов
                    // разом — данные активной страницы приходили через 40 с.
                }
                self.cz.customize_pages = pages;
            }
            ShellEvent::Tree(TreeEvent::RevealView(container, view)) => {
                // `ipc.ts:290-317`: у customize-контейнера reveal = открыть
                // Customize на КОНКРЕТНОЙ странице (`p.view ?? container`),
                // у остальных — пин/активация слота
                let is_cz = self.cz.customize_pages.iter().any(|c| c.id == container);
                if is_cz {
                    let page = view.unwrap_or_else(|| container.clone());
                    let _ = self
                        .tx
                        .try_send(ShellEvent::Cz(CzEvent::SetCustomizeContribPage(page)));
                } else {
                    use crate::activity::PanelSlot;
                    // Порядок предпочтения слотов при дублирующем пине
                    // (`ipc.ts:309`); отсутствие пина — дефолт по location
                    const PREF: [PanelSlot; 6] = [
                        PanelSlot::Main,
                        PanelSlot::MainBottom,
                        PanelSlot::CentralBottom,
                        PanelSlot::RightTop,
                        PanelSlot::RightBottom,
                        PanelSlot::Sidebar,
                    ];
                    let containing: Vec<PanelSlot> = PanelSlot::ALL
                        .into_iter()
                        .filter(|s| self.activity.is_pinned(*s, &container))
                        .collect();
                    let target = match containing.first() {
                        Some(first) => PREF
                            .into_iter()
                            .find(|s| containing.contains(s))
                            .unwrap_or(*first),
                        None => match crate::activity::dyn_tool(&container)
                            .as_ref()
                            .map(|t| t.location.as_str())
                        {
                            Some("auxiliarybar") => PanelSlot::RightTop,
                            Some("panel") => PanelSlot::CentralBottom,
                            _ => PanelSlot::Sidebar,
                        },
                    };
                    // Дубли пина рисуют вебвью ДВАЖДЫ — схлопываем в один слот
                    for s in containing {
                        if s != target {
                            self.activity.unpin(s, &container);
                        }
                    }
                    self.activity.pin(target, &container);
                    if target == PanelSlot::Sidebar {
                        self.sidebar_visible = true;
                    }
                    self.save_activity();
                }
            }
            ShellEvent::Cz(CzEvent::SetCustomizeContribPage(id)) => {
                // Кеш с прошлого запуска — страница открывается МГНОВЕННО, а
                // resolve уходит в фон лишь чтобы обновить. Без этого каждая
                // страница тянула ~1.6 МБ HTML через IPC и WS хоста, и его
                // цикл вставал полусекундными паузами (замер: 16 пауз ≈ 7 с).
                if !crate::ui::chat_webview::has_html(&id) && !self.pending_html.contains_key(&id) {
                    let cache = crate::host_link::data_dirs()
                        .1
                        .join("webview-html")
                        .join(format!("{id}.html"));
                    if let Ok(html) = std::fs::read_to_string(&cache)
                        && !html.is_empty()
                    {
                        self.pending_html.insert(id.clone(), html);
                    }
                }
                // Выбор страницы Customize подразумевает сам режим Customize:
                // иначе страница «выбрана», но экран показывает рабочую область
                self.cz.customize_open = true;
                self.cz.customize_panel = "contrib";
                // Другая страница — прежняя навигация больше не актуальна
                if self.cz.customize_contrib.as_deref() != Some(id.as_str()) {
                    self.cz.cz_nav_done = None;
                }
                if !crate::ui::chat_webview::has_html(&id) {
                    host_link::resolve_webview(id.clone());
                }
                self.cz.customize_contrib = Some(id);
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
