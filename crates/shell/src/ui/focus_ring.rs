//! `:focus-visible` (`global.css:34-43`): `outline: 2px solid accent-primary;
//! outline-offset: 2px` — только для КЛАВИАТУРНОГО фокуса; клик мышью кольцо
//! не показывает.
//!
//! Клавиатурную модальность различает вендорный патч gpui
//! (`Window::focus_visible`). Здесь — реестр фокус-хэндлов по строковому id:
//! рисующие функции (`tool_btn` и т.п.) не получают ни `cx`, ни `window`,
//! поэтому хэндлы чеканятся один раз в конце кадра (`flush`), а состояние
//! фокуса снимается в начале (`begin_frame`).

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use gpui::prelude::*;
use gpui::{App, FocusHandle, Window, div, px};

/// Толщина кольца (`outline: 2px`).
const RING: f32 = 2.0;
/// Зазор до элемента (`outline-offset: 2px`).
const OFFSET: f32 = 2.0;

static HANDLES: LazyLock<Mutex<HashMap<String, FocusHandle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
/// id, которые запросили в этом кадре, но хэндла ещё нет.
static PENDING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
/// Кадровый снимок: (клавиатурная модальность, id сфокусированного элемента).
static FRAME: LazyLock<Mutex<(bool, Option<String>)>> = LazyLock::new(|| Mutex::new((false, None)));

/// Снять состояние фокуса на кадр. Вызывать в начале `RootView::render`.
pub fn begin_frame(window: &Window) {
    let visible = window.focus_visible();
    let focused = HANDLES
        .lock()
        .unwrap()
        .iter()
        .find(|(_, h)| h.is_focused(window))
        .map(|(k, _)| k.clone());
    *FRAME.lock().unwrap() = (visible, focused);
}

/// Отчеканить хэндлы, запрошенные за кадр. Возвращает true, если появились
/// новые — тогда нужен ещё один кадр, чтобы элементы их подхватили.
pub fn flush(cx: &mut App) -> bool {
    let pending: Vec<String> = PENDING.lock().unwrap().drain().collect();
    if pending.is_empty() {
        return false;
    }
    let mut handles = HANDLES.lock().unwrap();
    for id in pending {
        // `.tab_index()` элемента применяется только к автоматически
        // созданному хэндлу (`div.rs:1584-1598`); свой помечаем сами, иначе
        // он попадает в `tab_stops` как НЕ-остановка и Tab его пропускает
        handles
            .entry(id)
            .or_insert_with(|| cx.focus_handle().tab_index(0).tab_stop(true));
    }
    true
}

/// Сделать элемент фокусируемым (tab-стоп) и надеть кольцо, когда фокус
/// пришёл с клавиатуры. `radius` — скругление самого элемента: кольцо шире на
/// зазор, поэтому его радиус больше на ту же величину.
pub fn focusable<E>(el: E, id: &str, radius: f32, accent: gpui::Rgba) -> E
where
    E: gpui::InteractiveElement + ParentElement + Styled,
{
    let handle = HANDLES.lock().unwrap().get(id).cloned();
    let Some(handle) = handle else {
        PENDING.lock().unwrap().insert(id.to_string());
        return el;
    };
    let (visible, focused) = {
        let f = FRAME.lock().unwrap();
        (f.0, f.1.clone())
    };
    let el = el.track_focus(&handle);
    if !(visible && focused.as_deref() == Some(id)) {
        return el;
    }
    // `outline` рисуется ВНЕ бокса и не двигает раскладку — у нас это
    // абсолютный оверлей, вынесенный на `offset + ring` наружу
    el.relative().child(
        div()
            .absolute()
            .top(px(-(OFFSET + RING)))
            .left(px(-(OFFSET + RING)))
            .right(px(-(OFFSET + RING)))
            .bottom(px(-(OFFSET + RING)))
            .border(px(RING))
            .border_color(accent)
            .rounded(px(radius + OFFSET + RING)),
    )
}

/// id элемента в фокусе (без учёта клавиатурной модальности) — нужно
/// обработчикам, которые у оригинала висят на САМОЙ строке.
pub fn focused_id() -> Option<String> {
    FRAME.lock().unwrap().1.clone()
}

/// Сфокусировать элемент по id (если хэндл уже отчеканен). Нужно там, где
/// оригинал ставит `autoFocus`/`ref.focus()` — например кнопке Confirm
/// модалки (`ConfirmModal.tsx:46-56`).
pub fn focus_id(id: &str, window: &mut Window) -> bool {
    let handle = HANDLES.lock().unwrap().get(id).cloned();
    match handle {
        Some(h) => {
            window.focus(&h);
            true
        }
        None => {
            // Хэндла ещё нет — попросим отчеканить на следующем кадре
            PENDING.lock().unwrap().insert(id.to_string());
            false
        }
    }
}

/// Снимок для probe: (клавиатурная модальность, id в фокусе, все tab-стопы).
pub fn debug_state() -> (bool, Option<String>, Vec<String>) {
    let f = FRAME.lock().unwrap();
    let mut ids: Vec<String> = HANDLES.lock().unwrap().keys().cloned().collect();
    ids.sort();
    (f.0, f.1.clone(), ids)
}

/// Тот же `focusable`, но методом — чтобы вешать кольцо ВНУТРИ цепочки
/// builder-а, не разрывая её на биндинги.
pub trait FocusRing: Sized {
    fn focus_ring(self, id: &str, radius: f32, accent: gpui::Rgba) -> Self;
}

impl<E> FocusRing for E
where
    E: gpui::InteractiveElement + ParentElement + Styled,
{
    fn focus_ring(self, id: &str, radius: f32, accent: gpui::Rgba) -> Self {
        focusable(self, id, radius, accent)
    }
}
