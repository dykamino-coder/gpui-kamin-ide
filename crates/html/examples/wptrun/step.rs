//! Шаг документа перед кадром окна (`Document::update_rendering`): кадр
//! области просмотра собирается и раскладывается до `Window::draw`, как в
//! приложении. Стопка страниц печатной пары собирается в кадре, как прежде.

use super::page::Page;
use gpui::{Entity, ElementPlace, Window};

/// Зарегистрировать шаг `page` в его окне на всё время жизни окна.
pub(super) fn register(page: &Entity<Page>, window: &mut Window) {
    // Документ — корень рендера view (`Page::render`).
    let parent = ElementPlace::of_view(page.entity_id());
    let page = page.downgrade();
    window
        .on_update_rendering(move |window, cx| {
            let _ = page.update(cx, |page, cx| {
                if !page.paged() {
                    let opts = page.opts(window, None);
                    page.doc.update_rendering(&opts, &parent, window, cx, |_| {});
                }
            });
        })
        .detach();
}
