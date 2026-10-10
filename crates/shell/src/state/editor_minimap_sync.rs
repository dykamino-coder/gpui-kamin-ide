//! Зеркало минимапы у КАЖДОГО таба редактора (INC-2026-0059).
//!
//! Раньше зеркало было одно на весь редактор и пересоздавалось только при
//! открытии нового файла: после переключения на уже открытый таб минимапа
//! показывала последний открытый файл, а правки буфера в неё не попадали
//! (флаг `minimap_stale` ставился, но никем не читался). Zed держит
//! `minimap_editor` на каждый редактор и делит с ним буфер; у нас буфер
//! отдельный, поэтому таб помечает зеркало устаревшим на `InputEvent::Change`,
//! а кадр копирует текст только в зеркало АКТИВНОГО таба и не чаще раза за
//! кадр — неактивные табы догоняются при активации.

use crate::state::editor_tab::EditorTab;
use gpui::{AppContext as _, Entity, Window};
use gpui_component::input::EditorState as CodeEditorState;

/// Read-only зеркало для минимапы: тот же текст и язык, без номеров строк.
pub(crate) fn new_mirror(
    lang: &'static str,
    text: String,
    window: &mut Window,
    cx: &mut gpui::App,
) -> Entity<CodeEditorState> {
    cx.new(|cx| {
        // Zed-минимапа не рисует номера строк и не подсвечивает текущую
        // строку/выделение — это чистый силуэт текста.
        let mut st = CodeEditorState::new(window, cx)
            .language(lang)
            .folding(false)
            .line_number(false)
            // Zed `EditorMode::Minimap`: read-only, без подписок и каретки.
            // Флаг ещё и снимает жёсткий line-height `Input`-а, иначе строки
            // идут через 20px.
            .minimap()
            .soft_wrap(false);
        st.set_value(text, window, cx);
        st
    })
}

/// Догнать зеркало таба до его буфера, если буфер менялся.
///
/// `set_value` сбрасывает прокрутку в 0; положение контента минимапы
/// производно от прокрутки редактора, поэтому прежний offset возвращаем,
/// иначе каждая правка внизу длинного файла дёргала бы минимапу к началу.
pub(crate) fn sync_mirror(tab: &mut EditorTab, window: &mut Window, cx: &mut gpui::App) {
    if !tab.minimap_stale {
        return;
    }
    tab.minimap_stale = false;
    let text = tab.input.read(cx).value();
    if tab.minimap.read(cx).value() == text {
        return;
    }
    tab.minimap.update(cx, |st, cx| {
        let off = st.scroll_handle.offset();
        st.set_value(text, window, cx);
        st.scroll_handle.set_offset(off);
    });
}

#[cfg(test)]
#[path = "editor_minimap_sync_tests.rs"]
mod tests;
