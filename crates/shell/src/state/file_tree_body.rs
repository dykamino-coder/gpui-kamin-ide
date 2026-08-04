//! Вставка компонента дерева файлов в тело тула (`plan/102-components.md`).
//!
//! Здесь и только здесь родитель делает три вещи: заводит сущность панели на
//! слот, обновляет её пропсы (панель сама решит, нужен ли `notify`) и вставляет
//! её КЭШИРУЕМЫМ элементом. Кэш и есть весь смысл: пока стор и пропсы не
//! менялись, gpui переигрывает прошлый кадр панели, а не считает его заново.

use crate::state::model::RootView;
use crate::ui::panels::file_tree_panel::{FileTreePanel, FileTreeProps};
use gpui::{
    AnyElement, AnyView, AppContext as _, Context, IntoElement, StyleRefinement, Styled as _,
};

/// probe-регион панели: СВОЙ на слот. Брать имя слота нельзя — регионы
/// `"sidebar"`/`"main"` уже заняты колонками, и дерево перетирало их
/// прямоугольники (ревью компонентизации).
fn probe_id_of(slot: &'static str) -> &'static str {
    match slot {
        "sidebar" => "file-tree@sidebar",
        "main" => "file-tree@main",
        "mainBottom" => "file-tree@mainBottom",
        "centralBottom" => "file-tree@centralBottom",
        "rightTop" => "file-tree@rightTop",
        "rightBottom" => "file-tree@rightBottom",
        _ => "file-tree",
    }
}

impl RootView {
    /// Ширина панели дерева в слоте — из раскладки, а не из probe прошлого
    /// кадра.
    fn tree_panel_width(&self, slot: &'static str) -> f32 {
        // Ширина панели минус её паддинги. Файловая колонка задана долей от
        // рабочей области — её пиксели считает раскладка кадра, поэтому берём
        // последнюю известную ширину области (probe колонки, не панели).
        let px = match slot {
            "sidebar" => self.layout.sidebar_width_px as f32,
            "rightTop" | "rightBottom" => self.layout.right_panel_width_px as f32,
            _ => crate::probe::registry::bounds_of("file-tree")
                .map(|[_, _, w, _]| w)
                .unwrap_or(320.0),
        };
        px.max(120.0)
    }

    /// Пропсы панели дерева для слота.
    fn file_tree_props(&self, slot: &'static str) -> FileTreeProps {
        FileTreeProps {
            workspace: self.workspace.clone().map(gpui::SharedString::from),
            width: self.tree_panel_width(slot),
            all_collapsed: self.tree_all_collapsed,
            has_editor_file: !self.ed.editor_tabs.is_empty(),
            theme: self.theme,
            look: kamin_theme::generation(),
        }
    }

    /// Обновить пропсы живых компонентов. Зовётся ПОСЛЕ применения события
    /// (`main.rs`), потому что `notify` из фазы отрисовки не заказывает кадр.
    pub(crate) fn sync_panels(&mut self, cx: &mut Context<Self>) {
        // Фокус-хендлы веб-вью: без них обёртка не получает клавиатуру.
        crate::web::ensure_focus_handles(cx);
        self.sync_web_visibility();
        // Слоты, где тул дерева уже не активен, панель не держим: её подписка
        // на стор дёргала бы перерисовку невидимого (ревью компонентизации).
        let live: Vec<&'static str> = crate::activity::PanelSlot::ALL
            .iter()
            .filter(|slot| self.activity.state(**slot).active.as_deref() == Some("tree"))
            .map(|slot| slot.as_str())
            .collect();
        self.file_tree_panels.retain(|slot, _| live.contains(slot));
        for slot in live {
            let props = self.file_tree_props(slot);
            if let Some(panel) = self.file_tree_panels.get(slot).cloned() {
                panel.update(cx, |panel, cx| panel.set_props(props, cx));
            }
        }
    }

    /// Сообщить CEF, какие вью сейчас видимы: скрытые дольше 20 с выгружаются
    /// (`web::reap_hidden`), возврат поднимает их из HTML-стора мгновенно.
    fn sync_web_visibility(&self) {
        // Панельные вью считаем видимыми ВСЕГДА, даже при открытых настройках:
        // раньше customize подменял множество целиком, чат уходил в hidden и
        // через 20с терял renderer — «настройки→назад перезагружают чат».
        let mut visible: Vec<String> = Vec::new();
        let mut registry_incomplete = false;
        for slot in crate::activity::PanelSlot::ALL {
            let Some(tool) = self.activity.state(slot).active.clone() else {
                continue;
            };
            let Some(d) = crate::activity::dyn_tool(&tool) else {
                // Активный contributed-тул без записи в реестре = снапшот
                // registry:update неполон (рестарт ext-host, гонка на смене
                // сессии). Не «скрываем» его вью — сохраняем прошлый набор.
                if crate::activity::is_singleton(&tool) {
                    registry_incomplete = true;
                }
                continue;
            };
            for v in d.views {
                if v.webview {
                    visible.push(v.id);
                }
            }
        }
        if self.layout.file_panel_visible && self.layout.file_panel_mode == "web" {
            visible.push("browser".to_string());
        }
        if self.cz.customize_open {
            if let Some(active) = self.cz.customize_contrib.clone() {
                // Открыта contributed-страница Customize — её вью под своим id.
                visible.push(active);
            }
        }
        if registry_incomplete {
            return; // прошлый mark_visible остаётся в силе до полного снапшота
        }
        crate::web::mark_visible(visible);
    }

    pub(crate) fn file_tree_body(
        &mut self,
        slot: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let panel = match self.file_tree_panels.get(slot) {
            Some(panel) => panel.clone(),
            None => {
                let tree = self.tree_store.clone();
                let tx = self.tx.clone();
                let probe_id = probe_id_of(slot);
                let props = self.file_tree_props(slot);
                let panel = cx.new(|cx| FileTreePanel::new(tree, tx, probe_id, props, cx));
                self.file_tree_panels.insert(slot, panel.clone());
                panel
            }
        };
        // `size_full`: кэшированный вью получает свой прямоугольник от стиля,
        // переданного здесь, а не от собственного корня.
        AnyView::from(panel)
            .cached(StyleRefinement::default().size_full())
            .into_any_element()
    }
}
