//! Тело вебвью-панелей: курсор, проброс ввода, динамические вью.
//!
//! Методы перенесены из `root.rs` дословно (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, div, px};
use kamin_theme::Palette;
/// Гап-обёртка колонки/карты: 4px ТОЛЬКО по горизонтали → 8px между
/// соседями, 4px у краёв окна (эмуляция `.body { gap: 8px; padding: 0 4px }`).
/// Вертикального паддинга у оригинала НЕТ: карты вплотную к титлбару и
/// статус-бару, шов между стопками закрывает ручка.
/// min 0 — иначе min-content контента (длинные имена) распирает flex-item
/// и выталкивает соседей (rail) за окно.
pub(crate) fn gap_wrap(el: impl IntoElement) -> AnyElement {
    div()
        .size_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .overflow_hidden()
        .px(px(4.0))
        .child(el)
        .into_any_element()
}

/// webview_body для ДИНАМИЧЕСКОГО view-id (contributed тулы; без probe_area —
/// probe-реестр требует 'static id).
/// Кэш-leak динамических view-id: probe-реестр и fwd-замыкания требуют
/// &'static str; набор вью мал и стабилен — утечка единовременная.
pub(crate) fn static_view_id(id: &str) -> &'static str {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, &'static str>>,
    > = std::sync::OnceLock::new();
    let m = CACHE.get_or_init(Default::default);
    let mut g = m.lock().unwrap();
    if let Some(v) = g.get(id) {
        return v;
    }
    let leaked: &'static str = Box::leak(id.to_string().into_boxed_str());
    g.insert(id.to_string(), leaked);
    leaked
}

pub fn webview_body_dyn(
    id: String,
    alive: bool,
    placeholder: (&'static str, crate::ui::panel_placeholder::SlotIcon),
    // Ожидание resolve: (секунды, попытка, бюджет исчерпан). None = вью жив.
    load: Option<(u64, u32, bool)>,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let has_html = crate::ui::chat_webview::has_html(&id);
    // Вью моста на CEF: страница живёт в кадре, без дырки в фоне и без
    // отдельного окна.
    #[cfg(windows)]
    if crate::web::enabled() && has_html && alive {
        let sid = static_view_id(&id);
        // Поднимаем вью моста и перечитываем страницу, если стор её обновил.
        crate::web::ensure_html_view(&id);
        return gpui::div()
            .size_full()
            .p(gpui::px(8.0))
            .child(crate::web::web_view(sid, kamin_metrics::RADIUS_MD))
            // Область под именем вью: по ней стенд находит панель, не гадая по
            // раскладке (`scripts/check_chat_ipc.py`).
            .child(crate::probe::registry::probe_area(sid))
            .into_any_element();
    }
    // Вью ещё не поднялся: скелет загрузки, а по исчерпании бюджета resolve —
    // терминальная ошибка с Retry (`WebviewLoadingSkeleton.tsx`). Раньше здесь
    // рисовался плейсхолдер ПУСТОГО слота — «Open new tool or drag-n-drop…»
    // на панели, которая на самом деле грузится.
    // Оригинал (`ContributedContainerBody.tsx:87,139`) показывает скелет, пока
    // НЕТ html; случай «alive, но html ещё не пришёл» падал в плейсхолдер
    // ПУСТОГО слота (ревью ц.13)
    let load = load.or(if has_html { None } else { Some((0, 0, false)) });
    if let Some((secs, tries, exhausted)) = load {
        let body = if exhausted {
            crate::ui::webview_skeleton::load_error(&id, tx, p)
        } else {
            crate::ui::webview_skeleton::skeleton(secs, tries, p)
        };
        return div().relative().size_full().child(body).into_any_element();
    }
    crate::ui::panel_placeholder::panel_placeholder(
        placeholder.0,
        "Open new tool or drag-n-drop tool from other panels",
        placeholder.1,
        p,
    )
}
